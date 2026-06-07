use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::{mpsc, Mutex};

use animus_control_protocol::types::{
    DaemonHealthResponse, DaemonLogEntry, DaemonLogsRequest, DaemonStatusResponse,
    PluginListRequest, PluginListResponse, QueueListRequest, QueueListResponse, QueueStats,
    SubjectListRequest, SubjectListResponse, WorkflowListRequest, WorkflowListResponse,
};
use animus_control_protocol::ControlClient;

/// Compute the default daemon control socket path.
///
/// Mirrors `crates/orchestrator-daemon-runtime/src/control/server.rs::control_socket_path`
/// but without the full protocol dependency: prefer `~/.animus/<scope>/control.sock`,
/// fall back to `<project_root>/.animus/control.sock` if `$HOME` is unavailable.
pub fn default_socket_path(explicit_project_root: Option<&Path>) -> Result<PathBuf> {
    let project_root = match explicit_project_root {
        Some(p) => {
            // Normalize the override the same way the implicit cwd path
            // does: absolutize against cwd if relative, then walk up to the
            // git common dir so a subdirectory of a repo still hashes to
            // the repo root the daemon used.
            let cwd = std::env::current_dir().context("could not read cwd")?;
            let abs = if p.is_absolute() {
                p.to_path_buf()
            } else {
                cwd.join(p)
            };
            git_common_root(&abs).unwrap_or(abs)
        }
        None => resolve_project_root()?,
    };

    if let Some(home) = dirs::home_dir() {
        let ao = home.join(".animus");
        // Match the daemon's `scoped_state_root` precedence:
        //   1. hash-derived scope (covers fresh setups)
        //   2. existing scope whose `.project-root` marker canonicalizes to us
        //      (covers projects relocated on disk or originally created under
        //      a different sanitized basename)
        let scope = repository_scope(&project_root);
        let scoped = ao.join(&scope).join("control.sock");
        if scoped.exists() {
            return Ok(scoped);
        }
        if let Some(by_marker) = scope_by_project_root_marker(&ao, &project_root) {
            return Ok(by_marker.join("control.sock"));
        }
        return Ok(scoped);
    }
    Ok(project_root.join(".animus").join("control.sock"))
}

/// Walk `~/.animus/*/` looking for a scope the daemon would adopt for
/// `project_root`. The daemon's `scoped_state_root` adopts:
///
///   1. a scope whose `.project-root` marker canonicalizes to the same path
///      we're being asked about,
///   2. a scope whose `.project-root` marker points at a path that no
///      longer exists (the user moved the repo),
///   3. a scope whose `.git-origin` matches our remote and either has no
///      `.project-root` marker (legacy) or whose recorded path canonicalizes
///      to ours.
///
/// We match cases 1 and 2 directly. For case 3, we cross-check
/// `git remote get-url origin` against the scope's `.git-origin` file.
fn scope_by_project_root_marker(ao_root: &Path, project_root: &Path) -> Option<PathBuf> {
    let our_canonical = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    let our_origin = git_remote_origin(project_root);
    let entries = std::fs::read_dir(ao_root).ok()?;
    let mut origin_match: Option<PathBuf> = None;
    for e in entries.flatten() {
        let dir = e.path();
        if !dir.is_dir() {
            continue;
        }
        let marker = dir.join(".project-root");
        match std::fs::read_to_string(&marker) {
            Ok(recorded) => {
                let recorded_path = Path::new(recorded.trim());
                match recorded_path.canonicalize() {
                    Ok(rc) if rc == our_canonical => return Some(dir),
                    Ok(_) => continue, // belongs to a different live clone
                    Err(_) => {
                        // Recorded path no longer exists. Only reclaim
                        // this scope if its `.git-origin` matches ours —
                        // otherwise we risk pointing at an unrelated
                        // project's stale socket.
                        if let (Some(our), Ok(existing)) = (
                            our_origin.as_ref(),
                            std::fs::read_to_string(dir.join(".git-origin")),
                        ) {
                            if existing.trim() == our {
                                return Some(dir);
                            }
                        }
                        continue;
                    }
                }
            }
            Err(_) => {
                // Legacy/unmigrated scope: fall back to .git-origin match.
                if origin_match.is_none() {
                    if let Some(our) = &our_origin {
                        if let Ok(existing) = std::fs::read_to_string(dir.join(".git-origin")) {
                            if existing.trim() == our {
                                origin_match = Some(dir);
                            }
                        }
                    }
                }
            }
        }
    }
    origin_match
}

