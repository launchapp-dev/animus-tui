use chrono::Utc;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use animus_control_protocol::types::{WorkflowRunSummary, WorkflowStatus};

use crate::theme::Theme;

#[derive(Default)]
pub struct WorkflowsView {
    pub runs: Vec<WorkflowRunSummary>,
    pub selected: usize,
    pub error: Option<String>,
}

impl WorkflowsView {
    pub fn set(&mut self, runs: Vec<WorkflowRunSummary>) {
        if self.selected >= runs.len() && !runs.is_empty() {
            self.selected = runs.len() - 1;
        }
        self.runs = runs;
        self.error = None;
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.error = Some(msg.into());
    }

    pub fn up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn down(&mut self) {
        if self.selected + 1 < self.runs.len() {
            self.selected += 1;
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let theme = Theme::current();
        let block = Block::default().borders(Borders::ALL).title(" Workflows ");
        if let Some(err) = &self.error {
            let p = Paragraph::new(Line::from(vec![Span::styled(
                format!("error: {err}"),
                Style::default().fg(theme.bad),
            )]))
            .block(block);
            f.render_widget(p, area);
            return;
        }
        if self.runs.is_empty() {
            let p = Paragraph::new(Line::from(vec![Span::styled(
                "no workflow runs",
                Style::default().fg(theme.fg_dim),
            )]))
            .block(block);
            f.render_widget(p, area);
            return;
        }

        let header =
            Row::new(vec!["id", "definition", "status", "subject", "age"]).style(theme.header());
        let now = Utc::now();
        let rows: Vec<Row> = self
            .runs
            .iter()
            .map(|r| {
                let status_text = format!("{:?}", r.status).to_lowercase();
                let status_color = status_color(&theme, r.status);
                let age = humanize_age(now - r.started_at);
                Row::new(vec![
                    short_id(&r.id, 14),
                    r.definition.clone(),
                    status_text,
                    r.subject_id
                        .as_ref()
                        .map(|s| s.as_str().to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    age,
                ])
                .style(Style::default().fg(status_color))
            })
            .collect();
        let widths = [
            ratatui::layout::Constraint::Length(16),
            ratatui::layout::Constraint::Length(24),
            ratatui::layout::Constraint::Length(10),
            ratatui::layout::Constraint::Length(20),
            ratatui::layout::Constraint::Length(8),
        ];
        let table = Table::new(rows, widths)
            .header(header)
            .block(block)
            .row_highlight_style(theme.selected_row());
        let mut state = TableState::default();
        state.select(Some(self.selected));
        f.render_stateful_widget(table, area, &mut state);
    }
}

fn short_id(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let trunc: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{trunc}…")
    }
}

fn status_color(theme: &Theme, status: WorkflowStatus) -> ratatui::style::Color {
    match status {
        WorkflowStatus::Completed => theme.good,
        WorkflowStatus::Running | WorkflowStatus::Pending | WorkflowStatus::Paused => theme.fg,
        WorkflowStatus::Failed | WorkflowStatus::Cancelled => theme.bad,
    }
}

fn humanize_age(d: chrono::Duration) -> String {
    let secs = d.num_seconds().max(0);
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}
