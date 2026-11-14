//! City captures avoid synchronous profile output unless explicitly requested.
pub(crate) fn enabled(city: bool, physics_profile: bool, setting: Option<&str>) -> bool {
    if physics_profile { return true; }
    match setting {
        Some("1") => true,
        Some("0") => false,
        _ => !city,
    }
}
#[cfg(test)]
mod tests {
    use super::enabled;
    #[test]
    fn city_defaults_to_quiet() { assert!(!enabled(true,false,None)); }
    #[test]
    fn other_reviews_keep_readiness_profiles() { assert!(enabled(false,false,None)); }
    #[test]
    fn explicit_setting_overrides_default() {
        for city in [false,true] {
            assert!(enabled(city,false,Some("1")));
            assert!(!enabled(city,false,Some("0")));
        }
    }
    #[test]
    fn explicit_physics_profile_wins() {
        for city in [false,true] {
            for setting in [None,Some("0"),Some("1"),Some("invalid")] {
                assert!(enabled(city,true,setting));
            }
        }
    }
    #[test]
    fn invalid_setting_preserves_mode_default() {
        for setting in ["","false","true","invalid"," 0 "] {
            assert!(!enabled(true,false,Some(setting)));
            assert!(enabled(false,false,Some(setting)));
        }
    }
}
