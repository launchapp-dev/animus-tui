use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::KeyEvent;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use tokio::sync::Mutex;

use animus_control_protocol::types::{
    DaemonLogEntry, PluginListRequest, QueueListRequest, SubjectListRequest, WorkflowListRequest,
    WorkflowStatus,
};
use animus_subject_protocol::SubjectFilter;
use tokio::sync::mpsc;

use crate::control_client::ControlAccess;
use crate::keybinds::{translate, translate_after_t, Action};
use crate::theme::Theme;
use crate::views::{
    cost::CostView, health::HealthView, logs::LogsView, plugins::PluginsView, queue::QueueView,
    subjects::SubjectsView, workflows::WorkflowsView, ViewId,
};
use crate::widgets::{render_sticky_header, StickyHeaderState};

/// Refresh cadence: each view auto-refreshes on this interval.
const REFRESH_EVERY: Duration = Duration::from_secs(3);

pub struct App {
    client: Arc<dyn ControlAccess>,
    pub view: ViewId,
    pub header: StickyHeaderState,
    pub workflows: WorkflowsView,
    pub subjects: SubjectsView,
    pub queue: QueueView,
    pub health: HealthView,
    pub logs: LogsView,
    pub cost: CostView,
    pub plugins: PluginsView,
    pub help_open: bool,
    pub awaiting_t_prefix: bool,
    last_refresh: Arc<Mutex<Instant>>,
    quit: bool,
    scoped_state_root: Option<PathBuf>,
    log_rx: Option<mpsc::Receiver<DaemonLogEntry>>,
}

impl App {
    pub fn new(client: Box<dyn ControlAccess>) -> Self {
        Self {
            client: Arc::from(client),
            view: ViewId::Workflows,
            header: StickyHeaderState::default(),
            workflows: WorkflowsView::default(),
            subjects: SubjectsView::default(),
            queue: QueueView::default(),
            health: HealthView::default(),
            logs: LogsView::default(),
            cost: CostView::default(),
            plugins: PluginsView::default(),
            help_open: false,
            awaiting_t_prefix: false,
            last_refresh: Arc::new(Mutex::new(
                Instant::now() - REFRESH_EVERY - Duration::from_secs(1),
            )),
            quit: false,
            scoped_state_root: None,
            log_rx: None,
        }
    }

    pub fn set_scoped_state_root(&mut self, p: PathBuf) {
        self.scoped_state_root = Some(p);
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    pub async fn bootstrap(&mut self) {
        // Resolve scope root by asking the daemon for its project_root. This
        // only runs if the caller didn't already pin a scope dir from the
        // connected socket path — that's the more reliable signal because
        // the socket lives in the scope the daemon actually adopted.
        if self.scoped_state_root.is_none() {
            if let Ok(s) = self.client.daemon_status().await {
                if let Some(pr) = &s.project_root {
                    if let Some(home) = dirs::home_dir() {
                        let scope_dir = crate::control_client::scope_dir_for_project_root(
                            home.as_path(),
                            pr.as_path(),
                        );
                        self.scoped_state_root = Some(scope_dir);
                    }
                }
                self.header.daemon_connected = s.running;
            }
        }
        // Start the log tail stream. RealControlClient subscribes to
        // daemon/logs --follow; fakes return an empty receiver.
        self.log_rx = Some(self.client.spawn_log_stream().await);
        self.refresh_now().await;
    }

    pub async fn tick(&mut self) {
        // Drain any pending log entries into LogsView without blocking.
        if let Some(rx) = &mut self.log_rx {
            loop {
                match rx.try_recv() {
                    Ok(entry) => self.logs.ingest(entry),
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        self.log_rx = None;
                        break;
                    }
                }
            }
        }

        let due = {
            let mut last = self.last_refresh.lock().await;
            if last.elapsed() >= REFRESH_EVERY {
                *last = Instant::now();
                true
            } else {
                false
            }
        };
        if due {
            self.refresh_now().await;
        }
    }

