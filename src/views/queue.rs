use chrono::Utc;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use animus_control_protocol::types::{QueueEntry, QueueEntryStatus};

use crate::theme::Theme;

#[derive(Default)]
pub struct QueueView {
    pub entries: Vec<QueueEntry>,
    pub selected: usize,
    pub error: Option<String>,
}

impl QueueView {
    pub fn set(&mut self, entries: Vec<QueueEntry>) {
        if self.selected >= entries.len() && !entries.is_empty() {
            self.selected = entries.len() - 1;
        }
        self.entries = entries;
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
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let theme = Theme::current();
        let block = Block::default().borders(Borders::ALL).title(" Queue ");
        if let Some(err) = &self.error {
            let p = Paragraph::new(Line::from(vec![Span::styled(
                format!("error: {err}"),
                Style::default().fg(theme.bad),
            )]))
            .block(block);
            f.render_widget(p, area);
            return;
        }
        if self.entries.is_empty() {
            let p = Paragraph::new(Line::from(vec![Span::styled(
                "queue empty",
                Style::default().fg(theme.fg_dim),
            )]))
            .block(block);
            f.render_widget(p, area);
            return;
        }
        let header = Row::new(vec!["id", "subject", "status", "prio", "age"]).style(theme.header());
        let now = Utc::now();
        let rows: Vec<Row> = self
            .entries
            .iter()
            .map(|e| {
                let status_text = format!("{:?}", e.status).to_lowercase();
                let color = status_color(&theme, e.status);
                let age = humanize_age(now - e.enqueued_at);
                Row::new(vec![
                    truncate(&e.id, 14),
                    e.subject_id.as_str().to_string(),
                    status_text,
                    e.priority.to_string(),
                    age,
                ])
                .style(Style::default().fg(color))
            })
            .collect();
        let widths = [
            ratatui::layout::Constraint::Length(16),
            ratatui::layout::Constraint::Length(28),
            ratatui::layout::Constraint::Length(10),
            ratatui::layout::Constraint::Length(6),
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

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let trunc: String = s.chars().take(n.saturating_sub(1)).collect();
        format!("{trunc}…")
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

fn status_color(theme: &Theme, s: QueueEntryStatus) -> ratatui::style::Color {
    match s {
        QueueEntryStatus::Done => theme.good,
        QueueEntryStatus::Held => theme.warn,
        QueueEntryStatus::Dropped => theme.bad,
        QueueEntryStatus::Ready | QueueEntryStatus::InFlight => theme.fg,
    }
}
