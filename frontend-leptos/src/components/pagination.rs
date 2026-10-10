//! Prev / "Page X of Y" / Next controls.

use leptos::prelude::*;

/// Number of pages for `total` items; always at least 1.
pub fn total_pages(total: u64, per_page: u32) -> u32 {
    let per_page = u64::from(per_page.max(1));
    u32::try_from(total.div_ceil(per_page).max(1)).unwrap_or(u32::MAX)
}

/// Hidden unless there is more than one page (`total > per_page`).
#[component]
pub fn Pagination(page: u32, per_page: u32, total: u64, on_page: Callback<u32>) -> impl IntoView {
    let pages = total_pages(total, per_page);
    (total > u64::from(per_page)).then(|| {
        view! {
            <nav class="pagination">
                <button
                    class="secondary outline"
                    disabled={page <= 1}
                    on:click=move |_| on_page.run(page - 1)
                >
                    "Prev"
                </button>
                <span>{format!("Page {page} of {pages}")}</span>
                <button
                    class="secondary outline"
                    // `view!` parses `>` as the end of the tag, even inside a closure,
                    // so a comparison in an attribute must be wrapped in braces.
                    disabled={page >= pages}
                    on:click=move |_| on_page.run(page + 1)
                >
                    "Next"
                </button>
            </nav>
        }
    })
}

/// Shown instead of an empty list when `?page=` is past the last page (for
/// example after deleting the last row of the last page).
#[component]
pub fn OutOfRange(on_page: Callback<u32>) -> impl IntoView {
    view! {
        <p>
            "This page has no results. "
            <a
                href="#"
                on:click=move |ev| {
                    ev.prevent_default();
                    on_page.run(1);
                }
            >
                "Go to page 1"
            </a>
        </p>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_count_rounds_up() {
        assert_eq!(total_pages(0, 20), 1);
        assert_eq!(total_pages(20, 20), 1);
        assert_eq!(total_pages(21, 20), 2);
        assert_eq!(total_pages(100, 20), 5);
        assert_eq!(total_pages(5, 0), 5);
    }
}