fn git_remote_origin(project_root: &Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(project_root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}

fn resolve_project_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("could not read cwd")?;
    if let Some(git_root) = git_common_root(&cwd) {
        return Ok(git_root);
    }
    Ok(cwd)
}

/// Mirror the daemon's `git rev-parse --git-common-dir`-based resolution
/// so linked worktrees scope to the main repo root rather than the
/// worktree path.
fn git_common_root(start: &Path) -> Option<PathBuf> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(start)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if raw.is_empty() {
        return None;
    }
    let git_dir = if Path::new(&raw).is_absolute() {
        PathBuf::from(&raw)
    } else {
        start.join(&raw)
    };
    let canonical_git_dir = git_dir.canonicalize().unwrap_or(git_dir);
    // Guard against submodule layouts where `--git-common-dir` returns
    // `.../.git/modules/<sub>`: only adopt the parent when the basename
    // is `.git`. Otherwise the project root is the working tree itself,
    // which the caller will fall back to via cwd.
    if canonical_git_dir.file_name()?.to_string_lossy() != ".git" {
        return None;
    }
    let root = canonical_git_dir.parent()?.to_path_buf();
    root.canonicalize().ok().or(Some(root))
}

/// Best-effort approximation of `protocol::repository_scope_for_path`:
/// `<basename>-<short-hash>`. If our slug doesn't match the actual daemon's
/// scope dir, the resulting socket path simply won't exist and the caller
/// shows the no-daemon splash — explicit `--daemon-socket` is the override.
pub fn scope_dir_for_project_root(home: &Path, project_root: &Path) -> PathBuf {
    home.join(".animus").join(repository_scope(project_root))
}

/// Match `protocol::repository_scope_for_path` byte-for-byte so the TUI
/// resolves the same `~/.animus/<scope>/` directory the daemon creates.
///
/// Algorithm (from `crates/protocol/src/repository_scope.rs`):
///   1. `canonical = path.canonicalize().unwrap_or(path.to_path_buf())`
///   2. `slug = sanitize_identifier(canonical.file_name(), "repo")`
///   3. `suffix = first 6 bytes of SHA-256(canonical.to_string_lossy())` (12 hex chars)
///   4. `format!("{slug}-{suffix}")`
fn repository_scope(project_root: &Path) -> String {
    let canonical = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    let canonical_display = canonical.to_string_lossy();
    let repo_name = canonical
        .file_name()
        .and_then(|v| v.to_str())
        .map(|s| sanitize_identifier(s, "repo"))
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "repo".to_string());

    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(canonical_display.as_bytes());
    let digest = hasher.finalize();
    let suffix = format!(
        "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5]
    );
    format!("{repo_name}-{suffix}")
}

/// Match `protocol::repository_scope::sanitize_identifier` byte-for-byte.
fn sanitize_identifier(value: &str, fallback: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut trailing_separator = false;
    for ch in value.chars() {
        match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => {
                out.push(ch.to_ascii_lowercase());
                trailing_separator = false;
            }
            ' ' | '_' | '-' if !out.is_empty() && !trailing_separator => {
                out.push('-');
                trailing_separator = true;
            }
            ' ' | '_' | '-' => {}
            _ => {}
        }
    }
    if trailing_separator {
        out.pop();
    }
    if out.is_empty() {
        fallback.to_string()
    } else {
        out
    }
}

/// Trait abstraction so tests can drive the App with an in-memory fake
/// client without spinning up a daemon. Only covers the methods used by
/// the views.
#[async_trait::async_trait]
pub trait ControlAccess: Send + Sync {
    async fn daemon_status(&self) -> Result<DaemonStatusResponse>;
    async fn daemon_health(&self) -> Result<DaemonHealthResponse>;
    async fn workflow_list(&self, request: WorkflowListRequest) -> Result<WorkflowListResponse>;
    async fn subject_list(&self, request: SubjectListRequest) -> Result<SubjectListResponse>;
    async fn queue_list(&self, request: QueueListRequest) -> Result<QueueListResponse>;
    async fn queue_stats(&self) -> Result<QueueStats> {
        Ok(QueueStats {
            ready: 0,
            held: 0,
            in_flight: 0,
            done_recent: 0,
            dropped_recent: 0,
        })
    }
    async fn plugin_list(&self, request: PluginListRequest) -> Result<PluginListResponse>;

