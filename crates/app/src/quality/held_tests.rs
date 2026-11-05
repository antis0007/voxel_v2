use super::*;

#[test]
fn a_cut_saves_the_players_choice() {
    let mut slot = None;
    Held::record(&mut slot, 8, 6);
    assert_eq!(Held::saved(slot, 6), 8);
}

#[test]
fn a_second_cut_keeps_the_first_choice() {
    let mut slot = None;
    Held::record(&mut slot, 8, 6);
    Held::record(&mut slot, 6, 4);
    assert_eq!(Held::saved(slot, 4), 8);
}

#[test]
fn a_player_change_after_a_cut_is_saved() {
    let mut slot = None;
    Held::record(&mut slot, 8, 6);
    assert_eq!(Held::saved(slot, 12), 12);
    // A cut taken from the player's new value starts from that value.
    Held::record(&mut slot, 12, 9);
    assert_eq!(Held::saved(slot, 9), 12);
}

#[test]
fn a_step_that_changed_nothing_records_nothing() {
    let mut slot = None;
    Held::record(&mut slot, 4096, 4096);
    assert!(slot.is_none());
    assert_eq!(Held::saved(slot, 4096), 4096);
}
