use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use animus_subject_protocol::{Subject, SubjectStatus};

use crate::theme::Theme;

pub struct SubjectsView {
    pub subjects: Vec<Subject>,
    pub selected: usize,
    pub kind: String, // "task" or "requirement"
    pub error: Option<String>,
}

impl Default for SubjectsView {
    fn default() -> Self {
        Self {
            subjects: Vec::new(),
            selected: 0,
            kind: "task".to_string(),
            error: None,
        }
    }
}

impl SubjectsView {
    pub fn set(&mut self, subjects: Vec<Subject>) {
        if self.selected >= subjects.len() && !subjects.is_empty() {
            self.selected = subjects.len() - 1;
        }
        self.subjects = subjects;
        self.error = None;
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.error = Some(msg.into());
    }

    pub fn set_kind(&mut self, k: &str) {
        self.kind = k.to_string();
        self.selected = 0;
    }

    pub fn up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }
    pub fn down(&mut self) {
        if self.selected + 1 < self.subjects.len() {
            self.selected += 1;
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let theme = Theme::current();
        let title = format!(" Subjects (kind={})  tk=task tr=requirement ", self.kind);
        let block = Block::default().borders(Borders::ALL).title(title);
        if let Some(err) = &self.error {
            let p = Paragraph::new(Line::from(vec![Span::styled(
                format!("error: {err}"),
                Style::default().fg(theme.bad),
            )]))
            .block(block);
            f.render_widget(p, area);
            return;
        }
        if self.subjects.is_empty() {
            let p = Paragraph::new(Line::from(vec![Span::styled(
                "no subjects",
                Style::default().fg(theme.fg_dim),
            )]))
            .block(block);
            f.render_widget(p, area);
            return;
        }
        let header =
            Row::new(vec!["id", "title", "status", "priority", "assignee"]).style(theme.header());
        let rows: Vec<Row> = self
            .subjects
            .iter()
            .map(|s| {
                let status_text = format!("{:?}", s.status).to_lowercase();
                let color = status_color(&theme, s.status);
                Row::new(vec![
                    s.id.as_str().to_string(),
                    truncate(&s.title, 40),
                    status_text,
                    s.priority
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                    s.assignee.clone().unwrap_or_else(|| "-".to_string()),
                ])
                .style(Style::default().fg(color))
            })
            .collect();
        let widths = [
            ratatui::layout::Constraint::Length(28),
            ratatui::layout::Constraint::Length(40),
            ratatui::layout::Constraint::Length(12),
            ratatui::layout::Constraint::Length(8),
            ratatui::layout::Constraint::Length(20),
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

fn status_color(theme: &Theme, status: SubjectStatus) -> ratatui::style::Color {
    match status {
        SubjectStatus::Done => theme.good,
        SubjectStatus::Blocked | SubjectStatus::Cancelled => theme.bad,
        SubjectStatus::InProgress | SubjectStatus::Ready => theme.fg,
    }
}