    pub async fn refresh_now(&mut self) {
        // daemon status + health (cheap, drives sticky header)
        match self.client.daemon_status().await {
            Ok(s) => {
                self.header.daemon_connected = s.running;
                self.health.status = Some(s);
            }
            Err(e) => {
                self.header.daemon_connected = false;
                self.health.set_error(format!("daemon_status: {e}"));
            }
        }
        match self.client.daemon_health().await {
            Ok(h) => {
                self.header.daemon_health = Some(h.status);
                if let Some(s) = &self.health.status {
                    self.health.set(s.clone(), h);
                }
            }
            Err(e) => {
                self.header.daemon_health = None;
                self.health.set_error(format!("daemon_health: {e}"));
            }
        }

        // workflows
        match self
            .client
            .workflow_list(WorkflowListRequest {
                status: None,
                cursor: None,
                limit: Some(100),
            })
            .await
        {
            Ok(r) => {
                self.header.active_workflows = r
                    .runs
                    .iter()
                    .filter(|x| {
                        matches!(
                            x.status,
                            WorkflowStatus::Running
                                | WorkflowStatus::Pending
                                | WorkflowStatus::Paused
                        )
                    })
                    .count();
                self.workflows.set(r.runs);
            }
            Err(e) => self.workflows.set_error(format!("workflow_list: {e}")),
        }

        // subjects (current kind)
        let filter = SubjectFilter {
            kind: vec![self.subjects.kind.clone()],
            ..Default::default()
        };
        match self
            .client
            .subject_list(SubjectListRequest { filter })
            .await
        {
            Ok(r) => self.subjects.set(r.subjects),
            Err(e) => self.subjects.set_error(format!("subject_list: {e}")),
        }

        // queue (use stats for accurate header counts; list for the visible table)
        if let Ok(stats) = self.client.queue_stats().await {
            self.header.queue_depth = (stats.ready + stats.held + stats.in_flight) as usize;
        }
        match self
            .client
            .queue_list(QueueListRequest {
                status: None,
                cursor: None,
                limit: Some(200),
            })
            .await
        {
            Ok(r) => self.queue.set(r.entries),
            Err(e) => self.queue.set_error(format!("queue_list: {e}")),
        }

        // plugins
        match self
            .client
            .plugin_list(PluginListRequest {
                include_warnings: false,
                kind: None,
            })
            .await
        {
            Ok(r) => self.plugins.set(r.plugins),
            Err(e) => self.plugins.set_error(format!("plugin_list: {e}")),
        }

        // cost (best effort, file may not exist)
        if let Some(root) = &self.scoped_state_root {
            self.cost.load(root);
        }
        // Surface top-spend workflow in the sticky header.
        self.header.top_spend_workflow = self.cost.state.as_ref().and_then(|s| {
            s.workflows
                .iter()
                .max_by(|a, b| {
                    a.cost_usd
                        .partial_cmp(&b.cost_usd)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|w| (w.workflow_id.clone(), w.cost_usd))
        });
    }

    pub async fn on_key(&mut self, key: KeyEvent) -> bool {
        if self.help_open {
            self.help_open = false;
            return false;
        }
        if self.awaiting_t_prefix {
            self.awaiting_t_prefix = false;
            if let Some(a) = translate_after_t(key) {
                self.dispatch_action(a).await;
            }
            return false;
        }

        // Special two-key prefix "t…"
        if let crossterm::event::KeyCode::Char('t') = key.code {
            self.awaiting_t_prefix = true;
            return false;
        }

        let action = translate(key);
        self.dispatch_action(action).await;
        self.quit
    }

    async fn dispatch_action(&mut self, action: Action) {
        match action {
            Action::Quit => self.quit = true,
            Action::Help => self.help_open = true,
            Action::Switch(n) => self.view = ViewId::from_idx(n),
            Action::Up => self.view_up(),
            Action::Down => self.view_down(),
            Action::Left | Action::Right => {}
            // v0.2 keymaps — accepted by the translator but no behavior yet.
            Action::Enter | Action::Escape | Action::Search => {}
            Action::Filter => {
                if matches!(self.view, ViewId::Logs) {
                    self.logs.cycle_filter();
                }
            }
            Action::SubjectKindTask => {
                self.subjects.set_kind("task");
                self.refresh_now().await;
            }
            Action::SubjectKindRequirement => {
                self.subjects.set_kind("requirement");
                self.refresh_now().await;
            }
            Action::Refresh => self.refresh_now().await,
            Action::None => {}
        }
    }

    fn view_up(&mut self) {
        match self.view {
            ViewId::Workflows => self.workflows.up(),
            ViewId::Subjects => self.subjects.up(),
            ViewId::Queue => self.queue.up(),
            ViewId::Plugins => self.plugins.up(),
            _ => {}
        }
    }

    fn view_down(&mut self) {
        match self.view {
            ViewId::Workflows => self.workflows.down(),
            ViewId::Subjects => self.subjects.down(),
            ViewId::Queue => self.queue.down(),
            ViewId::Plugins => self.plugins.down(),
            _ => {}
        }
    }

    pub fn render(&self, f: &mut Frame) {
        let theme = Theme::current();
        let area = f.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(area);

        // Header
        let mut header_state = self.header.clone();
        header_state.active_view = self.view.idx();
        render_sticky_header(f, chunks[0], &header_state);

        // Body
        match self.view {
            ViewId::Workflows => self.workflows.render(f, chunks[1]),
            ViewId::Subjects => self.subjects.render(f, chunks[1]),
            ViewId::Queue => self.queue.render(f, chunks[1]),
            ViewId::Health => self.health.render(f, chunks[1]),
            ViewId::Logs => self.logs.render(f, chunks[1]),
            ViewId::Cost => self.cost.render(f, chunks[1]),
            ViewId::Plugins => self.plugins.render(f, chunks[1]),
        }

        // Footer
        let footer = Paragraph::new(Line::from(
            "1-7 view | hjkl move | r refresh | f filter (logs) | tk/tr subject kind | ? help | q quit",
        ))
        .style(ratatui::style::Style::default().fg(theme.fg_dim));
        f.render_widget(footer, chunks[2]);

        if self.awaiting_t_prefix {
            let prompt = Paragraph::new(Line::from("t…  (k=task, r=requirement)"))
                .block(Block::default().borders(Borders::ALL).title(" prefix "));
            let popup = centered_rect(40, 3, area);
            f.render_widget(Clear, popup);
            f.render_widget(prompt, popup);
        }

        if self.help_open {
            let lines = vec![
                Line::from(" Animus TUI — help (v0.1) "),
                Line::from(""),
                Line::from(" 1..7      switch view"),
                Line::from(" h j k l   move (left/down/up/right)"),
                Line::from(" f         cycle severity filter (logs view)"),
                Line::from(" tk        subjects → task kind"),
                Line::from(" tr        subjects → requirement kind"),
                Line::from(" r         force refresh"),
                Line::from(" ?         this help (any key to close)"),
                Line::from(" q / Ctrl-C quit"),
                Line::from(""),
                Line::from(" v0.2 will add: Enter (detail pane), / (search), Esc (back)"),
            ];
            let help =
                Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" Help "));
            let popup = centered_rect(60, 16, area);
            f.render_widget(Clear, popup);
            f.render_widget(help, popup);
        }
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}
