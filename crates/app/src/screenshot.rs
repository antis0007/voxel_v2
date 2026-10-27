//! Screenshot file naming under the save directory.

use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

/// A fresh PNG path named by UTC time; a numeric suffix separates shots in the same second.
pub fn next_path() -> PathBuf {
    let dir = crate::world::storage::save_directory().join("screenshots");
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let stem = stamp(secs);
    let mut path = dir.join(format!("{stem}.png"));
    let mut n = 2;
    while path.exists() && n < 1000 {
        path = dir.join(format!("{stem}_{n}.png"));
        n += 1;
    }
    path
}

/// `YYYY-MM-DD_HH.MM.SS` for Unix seconds, UTC.
fn stamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Howard Hinnant's days-to-civil conversion.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}_{:02}.{:02}.{:02}",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn stamp_matches_known_dates() {
        assert_eq!(super::stamp(0), "1970-01-01_00.00.00");
        assert_eq!(super::stamp(951_782_400), "2000-02-29_00.00.00");
        assert_eq!(super::stamp(1_792_998_245), "2026-10-26_07.04.05");
    }
}
