//! Pure display helpers (dates, labels). Kept free of browser calls so they
//! can be unit-tested natively.

use chrono::{DateTime, Duration, Utc};
use library_shared::Loan;

/// `YYYY-MM-DD` for a UTC instant shifted by the browser's UTC offset.
///
/// `offset_minutes` follows JavaScript's `getTimezoneOffset()`: minutes *behind*
/// UTC, so UTC+2 is `-120`.
pub fn format_date(instant: DateTime<Utc>, offset_minutes: i32) -> String {
    let local = instant - Duration::minutes(i64::from(offset_minutes));
    local.format("%Y-%m-%d").to_string()
}

/// "2 / 3" for available / total copies.
pub fn copies_label(available: i64, total: i64) -> String {
    format!("{available} / {total}")
}

/// "Returned YYYY-MM-DD", "Overdue" or "Active". `offset_minutes` is as in [`format_date`].
pub fn loan_status_label(loan: &Loan, offset_minutes: i32) -> String {
    match loan.returned_at {
        Some(returned_at) => format!("Returned {}", format_date(returned_at, offset_minutes)),
        None if loan.is_overdue => "Overdue".to_string(),
        None => "Active".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(secs, 0).unwrap()
    }

    #[test]
    fn formats_utc_date() {
        // 2027-01-15 23:30:00 UTC
        let t = at(1_800_055_800);
        assert_eq!(format_date(t, 0), "2027-01-15");
    }

    #[test]
    fn applies_offset_across_midnight() {
        let t = at(1_800_055_800); // 23:30 UTC
        assert_eq!(format_date(t, -120), "2027-01-16"); // UTC+2
        assert_eq!(format_date(t, 300), "2027-01-15"); // UTC-5
    }

    #[test]
    fn copies_label_shows_available_over_total() {
        assert_eq!(copies_label(2, 3), "2 / 3");
    }

    #[test]
    fn loan_status_prefers_returned() {
        let now = at(1_800_000_000);
        let mut loan = Loan {
            id: 1,
            book_id: 1,
            book_title: "t".into(),
            member_id: 1,
            member_name: "m".into(),
            borrowed_at: now,
            due_at: now,
            returned_at: None,
            is_overdue: true,
        };
        assert_eq!(loan_status_label(&loan, 0), "Overdue");
        loan.returned_at = Some(now);
        assert_eq!(loan_status_label(&loan, 0), "Returned 2027-01-15");
        assert_eq!(loan_status_label(&loan, -1_000), "Returned 2027-01-16");
        loan.returned_at = None;
        loan.is_overdue = false;
        assert_eq!(loan_status_label(&loan, 0), "Active");
    }
}