    /// Spawn the daemon log tail subscription (if supported). Returns an
    /// mpsc receiver the App polls on every tick. Implementations that
    /// can't subscribe (fakes, dead sockets) return an immediately-closed
    /// receiver.
    async fn spawn_log_stream(&self) -> mpsc::Receiver<DaemonLogEntry> {
        let (_tx, rx) = mpsc::channel(1);
        rx
    }
}

pub struct RealControlClient {
    inner: Arc<Mutex<ControlClient>>,
}

impl RealControlClient {
    pub async fn connect(socket: &Path) -> Result<Self> {
        let inner = ControlClient::connect(socket)
            .await
            .with_context(|| format!("connect to daemon socket {}", socket.display()))?;
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }
}

#[async_trait::async_trait]
impl ControlAccess for RealControlClient {
    async fn daemon_status(&self) -> Result<DaemonStatusResponse> {
        let g = self.inner.lock().await;
        g.daemon_status().await
    }
    async fn daemon_health(&self) -> Result<DaemonHealthResponse> {
        let g = self.inner.lock().await;
        g.daemon_health().await
    }
    async fn workflow_list(&self, request: WorkflowListRequest) -> Result<WorkflowListResponse> {
        let g = self.inner.lock().await;
        g.workflow_list(request).await
    }
    async fn subject_list(&self, request: SubjectListRequest) -> Result<SubjectListResponse> {
        let g = self.inner.lock().await;
        g.subject_list(request).await
    }
    async fn queue_list(&self, request: QueueListRequest) -> Result<QueueListResponse> {
        let g = self.inner.lock().await;
        g.queue_list(request).await
    }
    async fn queue_stats(&self) -> Result<QueueStats> {
        let g = self.inner.lock().await;
        g.queue_stats().await
    }
    async fn plugin_list(&self, request: PluginListRequest) -> Result<PluginListResponse> {
        let g = self.inner.lock().await;
        g.plugin_list(request).await
    }

    async fn spawn_log_stream(&self) -> mpsc::Receiver<DaemonLogEntry> {
        let (tx, rx) = mpsc::channel::<DaemonLogEntry>(256);
        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move {
            let g = inner.lock().await;
            // Cap the historical replay so we don't drag the whole log
            // archive across the wire on startup; the live tail then flows
            // in real time.
            let since = Some(chrono::Utc::now() - chrono::Duration::minutes(5));
            let sub_res = g
                .daemon_logs_follow(DaemonLogsRequest {
                    since,
                    level: None,
                    plugin: None,
                    follow: true,
                })
                .await;
            drop(g);
            let mut sub = match sub_res {
                Ok(s) => s,
                Err(_) => return,
            };
            while let Some(entry) = sub.recv().await {
                if tx.send(entry).await.is_err() {
                    break;
                }
            }
        });
        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_identifier_matches_protocol_examples() {
        // Mirrors the assertions in `protocol::repository_scope::tests`.
        assert_eq!(sanitize_identifier("Repo Name", "repo"), "repo-name");
        assert_eq!(sanitize_identifier("___", "repo"), "repo");
        assert_eq!(sanitize_identifier("A__B--C", "repo"), "a-b-c");
        assert_eq!(
            sanitize_identifier("  __My Repo!! -- 2026__  ", "repo"),
            "my-repo-2026"
        );
        assert_eq!(sanitize_identifier("日本語", "repo"), "repo");
    }

    #[test]
    fn repository_scope_has_slug_and_12_hex_suffix() {
        let tmp = std::env::temp_dir();
        let scope = repository_scope(tmp.as_path());
        let (slug, suffix) = scope.rsplit_once('-').expect("scope has hyphen");
        assert!(!slug.is_empty());
        assert_eq!(suffix.len(), 12);
        assert!(suffix.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
