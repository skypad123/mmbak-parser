use ratatui::crossterm::event::KeyCode;

/// Actions the user can trigger via keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Up,
    Down,
    SwitchPanel,
    Select,
    Deselect,
    Quit,
}

/// Map a crossterm key code to an application action.
pub fn map_key(code: KeyCode) -> Option<Action> {
    match code {
        KeyCode::Up | KeyCode::Char('k') => Some(Action::Up),
        KeyCode::Down | KeyCode::Char('j') => Some(Action::Down),
        KeyCode::Tab => Some(Action::SwitchPanel),
        KeyCode::Enter => Some(Action::Select),
        KeyCode::Esc => Some(Action::Deselect),
        KeyCode::Char('q') => Some(Action::Quit),
        _ => None,
    }
}
