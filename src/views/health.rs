use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use animus_control_protocol::types::{
    DaemonHealthResponse, DaemonHealthStatus, DaemonStatusResponse,
};

use crate::theme::Theme;

#[derive(Default)]
pub struct HealthView {
    pub status: Option<DaemonStatusResponse>,
    pub health: Option<DaemonHealthResponse>,
    pub error: Option<String>,
}

impl HealthView {
    pub fn set(&mut self, status: DaemonStatusResponse, health: DaemonHealthResponse) {
        self.status = Some(status);
        self.health = Some(health);
        self.error = None;
    }
    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.error = Some(msg.into());
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let theme = Theme::current();
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Daemon health ");
        if let Some(err) = &self.error {
            let p = Paragraph::new(Line::from(vec![Span::styled(
                format!("error: {err}"),
                Style::default().fg(theme.bad),
            )]))
            .block(block);
            f.render_widget(p, area);
            return;
        }

        let mut lines: Vec<Line> = Vec::new();

        if let Some(s) = &self.status {
            let pid = s.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into());
            let uptime = s
                .uptime_seconds
                .map(humanize_uptime)
                .unwrap_or_else(|| "-".into());
            let ver = s.version.clone().unwrap_or_else(|| "-".into());
            let log = s
                .log_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "-".into());
            lines.push(Line::from(vec![
                Span::styled("PID:     ", Style::default().fg(theme.fg_dim)),
                Span::raw(pid),
            ]));
            lines.push(Line::from(vec![
                Span::styled("uptime:  ", Style::default().fg(theme.fg_dim)),
                Span::raw(uptime),
            ]));
            lines.push(Line::from(vec![
                Span::styled("version: ", Style::default().fg(theme.fg_dim)),
                Span::raw(ver),
            ]));
            lines.push(Line::from(vec![
                Span::styled("log:     ", Style::default().fg(theme.fg_dim)),
                Span::raw(log),
            ]));
        }

        if let Some(h) = &self.health {
            let (label, color) = match h.status {
                DaemonHealthStatus::Healthy => ("healthy", theme.good),
                DaemonHealthStatus::Degraded => ("degraded", theme.warn),
                DaemonHealthStatus::Unhealthy => ("unhealthy", theme.bad),
                DaemonHealthStatus::Down => ("down", theme.bad),
            };
            lines.push(Line::from(vec![
                Span::styled("health:  ", Style::default().fg(theme.fg_dim)),
                Span::styled(label, Style::default().fg(color)),
            ]));
            if let Some(err) = &h.last_error {
                lines.push(Line::from(vec![
                    Span::styled("last err: ", Style::default().fg(theme.fg_dim)),
                    Span::styled(err.as_str(), Style::default().fg(theme.bad)),
                ]));
            }
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![Span::styled(
                "Plugins",
                Style::default().fg(theme.accent),
            )]));
            for p in &h.plugins {
                let (label, color) = match p.status {
                    DaemonHealthStatus::Healthy => ("healthy", theme.good),
                    DaemonHealthStatus::Degraded => ("degraded", theme.warn),
                    DaemonHealthStatus::Unhealthy => ("unhealthy", theme.bad),
                    DaemonHealthStatus::Down => ("down", theme.bad),
                };
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::raw(p.name.clone()),
                    Span::raw("  "),
                    Span::styled(p.kind.clone(), Style::default().fg(theme.fg_dim)),
                    Span::raw("  "),
                    Span::styled(label, Style::default().fg(color)),
                ]));
            }
        }

        let para = Paragraph::new(lines).block(block);
        f.render_widget(para, area);
    }
}

fn humanize_uptime(seconds: u64) -> String {
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else if seconds < 86400 {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    } else {
        format!("{}d {}h", seconds / 86400, (seconds % 86400) / 3600)
    }
}
