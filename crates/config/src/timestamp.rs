//! Marca de tiempo `YYYYMMDD-HHMMSS` en UTC, sin dependencias de fecha.

use std::time::{SystemTime, UNIX_EPOCH};

/// Sufijo de backup para el instante actual.
pub(crate) fn stamp() -> String {
    stamp_at(SystemTime::now())
}

/// Sufijo de backup determinista para tests.
pub(crate) fn stamp_at(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let days = seconds / 86_400;
    let time_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    let hour = time_of_day / 3_600;
    let minute = (time_of_day % 3_600) / 60;
    let second = time_of_day % 60;
    format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}")
}

// Howard Hinnant, `civil_from_days` (dominio público).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn timestamp_has_the_expected_shape() {
        assert_eq!(stamp_at(UNIX_EPOCH), "19700101-000000");
        assert_eq!(
            stamp_at(UNIX_EPOCH + Duration::from_secs(1_700_000_000)),
            "20231114-221320"
        );
    }
}
