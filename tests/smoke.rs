use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Result;
use chrono::{TimeZone, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tokio::sync::Mutex as TokioMutex;

use animus_control_protocol::types::{
    DaemonHealthResponse, DaemonHealthStatus, DaemonStatusResponse, PluginInfo, PluginListRequest,
    PluginListResponse, QueueEntry, QueueEntryStatus, QueueListRequest, QueueListResponse,
    SubjectListRequest, SubjectListResponse, WorkflowListRequest, WorkflowListResponse,
    WorkflowRunSummary, WorkflowStatus,
};
use animus_subject_protocol::{Subject, SubjectId, SubjectStatus};

use animus_tui::control_client::ControlAccess;
use animus_tui::App;

#[derive(Default, Clone)]
struct FakeClient {
    pub calls: Arc<TokioMutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl ControlAccess for FakeClient {
    async fn daemon_status(&self) -> Result<DaemonStatusResponse> {
        self.calls.lock().await.push("daemon_status".into());
        Ok(DaemonStatusResponse {
            running: true,
            pid: Some(4242),
            uptime_seconds: Some(3661),
            version: Some("0.5.6".into()),
            project_root: Some("/tmp/fake".into()),
            log_path: None,
        })
    }
    async fn daemon_health(&self) -> Result<DaemonHealthResponse> {
        self.calls.lock().await.push("daemon_health".into());
        Ok(DaemonHealthResponse {
            status: DaemonHealthStatus::Healthy,
            plugins: vec![],
            last_error: None,
        })
    }
    async fn workflow_list(&self, _req: WorkflowListRequest) -> Result<WorkflowListResponse> {
        self.calls.lock().await.push("workflow_list".into());
        let started = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
        Ok(WorkflowListResponse {
            runs: vec![WorkflowRunSummary {
                id: "wf-0001".into(),
                definition: "animus.task/standard".into(),
                status: WorkflowStatus::Running,
                subject_id: Some(SubjectId::new("task:TASK-1")),
                started_at: started,
                finished_at: None,
            }],
            next_cursor: None,
        })
    }
    async fn subject_list(&self, _req: SubjectListRequest) -> Result<SubjectListResponse> {
        self.calls.lock().await.push("subject_list".into());
        let now = Utc::now();
        Ok(SubjectListResponse {
            subjects: vec![Subject {
                id: SubjectId::new("task:TASK-1"),
                kind: "task".into(),
                title: "first task".into(),
                description: None,
                status: SubjectStatus::InProgress,
                priority: Some(2),
                assignee: Some("agent:claude".into()),
                labels: vec![],
                parent: None,
                children: vec![],
                url: None,
                created_at: now,
                updated_at: now,
                custom: BTreeMap::new(),
            }],
            next_cursor: None,
            fetched_at: now,
        })
    }
    async fn queue_list(&self, _req: QueueListRequest) -> Result<QueueListResponse> {
        self.calls.lock().await.push("queue_list".into());
        let now = Utc::now();
        Ok(QueueListResponse {
            entries: vec![QueueEntry {
                id: "q-0001".into(),
                subject_id: SubjectId::new("task:TASK-1"),
                status: QueueEntryStatus::Ready,
                priority: 2,
                enqueued_at: now,
                hold_reason: None,
            }],
            next_cursor: None,
        })
    }
    async fn plugin_list(&self, _req: PluginListRequest) -> Result<PluginListResponse> {
        self.calls.lock().await.push("plugin_list".into());
        Ok(PluginListResponse {
            plugins: vec![PluginInfo {
                name: "animus-provider-claude".into(),
                version: "0.2.2".into(),
                kind: "provider".into(),
                source: Some("launchapp-dev/animus-provider-claude".into()),
                signature_verified: false,
                description: None,
                binary_path: None,
            }],
            warnings: vec![],
        })
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

#[tokio::test]
async fn renders_each_view_without_panic() {
    let fake = FakeClient::default();
    let mut app = App::new(Box::new(fake.clone()));
    app.bootstrap().await;

    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    for view_key in ['1', '2', '3', '4', '5', '6', '7'] {
        app.on_key(key(KeyCode::Char(view_key))).await;
        terminal.draw(|f| app.render(f)).unwrap();
    }

    let calls = fake.calls.lock().await.clone();
    assert!(calls.contains(&"daemon_status".to_string()));
    assert!(calls.contains(&"workflow_list".to_string()));
    assert!(calls.contains(&"subject_list".to_string()));
    assert!(calls.contains(&"queue_list".to_string()));
    assert!(calls.contains(&"plugin_list".to_string()));
}

#[tokio::test]
async fn help_overlay_opens_and_closes() {
    let fake = FakeClient::default();
    let mut app = App::new(Box::new(fake));
    app.bootstrap().await;

    assert!(!app.help_open);
    app.on_key(key(KeyCode::Char('?'))).await;
    assert!(app.help_open);
    // Any key closes it.
    app.on_key(key(KeyCode::Char('a'))).await;
    assert!(!app.help_open);
}

#[tokio::test]
async fn t_prefix_switches_subject_kind() {
    let fake = FakeClient::default();
    let mut app = App::new(Box::new(fake));
    app.bootstrap().await;

    assert_eq!(app.subjects.kind, "task");
    app.on_key(key(KeyCode::Char('t'))).await;
    assert!(app.awaiting_t_prefix);
    app.on_key(key(KeyCode::Char('r'))).await;
    assert_eq!(app.subjects.kind, "requirement");
}

#[tokio::test]
async fn quit_key_sets_quit_flag() {
    let fake = FakeClient::default();
    let mut app = App::new(Box::new(fake));
    app.bootstrap().await;
    assert!(!app.should_quit());
    app.on_key(key(KeyCode::Char('q'))).await;
    assert!(app.should_quit());
}

#[test]
fn plugin_manifest_is_valid_json_with_required_fields() {
    let v: serde_json::Value =
        serde_json::from_str(animus_tui::PLUGIN_MANIFEST_JSON).expect("valid manifest JSON");
    assert_eq!(v["name"], "animus-tui");
    assert_eq!(v["plugin_kind"], "custom");
    assert!(v["version"].as_str().is_some());
    assert!(v["protocol_version"].as_str().is_some());
    assert!(v["capabilities"].is_array());
    assert!(v["env_required"].is_array());
}

#[tokio::test]
async fn cycle_log_filter() {
    let fake = FakeClient::default();
    let mut app = App::new(Box::new(fake));
    app.bootstrap().await;
    app.on_key(key(KeyCode::Char('5'))).await; // logs view
    assert!(app.logs.min_severity.is_none());
    app.on_key(key(KeyCode::Char('f'))).await;
    assert!(app.logs.min_severity.is_some());
}
