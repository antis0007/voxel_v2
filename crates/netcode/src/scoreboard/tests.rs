use super::*;

const DEATHS: u8 = 0;
const PLAYED: u8 = 1;
const DEPTH: u8 = 2;

fn objective(label: &str, unit: Unit, order: Order) -> Objective {
    Objective { label: label.into(), unit, order }
}

fn server() -> BoardWriter {
    let mut w = BoardWriter::default();
    w.change(Change::Define { id: DEATHS, objective: objective("Deaths", Unit::Count, Order::LowFirst) }).unwrap();
    w.change(Change::Define { id: PLAYED, objective: objective("Played", Unit::Seconds, Order::HighFirst) }).unwrap();
    w.change(Change::Define { id: DEPTH, objective: objective("Deepest", Unit::Metres, Order::HighFirst) }).unwrap();
    w.change(Change::Show { slot: Slot::List, id: Some(PLAYED) }).unwrap();
    w.change(Change::Show { slot: Slot::Sidebar, id: Some(DEPTH) }).unwrap();
    w
}

#[test]
fn clients_and_late_joiners_rebuild_the_server_board() {
    let mut w = server();
    let mut client = Board::default();
    apply_flush(&mut client, &w.flush()).unwrap();
    for tick in 0..50i64 {
        for player in 0..4u32 {
            w.add(PLAYED, player, 1).unwrap();
            w.set(DEPTH, player, i64::from(player) * 40 + tick / 10).unwrap();
        }
        if tick == 20 {
            w.add(DEATHS, 2, 1).unwrap();
            w.change(Change::Clear { player: 3 }).unwrap();
        }
        let bytes = w.flush();
        apply_flush(&mut client, &bytes).unwrap();
    }
    assert_eq!(&client, w.board());
    let mut joiner = Board::default();
    apply_flush(&mut joiner, &encode(&w.board().snapshot())).unwrap();
    assert_eq!(&joiner, w.board());
}

#[test]
fn a_counter_bumped_many_times_sends_once_per_flush() {
    let mut w = server();
    w.flush();
    for _ in 0..1_000 {
        w.add(PLAYED, 7, 1).unwrap();
    }
    let bytes = w.flush();
    assert!(bytes.len() <= 6, "{} bytes", bytes.len());
    assert!(w.flush().is_empty());
    w.set(PLAYED, 7, 1_000).unwrap();
    assert!(w.flush().is_empty(), "an unchanged value sends nothing");
}

#[test]
fn rankings_follow_the_objective_order_with_ties_by_player() {
    let mut w = server();
    for (player, deaths) in [(4, 2), (1, 0), (9, 2), (3, 5)] {
        w.set(DEATHS, player, deaths).unwrap();
        w.set(DEPTH, player, deaths * 10).unwrap();
    }
    assert_eq!(w.board().ranking(DEATHS, 3), vec![(1, 0), (4, 2), (9, 2)]);
    assert_eq!(w.board().ranking(DEPTH, 2), vec![(3, 50), (4, 20)]);
    assert_eq!(w.board().shown(Slot::Sidebar).map(|(id, o)| (id, o.label.as_str())), Some((DEPTH, "Deepest")));
}

#[test]
fn removing_an_objective_clears_its_scores_and_slots() {
    let mut w = server();
    w.set(DEPTH, 1, 30).unwrap();
    w.change(Change::Remove { id: DEPTH }).unwrap();
    assert_eq!(w.board().shown(Slot::Sidebar), None);
    assert_eq!(w.board().score(DEPTH, 1), None);
    let mut client = Board::default();
    apply_flush(&mut client, &w.flush()).unwrap();
    assert_eq!(&client, w.board());
}

#[test]
fn values_format_with_units() {
    assert_eq!(format_value(42, Unit::Count), "42");
    assert_eq!(format_value(65, Unit::Seconds), "1:05");
    assert_eq!(format_value(3_725, Unit::Seconds), "1:02:05");
    assert_eq!(format_value(-5, Unit::Seconds), "-0:05");
    assert_eq!(format_value(-120, Unit::Metres), "-120 m");
}

#[test]
fn hostile_flushes_leave_the_board_unchanged() {
    let mut board = Board::default();
    apply_flush(&mut board, &encode(&server().board().snapshot())).unwrap();
    let before = board.clone();
    let bad = [
        vec![Change::Score { id: 99, player: 1, value: 1 }],
        vec![Change::Define { id: 50, objective: objective(&"x".repeat(25), Unit::Count, Order::HighFirst) }],
        vec![Change::Define { id: 255, objective: objective("Kills", Unit::Count, Order::HighFirst) }],
        vec![Change::Define { id: 50, objective: objective("a\nb", Unit::Count, Order::HighFirst) }],
        (10..30).map(|id| Change::Define { id, objective: objective("Kills", Unit::Count, Order::HighFirst) }).collect(),
        vec![Change::Score { id: DEATHS, player: 1, value: 3 }, Change::Show { slot: Slot::BelowName, id: Some(77) }],
    ];
    for changes in bad {
        assert!(apply_flush(&mut board, &encode(&changes)).is_err(), "{changes:?}");
        assert_eq!(board, before);
    }
    let mut trailing = encode(&[Change::Clear { player: 1 }]);
    trailing.push(0);
    assert!(apply_flush(&mut board, &trailing).is_err());
    assert!(apply_flush(&mut board, &[1, 9]).is_err());
}
