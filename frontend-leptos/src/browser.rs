//! Thin wrappers over browser APIs, so pages stay free of `web_sys` details.

use chrono::{DateTime, Utc};

use crate::format::format_date;

/// Shows a blocking `window.confirm` dialog; any failure counts as "no".
pub fn confirm(message: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.confirm_with_message(message).ok())
        .unwrap_or(false)
}

/// The browser's UTC offset at `instant`, in minutes (JavaScript's `getTimezoneOffset()`).
pub fn local_offset_minutes(instant: DateTime<Utc>) -> i32 {
    let js_date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(
        instant.timestamp_millis() as f64
    ));
    // `get_timezone_offset` is whole or fractional minutes; rounding is fine
    // for a date-only display.
    js_date.get_timezone_offset().round() as i32
}

/// `YYYY-MM-DD` in the browser's local time zone.
pub fn local_date(instant: DateTime<Utc>) -> String {
    format_date(instant, local_offset_minutes(instant))
}
