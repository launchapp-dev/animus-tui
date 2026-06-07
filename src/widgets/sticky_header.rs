use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::theme::Theme;
use animus_control_protocol::types::DaemonHealthStatus;

#[derive(Debug, Clone, Default)]
pub struct StickyHeaderState {
    pub daemon_connected: bool,
    pub daemon_health: Option<DaemonHealthStatus>,
    pub active_workflows: usize,
    pub queue_depth: usize,
    pub top_spend_workflow: Option<(String, f64)>,
    pub active_view: usize,
}

pub fn render_sticky_header(f: &mut Frame, area: Rect, state: &StickyHeaderState) {
    let theme = Theme::current();

    let dot = if !state.daemon_connected {
        ("⬤ disconnected", theme.bad)
    } else {
        match state.daemon_health {
            Some(DaemonHealthStatus::Healthy) => ("⬤ healthy", theme.good),
            Some(DaemonHealthStatus::Degraded) => ("⬤ degraded", theme.warn),
            Some(DaemonHealthStatus::Unhealthy) => ("⬤ unhealthy", theme.bad),
            Some(DaemonHealthStatus::Down) => ("⬤ down", theme.bad),
            None => ("⬤ unknown", theme.fg_dim),
        }
    };

    let view_names = [
        "1:wf",
        "2:subj",
        "3:queue",
        "4:daemon",
        "5:logs",
        "6:cost",
        "7:plugins",
    ];

    let mut spans = vec![
        Span::styled("animus-tui ", Style::default().fg(theme.accent)),
        Span::styled(dot.0, Style::default().fg(dot.1)),
        Span::raw("  "),
        Span::styled(
            format!("wf:{}", state.active_workflows),
            Style::default().fg(theme.fg),
        ),
        Span::raw("  "),
        Span::styled(
            format!("q:{}", state.queue_depth),
            Style::default().fg(theme.fg),
        ),
        Span::raw("  "),
    ];

    match &state.top_spend_workflow {
        Some((id, cost)) => spans.push(Span::styled(
            format!("top$: {} ${:.2}", id, cost),
            Style::default().fg(theme.fg_dim),
        )),
        None => spans.push(Span::styled("top$: --", Style::default().fg(theme.fg_dim))),
    };

    let line1 = Line::from(spans);

    let mut view_spans: Vec<Span> = Vec::with_capacity(view_names.len() * 2);
    for (idx, name) in view_names.iter().enumerate() {
        let style = if idx == state.active_view {
            Style::default()
                .fg(theme.accent)
                .add_modifier(ratatui::style::Modifier::BOLD)
        } else {
            Style::default().fg(theme.fg_dim)
        };
        view_spans.push(Span::styled(*name, style));
        if idx + 1 < view_names.len() {
            view_spans.push(Span::raw(" "));
        }
    }
    let line2 = Line::from(view_spans);

    let para = Paragraph::new(vec![line1, line2]).style(theme.header());
    f.render_widget(para, area);
}
