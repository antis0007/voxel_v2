use super::*;

fn at_rest(p: Pose) -> bool {
    p.offset.length() < 1e-5 && p.rotation.angle_between(Quat::IDENTITY) < 1e-4
}

#[test]
fn viewmodel_clips_load_with_ordered_keys_spanning_each_clip() {
    for name in Name::ALL {
        let clip = name.clip();
        assert!(clip.duration > 0.0 && clip.keys.len() >= 2, "{name:?}");
        assert_eq!(clip.keys[0].t, 0.0, "{name:?}");
        assert!((clip.keys[clip.keys.len() - 1].t - clip.duration).abs() < 1e-6, "{name:?}");
        assert!(clip.keys.windows(2).all(|w| w[0].t < w[1].t), "{name:?} keys out of order");
        for key in &clip.keys {
            assert!(key.offset.iter().chain(&key.turn).all(|x| x.is_finite()), "{name:?}");
            // A turn past half a revolution would slerp the short way round, not as authored.
            assert!(Vec3::from_array(key.turn).length() <= std::f32::consts::PI, "{name:?}");
        }
    }
}

#[test]
fn viewmodel_clips_start_and_end_at_rest() {
    for name in Name::ALL {
        let clip = name.clip();
        assert!(at_rest(clip.sample(clip.duration)), "{name:?} must end at the rest hold");
        if !matches!(name, Name::Equip | Name::Idle) {
            assert!(at_rest(clip.sample(0.0)), "{name:?} must start at the rest hold");
        }
    }
    let idle = Name::Idle.clip();
    assert!(idle.looped);
    let (a, b) = (idle.sample(0.0), idle.sample(idle.duration - 1e-4));
    assert!(a.offset.distance(b.offset) < 1e-4 && a.rotation.angle_between(b.rotation) < 1e-3, "idle loop seam");
}

#[test]
fn viewmodel_clips_strikes_wind_up_slowly_and_cut_fast() {
    for name in [Name::Forehand, Name::Backhand, Name::Overhead, Name::Chop] {
        let clip = name.clip();
        let impact = clip.impact.unwrap_or_else(|| panic!("{name:?} has no impact marker"));
        assert!(impact > 0.0 && impact < clip.duration, "{name:?}");
        let cut = clip.keys.iter().position(|k| (k.t - impact).abs() < 1e-6)
            .unwrap_or_else(|| panic!("{name:?}: impact must sit on a key"));
        assert!(cut >= 2, "{name:?} needs a wind-up key before the cut");
        let (load, contact) = (clip.keys[cut - 1].t, clip.keys[cut].t);
        assert_eq!(clip.keys[cut].ease, Ease::In, "{name:?} cut must still be accelerating at contact");
        if name != Name::Chop {
            assert!((contact - load - 0.07).abs() < 1e-3, "{name:?} cut lasts {} s", contact - load);
            assert!(load > contact - load, "{name:?} wind-up must outlast the cut");
        }
    }
    assert!(Name::Use.clip().impact.is_none() && Name::Idle.clip().impact.is_none());
}

#[test]
fn viewmodel_clips_impact_times_match_owner_contact() {
    // 2026-11-06: contact 180 ms on combo steps, 260 ms on the finisher, at swing speed 1.
    for (name, contact) in [(Name::Forehand, 0.18), (Name::Backhand, 0.18), (Name::Overhead, 0.26)] {
        let clip = name.clip();
        assert!((clip.duration * clip.impact_fraction() - contact).abs() < 1e-5, "{name:?}");
    }
    assert_eq!(Name::stroke(false, false), Name::Forehand);
    assert_eq!(Name::stroke(true, false), Name::Backhand);
    assert_eq!(Name::stroke(true, true), Name::Overhead);
}

#[test]
fn viewmodel_clips_sample_out_of_range_times_safely() {
    for name in Name::ALL {
        let clip = name.clip();
        for t in [-1.0, f32::NAN, f32::INFINITY, clip.duration * 3.5] {
            let p = clip.sample(t);
            assert!(p.offset.is_finite() && p.rotation.is_finite(), "{name:?} at {t}");
        }
    }
}
