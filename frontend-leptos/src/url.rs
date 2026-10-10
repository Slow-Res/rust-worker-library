//! Pure helpers for route parameters and in-app URLs.

use library_shared::{Availability, LoanStatus};

/// Parses a route `:id`; only positive integers are valid ids.
pub fn parse_id(raw: Option<&str>) -> Option<i64> {
    raw?.parse::<i64>().ok().filter(|id| *id > 0)
}

/// `?page=` value; anything missing or invalid means page 1.
pub fn parse_page(raw: Option<&str>) -> u32 {
    raw.and_then(|p| p.parse::<u32>().ok())
        .filter(|p| *p >= 1)
        .unwrap_or(1)
}

pub fn parse_availability(raw: Option<&str>) -> Availability {
    match raw {
        Some("available") => Availability::Available,
        Some("on_loan") => Availability::OnLoan,
        _ => Availability::All,
    }
}

pub fn availability_param(value: Availability) -> &'static str {
    match value {
        Availability::All => "",
        Availability::Available => "available",
        Availability::OnLoan => "on_loan",
    }
}

pub fn parse_loan_status(raw: Option<&str>) -> LoanStatus {
    match raw {
        Some("overdue") => LoanStatus::Overdue,
        Some("returned") => LoanStatus::Returned,
        Some("all") => LoanStatus::All,
        _ => LoanStatus::Active,
    }
}

pub fn loan_status_param(value: LoanStatus) -> &'static str {
    match value {
        LoanStatus::Active => "active",
        LoanStatus::Overdue => "overdue",
        LoanStatus::Returned => "returned",
        LoanStatus::All => "all",
    }
}

/// `path?k=v&...`, skipping pairs whose value is empty so the URL only
/// carries non-default state.
pub fn build_url(path: &str, params: &[(&str, &str)]) -> String {
    let kept: Vec<(&str, &str)> = params
        .iter()
        .copied()
        .filter(|(_, v)| !v.is_empty())
        .collect();
    match serde_urlencoded::to_string(&kept) {
        Ok(qs) if !qs.is_empty() => format!("{path}?{qs}"),
        _ => path.to_string(),
    }
}

/// `page` as a URL value: page 1 is the default, so it is omitted.
pub fn page_param(page: u32) -> String {
    if page <= 1 {
        String::new()
    } else {
        page.to_string()
    }
}

/// Only same-site absolute paths may be used as a post-login destination;
/// anything else (e.g. `//evil.com`) falls back to `/loans`.
pub fn safe_next(raw: Option<&str>) -> String {
    match raw {
        Some(next) if next.starts_with('/') && !next.starts_with("//") => next.to_string(),
        _ => "/loans".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_must_be_positive_integers() {
        assert_eq!(parse_id(Some("42")), Some(42));
        for bad in ["0", "-3", "abc", "1.5", "", "99999999999999999999"] {
            assert_eq!(parse_id(Some(bad)), None, "{bad}");
        }
        assert_eq!(parse_id(None), None);
    }

    #[test]
    fn page_defaults_to_one() {
        assert_eq!(parse_page(Some("3")), 3);
        assert_eq!(parse_page(Some("0")), 1);
        assert_eq!(parse_page(Some("x")), 1);
        assert_eq!(parse_page(None), 1);
    }

    #[test]
    fn filters_round_trip() {
        for a in [
            Availability::All,
            Availability::Available,
            Availability::OnLoan,
        ] {
            assert_eq!(parse_availability(Some(availability_param(a))), a);
        }
        for s in [
            LoanStatus::Active,
            LoanStatus::Overdue,
            LoanStatus::Returned,
            LoanStatus::All,
        ] {
            assert_eq!(parse_loan_status(Some(loan_status_param(s))), s);
        }
    }

    #[test]
    fn build_url_drops_empty_values_and_encodes() {
        assert_eq!(build_url("/books", &[("q", ""), ("page", "")]), "/books");
        assert_eq!(
            build_url(
                "/books",
                &[("q", "a b"), ("availability", "on_loan"), ("page", "2")]
            ),
            "/books?q=a+b&availability=on_loan&page=2"
        );
    }

    #[test]
    fn page_param_omits_first_page() {
        assert_eq!(page_param(1), "");
        assert_eq!(page_param(4), "4");
    }

    #[test]
    fn next_must_be_a_local_path() {
        assert_eq!(safe_next(Some("/members?page=2")), "/members?page=2");
        assert_eq!(safe_next(Some("//evil.com")), "/loans");
        assert_eq!(safe_next(Some("https://evil.com")), "/loans");
        assert_eq!(safe_next(None), "/loans");
    }
}
