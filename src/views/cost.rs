use std::path::{Path, PathBuf};

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use serde::Deserialize;

use crate::theme::Theme;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CostState {
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(default)]
    pub total_cost_usd: f64,
    #[serde(default)]
    pub workflows: Vec<CostWorkflowEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CostWorkflowEntry {
    pub workflow_id: String,
    #[serde(default)]
    pub tokens: u64,
    #[serde(default)]
    pub cost_usd: f64,
}

#[derive(Default)]
pub struct CostView {
    pub state: Option<CostState>,
    pub error: Option<String>,
    pub source_path: Option<PathBuf>,
}

impl CostView {
    pub fn load(&mut self, scoped_state_root: &Path) {
        let path = scoped_state_root.join("cost-state.v1.json");
        self.source_path = Some(path.clone());
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<CostState>(&text) {
                Ok(parsed) => {
                    self.state = Some(parsed);
                    self.error = None;
                }
                Err(e) => self.error = Some(format!("parse cost state: {e}")),
            },
            Err(_) => {
                self.state = None;
                self.error = None;
            }
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let theme = Theme::current();
        let block = Block::default().borders(Borders::ALL).title(" Cost ");
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
        match &self.state {
            None => {
                lines.push(Line::from(vec![Span::styled(
                    "no cost data",
                    Style::default().fg(theme.fg_dim),
                )]));
                if let Some(p) = &self.source_path {
                    lines.push(Line::from(vec![
                        Span::styled("source:  ", Style::default().fg(theme.fg_dim)),
                        Span::raw(p.display().to_string()),
                    ]));
                }
                lines.push(Line::from(vec![Span::styled(
                    "(this file is populated by the daemon's budget-caps feature, landed in v0.5.5)",
                    Style::default().fg(theme.fg_dim),
                )]));
            }
            Some(s) => {
                lines.push(Line::from(vec![
                    Span::styled("total tokens: ", Style::default().fg(theme.fg_dim)),
                    Span::raw(format_thousands(s.total_tokens)),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("total USD:    ", Style::default().fg(theme.fg_dim)),
                    Span::raw(format!("${:.4}", s.total_cost_usd)),
                ]));
                lines.push(Line::raw(""));
                lines.push(Line::from(vec![Span::styled(
                    "Per workflow",
                    Style::default().fg(theme.accent),
                )]));
                let mut sorted = s.workflows.clone();
                sorted.sort_by(|a, b| {
                    b.cost_usd
                        .partial_cmp(&a.cost_usd)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                for w in sorted.iter().take(40) {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::raw(format!("{:<32}", truncate(&w.workflow_id, 32))),
                        Span::raw(format!("{:>10}  ", format_thousands(w.tokens))),
                        Span::raw(format!("${:>8.4}", w.cost_usd)),
                    ]));
                }
            }
        }
        let para = Paragraph::new(lines).block(block);
        f.render_widget(para, area);
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

fn format_thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}
