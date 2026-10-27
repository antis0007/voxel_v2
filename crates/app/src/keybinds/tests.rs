use super::*;
use winit::{event::MouseButton, keyboard::KeyCode};

#[test]
fn default_asset_names_every_action_once_with_a_label() {
    let rows: Vec<DefaultRow> = ron::from_str(DEFAULTS).expect("keybinds.ron parses");
    for action in Action::ALL {
        let matching = rows.iter().filter(|r| r.action == action.name()).count();
        assert_eq!(matching, 1, "{action:?} rows");
        assert!(!action.label().is_empty() && !action.group().is_empty(), "{action:?}");
    }
    assert_eq!(rows.len(), Action::ALL.len(), "rows naming no action");
    for row in &rows {
        for key in &row.keys {
            assert!(Binding::parse(key).is_some(), "{} names unknown input {key}", row.action);
        }
    }
}

#[test]
fn defaults_keep_the_historic_layout_without_conflicts() {
    let b = Bindings::defaults();
    let key = |code| b.lookup(Binding::Key(code)).map(|(a, _)| a);
    assert_eq!(key(KeyCode::KeyW), Some(Action::Forward));
    assert_eq!(key(KeyCode::ShiftRight), Some(Action::Sneak));
    assert_eq!(key(KeyCode::Tab), Some(Action::Inventory));
    assert_eq!(key(KeyCode::Digit9), Some(Action::Hotbar9));
    assert_eq!(b.lookup(Binding::Mouse(MouseButton::Left)), Some((Action::Break, 0)));
    assert_eq!(b.lookup(Binding::Mouse(MouseButton::Back)), Some((Action::Dash, 1)));
    let rows: Vec<DefaultRow> = ron::from_str(DEFAULTS).expect("keybinds.ron parses");
    let total: usize = rows.iter().map(|r| r.keys.len()).sum();
    let bound: usize = Action::ALL.iter().map(|a| b.slots(*a).iter().flatten().count()).sum();
    assert_eq!(bound, total, "a default binding was displaced by a duplicate");
}

#[test]
fn input_actions_lead_the_enum_and_hotbar_slots_map() {
    for action in Action::ALL {
        assert_eq!(action.mouse_bindable(), (*action as usize) < INPUT_ACTIONS);
    }
    assert_eq!(Action::Hotbar1.hotbar_slot(), Some(0));
    assert_eq!(Action::Hotbar9.hotbar_slot(), Some(8));
    assert_eq!(Action::Fullscreen.hotbar_slot(), None);
}

#[test]
fn assigning_a_used_binding_unbinds_the_previous_action() {
    let mut b = Bindings::defaults();
    let displaced = b.assign(Action::Zoom, 1, Some(Binding::Key(KeyCode::KeyC)));
    assert_eq!(displaced, Some(Action::ToolBuilders));
    assert_eq!(b.slots(Action::ToolBuilders), [None, None]);
    assert_eq!(b.lookup(Binding::Key(KeyCode::KeyC)), Some((Action::Zoom, 1)));
    // Moving a binding between slots of one action is not a displacement.
    assert_eq!(b.assign(Action::Zoom, 0, Some(Binding::Key(KeyCode::KeyC))), None);
    assert_eq!(b.slots(Action::Zoom), [Some(Binding::Key(KeyCode::KeyC)), None]);
}

#[test]
fn mouse_buttons_are_refused_for_key_only_actions() {
    let mut b = Bindings::defaults();
    let before = b.clone();
    assert_eq!(b.assign(Action::Inventory, 0, Some(Binding::Mouse(MouseButton::Forward))), None);
    assert_eq!(b, before);
}

#[test]
fn user_file_round_trips_and_keeps_new_actions_at_default() {
    let mut b = Bindings::defaults();
    b.assign(Action::Jump, 1, Some(Binding::Mouse(MouseButton::Other(7))));
    b.assign(Action::Lamp, 0, None);
    let text = b.to_user().expect("serialises");
    assert_eq!(Bindings::from_user(&text).expect("parses"), b);

    let partial = r#"[(action: "Forward", keys: [Some("ArrowUp"), None]), (action: "Gone", keys: [])]"#;
    let loaded = Bindings::from_user(partial).expect("parses");
    assert_eq!(loaded.slots(Action::Forward), [Some(Binding::Key(KeyCode::ArrowUp)), None]);
    assert_eq!(loaded.slots(Action::Back), Bindings::defaults().slots(Action::Back));
}

#[test]
fn malformed_user_file_is_an_error_and_unknown_keys_drop() {
    assert!(Bindings::from_user("[(action: 3)]").is_err());
    let loaded = Bindings::from_user(r#"[(action: "Jump", keys: [Some("NoSuchKey")])]"#).expect("parses");
    assert_eq!(loaded.slots(Action::Jump), [None, None]);
}

#[test]
fn binding_names_round_trip_and_labels_are_short() {
    for name in ["KeyW", "Digit3", "F11", "ShiftLeft", "MouseLeft", "MouseForward", "Mouse9", "Numpad5"] {
        let binding = Binding::parse(name).expect(name);
        assert_eq!(binding.name(), name);
    }
    assert_eq!(Binding::Key(KeyCode::KeyW).label(), "W");
    assert_eq!(Binding::Key(KeyCode::Digit3).label(), "3");
    assert_eq!(Binding::Key(KeyCode::Numpad5).label(), "Num 5");
    assert_eq!(Binding::Key(KeyCode::ShiftLeft).label(), "Left Shift");
    assert_eq!(Binding::Mouse(MouseButton::Back).label(), "Mouse 4");
}

#[test]
fn default_labels_resolve_to_their_action() {
    assert_eq!(Action::ALL.iter().filter(|a| a.hotbar_slot().is_some()).count(), 9);
    let defaults = Bindings::defaults();
    for label in ["F", "E", "L", "R"] {
        let action = Action::ALL
            .iter()
            .find(|a| defaults.slots(**a).iter().flatten().any(|b| b.label() == label));
        assert!(action.is_some(), "{label}");
    }
}
