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
                    disabled=move || page <= 1
                    on:click=move |_| on_page.run(page - 1)
                >
                    "Prev"
                </button>
                <span>{format!("Page {page} of {pages}")}</span>
                <button
                    class="secondary outline"
                    // `>` would end the tag inside `view!`, so compound attribute
                    // expressions go in a closure (or braces).
                    disabled=move || page >= pages
                    on:click=move |_| on_page.run(page + 1)
                >
                    "Next"
                </button>
            </nav>
        }
    })
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
