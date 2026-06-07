use std::collections::VecDeque;

use animus_control_protocol::types::DaemonLogEntry;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::theme::Theme;

const MAX_LINES: usize = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub severity: Severity,
    pub timestamp: String,
    pub message: String,
}

#[derive(Default)]
pub struct LogsView {
    pub lines: VecDeque<LogEntry>,
    pub min_severity: Option<Severity>,
    pub dropped: usize,
    pub error: Option<String>,
}

impl LogsView {
    pub fn push(&mut self, entry: LogEntry) {
        if self.lines.len() >= MAX_LINES {
            self.lines.pop_front();
            self.dropped += 1;
        }
        self.lines.push_back(entry);
    }

    /// Convert a daemon-side log entry into the local view shape and push it.
    pub fn ingest(&mut self, entry: DaemonLogEntry) {
        use animus_log_storage_protocol::LogLevel;
        let severity = match entry.level {
            LogLevel::Error => Severity::Error,
            LogLevel::Warn => Severity::Warn,
            LogLevel::Info | LogLevel::Debug | LogLevel::Trace => Severity::Info,
        };
        let timestamp = entry.ts.format("%H:%M:%S").to_string();
        self.push(LogEntry {
            severity,
            timestamp,
            message: entry.message,
        });
    }
    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.error = Some(msg.into());
    }
    pub fn cycle_filter(&mut self) {
        self.min_severity = match self.min_severity {
            None => Some(Severity::Info),
            Some(Severity::Info) => Some(Severity::Warn),
            Some(Severity::Warn) => Some(Severity::Error),
            Some(Severity::Error) => None,
        };
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let theme = Theme::current();
        let title = format!(
            " Logs  (filter: {})  dropped: {} ",
            match self.min_severity {
                None => "all",
                Some(Severity::Info) => "info+",
                Some(Severity::Warn) => "warn+",
                Some(Severity::Error) => "error",
            },
            self.dropped
        );
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

        // Two block borders + a margin row, so the inner area is
        // (height - 2) at minimum. Render only the last N filtered entries
        // so the view behaves like a tail rather than freezing on the
        // oldest buffered line.
        let visible_rows = area.height.saturating_sub(2) as usize;
        let filtered: Vec<&LogEntry> = self
            .lines
            .iter()
            .filter(|e| match self.min_severity {
                None => true,
                Some(s) => severity_rank(e.severity) >= severity_rank(s),
            })
            .collect();
        let start = filtered.len().saturating_sub(visible_rows.max(1));
        let visible: Vec<Line> = filtered[start..]
            .iter()
            .map(|e| {
                let color = match e.severity {
                    Severity::Info => theme.fg,
                    Severity::Warn => theme.warn,
                    Severity::Error => theme.bad,
                };
                Line::from(vec![
                    Span::styled(e.timestamp.clone(), Style::default().fg(theme.fg_dim)),
                    Span::raw("  "),
                    Span::styled(e.message.clone(), Style::default().fg(color)),
                ])
            })
            .collect();

        let para = Paragraph::new(visible).block(block);
        f.render_widget(para, area);
    }
}

fn severity_rank(s: Severity) -> u8 {
    match s {
        Severity::Info => 0,
        Severity::Warn => 1,
        Severity::Error => 2,
    }
}
