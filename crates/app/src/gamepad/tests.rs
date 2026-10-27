use super::state::{PadEvent, PadState, radial_dead_zone, shape_look};
use crate::keybinds::{Action, INPUT_ACTIONS, PadButton, SLOTS};
use crate::player::Input;
use glam::Vec2;

#[test]
fn dead_zone_zeroes_the_centre_and_reaches_one_at_the_rim() {
    assert_eq!(radial_dead_zone(Vec2::new(0.1, 0.05), 0.15), Vec2::ZERO);
    assert_eq!(radial_dead_zone(Vec2::new(f32::NAN, 0.0), 0.15), Vec2::ZERO);
    let rim = radial_dead_zone(Vec2::new(0.0, -1.0), 0.15);
    assert!((rim - Vec2::new(0.0, -1.0)).length() < 1e-6);
    // Corners of square-gated pads read past 1 and clamp.
    assert!((radial_dead_zone(Vec2::splat(1.0), 0.15).length() - 1.0).abs() < 1e-6);
    let mid = radial_dead_zone(Vec2::new(0.575, 0.0), 0.15);
    assert!((mid.x - 0.5).abs() < 1e-6 && mid.y == 0.0);
}

#[test]
fn look_curve_keeps_direction_and_softens_small_deflection() {
    assert_eq!(shape_look(Vec2::new(0.1, 0.0)), Vec2::ZERO);
    let half = shape_look(Vec2::new(0.575, 0.0));
    assert!((half.x - 0.25).abs() < 1e-5, "{half}");
    let diagonal = shape_look(Vec2::new(-0.6, 0.6));
    assert!(diagonal.x < 0.0 && (diagonal.x + diagonal.y).abs() < 1e-6);
    assert!((shape_look(Vec2::new(1.0, 0.0)).x - 1.0).abs() < 1e-6);
}

fn sticks(left: Vec2) -> PadEvent {
    PadEvent::Sticks { left, right: Vec2::ZERO }
}

#[test]
fn move_stick_presses_with_hysteresis_and_diagonals() {
    let mut pad = PadState::default();
    let mut out = Vec::new();
    pad.feed(sticks(Vec2::new(0.0, 0.6)), &mut out);
    assert_eq!(out, [(PadButton::LeftStickUp, true)]);
    out.clear();
    // Between the release and press thresholds the held direction stays held.
    pad.feed(sticks(Vec2::new(0.0, 0.42)), &mut out);
    assert!(out.is_empty());
    pad.feed(sticks(Vec2::new(0.8, 0.8)), &mut out);
    assert_eq!(out, [(PadButton::LeftStickRight, true)]);
    out.clear();
    pad.feed(sticks(Vec2::ZERO), &mut out);
    assert_eq!(out, [(PadButton::LeftStickUp, false), (PadButton::LeftStickRight, false)]);
}

#[test]
fn pad_events_drive_input_through_default_bindings() {
    let mut pad = PadState::default();
    let mut input = Input::default();
    let mut out = Vec::new();
    for event in [
        sticks(Vec2::new(0.0, 1.0)),
        PadEvent::Button(PadButton::South, true),
        PadEvent::Button(PadButton::RightTrigger, true),
        PadEvent::Button(PadButton::RightTrigger, false),
    ] {
        pad.feed(event, &mut out);
    }
    for &(button, pressed) in &out {
        input.pad(button, pressed);
    }
    assert!(input.forward && input.jump && !input.back);
    assert!(input.break_pressed && !input.break_block, "a tap is kept until the next step");

    out.clear();
    pad.feed(PadEvent::Lost, &mut out);
    for &(button, pressed) in &out {
        input.pad(button, pressed);
    }
    assert!(!input.forward && !input.jump);
    out.clear();
    pad.feed(PadEvent::Lost, &mut out);
    assert!(out.is_empty(), "releases are sent once");
}

#[test]
fn every_held_binding_slot_fits_the_input_mask() {
    let mut input = Input::default();
    // An overflowing shift panics in test builds.
    for &action in &Action::ALL[..INPUT_ACTIONS] {
        for slot in 0..SLOTS {
            input.apply(action, slot, true);
            input.apply(action, slot, false);
        }
    }
    assert!(!input.forward && !input.zoom && !input.break_block);
}
