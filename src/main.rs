use std::io;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::signal::ctrl_c;

use animus_tui::app::App;
use animus_tui::control_client::{default_socket_path, RealControlClient};
use animus_tui::theme;

#[derive(Parser, Debug)]
#[command(
    name = "animus-tui",
    about = "Terminal control plane for the Animus daemon",
    version
)]
struct Args {
    /// Override the daemon control socket path
    #[arg(long)]
    daemon_socket: Option<PathBuf>,

    /// Override the project root used to resolve the default socket path
    #[arg(long)]
    project_root: Option<PathBuf>,

    /// Emit the plugin manifest JSON and exit. Used by `animus plugin
    /// install` to verify identity before committing the install.
    #[arg(long)]
    manifest: bool,
}

use animus_tui::PLUGIN_MANIFEST_JSON;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.manifest {
        print!("{}", PLUGIN_MANIFEST_JSON);
        return Ok(());
    }

    let socket = match args.daemon_socket {
        Some(p) => p,
        None => default_socket_path(args.project_root.as_deref())
            .context("could not resolve default daemon socket path")?,
    };

    if !socket.exists() {
        print_no_daemon_splash(&socket);
        return Ok(());
    }

    let client = match RealControlClient::connect(&socket).await {
        Ok(c) => c,
        Err(err) => {
            print_no_daemon_splash(&socket);
            eprintln!("\nUnderlying error: {err:#}");
            return Ok(());
        }
    };

    // Only pin the scope dir from the socket's parent when the socket
    // lives under `~/.animus/<scope>/control.sock`. Custom `--daemon-socket`
    // overrides (test sockets, sshfs mounts, etc.) shouldn't be treated as
    // state roots — let App::bootstrap ask the daemon for project_root in
    // that case so the Cost view reads from the real scoped directory.
    let scope_dir = socket.parent().and_then(|parent| {
        let home = dirs::home_dir()?;
        let ao = home.join(".animus");
        if parent.starts_with(&ao) {
            Some(parent.to_path_buf())
        } else {
            None
        }
    });

    install_panic_hook();
    let mut terminal = setup_terminal()?;
    let app_result = run_app(&mut terminal, client, scope_dir).await;
    restore_terminal(&mut terminal)?;
    app_result
}

fn print_no_daemon_splash(socket: &std::path::Path) {
    let bold = theme::ansi_bold_supported();
    let (b, r) = if bold {
        ("\x1b[1m", "\x1b[0m")
    } else {
        ("", "")
    };
    println!("{b}Animus daemon is not running.{r}");
    println!();
    println!("Start it with:    animus daemon start");
    println!();
    println!("Expected socket:  {}", socket.display());
}

fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original(info);
    }));
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend).context("failed to construct terminal")?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode().ok();
    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();
    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    client: RealControlClient,
    scope_dir: Option<std::path::PathBuf>,
) -> Result<()> {
    let mut app = App::new(Box::new(client));
    if let Some(p) = scope_dir {
        app.set_scoped_state_root(p);
    }
    app.bootstrap().await;

    let tick = Duration::from_millis(100);
    let ctrl_c_fut = ctrl_c();
    tokio::pin!(ctrl_c_fut);

    loop {
        terminal.draw(|f| app.render(f))?;

        tokio::select! {
            biased;
            _ = &mut ctrl_c_fut => {
                break;
            }
            _ = tokio::time::sleep(tick) => {
                app.tick().await;
            }
        }

        // Drain terminal input events without blocking.
        while crossterm::event::poll(Duration::from_millis(0))? {
            let ev = crossterm::event::read()?;
            if let crossterm::event::Event::Key(key) = ev {
                if app.on_key(key).await {
                    return Ok(());
                }
            }
        }

        if app.should_quit() {
            break;
        }
    }
    Ok(())
}
