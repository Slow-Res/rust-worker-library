//! Thin wrappers over browser APIs, so pages stay free of `web_sys` details.

use chrono::{DateTime, Utc};

use crate::format::format_date;

/// Shows a blocking `window.confirm` dialog; any failure counts as "no".
pub fn confirm(message: &str) -> bool {
    web_sys::window()
        .and_then(|w| w.confirm_with_message(message).ok())
        .unwrap_or(false)
}

/// `YYYY-MM-DD` in the browser's local time zone.
pub fn local_date(instant: DateTime<Utc>) -> String {
    let js_date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(
        instant.timestamp_millis() as f64
    ));
    // `get_timezone_offset` is whole or fractional minutes; rounding is fine
    // for a date-only display.
    format_date(instant, js_date.get_timezone_offset().round() as i32)
}
