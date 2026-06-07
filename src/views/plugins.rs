use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};
use ratatui::Frame;

use animus_control_protocol::types::PluginInfo;

use crate::theme::Theme;

#[derive(Default)]
pub struct PluginsView {
    pub plugins: Vec<PluginInfo>,
    pub selected: usize,
    pub error: Option<String>,
}

impl PluginsView {
    pub fn set(&mut self, plugins: Vec<PluginInfo>) {
        if self.selected >= plugins.len() && !plugins.is_empty() {
            self.selected = plugins.len() - 1;
        }
        self.plugins = plugins;
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
        if self.selected + 1 < self.plugins.len() {
            self.selected += 1;
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let theme = Theme::current();
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Plugins  (read-only in v0.1) ");
        if let Some(err) = &self.error {
            let p = Paragraph::new(Line::from(vec![Span::styled(
                format!("error: {err}"),
                Style::default().fg(theme.bad),
            )]))
            .block(block);
            f.render_widget(p, area);
            return;
        }
        if self.plugins.is_empty() {
            let p = Paragraph::new(Line::from(vec![Span::styled(
                "no plugins installed",
                Style::default().fg(theme.fg_dim),
            )]))
            .block(block);
            f.render_widget(p, area);
            return;
        }
        let header = Row::new(vec!["name", "kind", "version", "source"]).style(theme.header());
        let rows: Vec<Row> = self
            .plugins
            .iter()
            .map(|p| {
                Row::new(vec![
                    p.name.clone(),
                    p.kind.clone(),
                    p.version.clone(),
                    p.source.clone().unwrap_or_else(|| "-".to_string()),
                ])
            })
            .collect();
        let widths = [
            ratatui::layout::Constraint::Length(36),
            ratatui::layout::Constraint::Length(18),
            ratatui::layout::Constraint::Length(10),
            ratatui::layout::Constraint::Length(40),
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
