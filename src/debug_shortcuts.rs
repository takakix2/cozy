use std::collections::HashMap;
use crossterm::event::{KeyCode, KeyModifiers};

// Mock EditorAction for testing
#[derive(Debug, PartialEq)]
enum EditorAction {
    Help,
    ToggleLineNumbers,
    DeleteLine,
    // Add other variants as needed
}

// Mock Shortcut struct
struct Shortcut {
    key: KeyCode,
    modifiers: KeyModifiers,
    action: EditorAction,
    label: &'static str,
}

fn sc(key: KeyCode, modifiers: KeyModifiers, action: EditorAction, label: &'static str) -> Shortcut {
    Shortcut { key, modifiers, action, label }
}

fn utility_shortcuts() -> Vec<Shortcut> {
    vec![
        sc(KeyCode::Char('g'), KeyModifiers::CONTROL, EditorAction::Help, "Ctrl+G Help"),
        sc(KeyCode::Char('l'), KeyModifiers::CONTROL, EditorAction::ToggleLineNumbers, "Ctrl+L LineNo"),
        sc(KeyCode::Char('k'), KeyModifiers::CONTROL, EditorAction::DeleteLine, "Ctrl+K DelLine"),
    ]
}

fn main() {
    let shortcuts = utility_shortcuts();
    let mut map = HashMap::new();
    for shortcut in shortcuts {
        map.insert((shortcut.key, shortcut.modifiers), shortcut.action);
    }
    
    println!("Checking Ctrl+M mapping:");
    let key = (KeyCode::Char('m'), KeyModifiers::CONTROL);
    if let Some(action) = map.get(&key) {
        println!("Found action: {:?}", action);
    } else {
        println!("Action not found!");
    }
}
