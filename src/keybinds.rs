use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    Help,
    Switch(usize),
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
    Search,
    Filter,
    SubjectKindTask,
    SubjectKindRequirement,
    Refresh,
    None,
}

pub fn translate(key: KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Action::Quit;
    }
    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('?') => Action::Help,
        KeyCode::Char(c @ '1'..='7') => Action::Switch((c as u8 - b'1') as usize),
        KeyCode::Char('h') | KeyCode::Left => Action::Left,
        KeyCode::Char('j') | KeyCode::Down => Action::Down,
        KeyCode::Char('k') | KeyCode::Up => Action::Up,
        KeyCode::Char('l') | KeyCode::Right => Action::Right,
        // Enter / Esc / / are reserved for v0.2 (detail pane, back, search);
        // ignore them in v0.1 to avoid silent no-ops.
        KeyCode::Char('f') => Action::Filter,
        KeyCode::Char('r') => Action::Refresh,
        // `tk` / `tr` two-key prefix is recognized at the App layer with a
        // pending-key buffer; here we surface the second keystroke as a
        // direct action so the App layer can collapse on the prefix.
        KeyCode::Char('t') => Action::None, // App tracks the prefix
        _ => Action::None,
    }
}

/// Two-key prefix translation used by the subjects view.
pub fn translate_after_t(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('k') => Some(Action::SubjectKindTask),
        KeyCode::Char('r') => Some(Action::SubjectKindRequirement),
        _ => None,
    }
}
