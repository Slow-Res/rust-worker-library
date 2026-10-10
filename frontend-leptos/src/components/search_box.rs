//! Search input that reports changes 300 ms after the user stops typing.

use gloo_timers::callback::Timeout;
use leptos::prelude::*;

const DEBOUNCE_MS: u32 = 300;

/// Whether the input should adopt the committed (URL) value. Callers trim
/// what they commit, so a trailing space the user typed must not count as a
/// difference, and nothing is overwritten while a debounce is pending.
fn should_sync(text: &str, committed: &str, typing: bool) -> bool {
    !typing && text.trim() != committed.trim()
}

/// `value` is the committed value (usually read from the URL); `on_change`
/// receives the new text once typing pauses.
#[component]
pub fn SearchBox(
    #[prop(into)] value: Signal<String>,
    on_change: Callback<String>,
    #[prop(into)] placeholder: String,
) -> impl IntoView {
    // What is in the input right now, which may be ahead of `value`.
    let text = RwSignal::new(value.get_untracked());
    // The pending timer. Dropping a `Timeout` cancels it, so replacing it on
    // every keystroke is the whole debounce. `new_local` because the handle
    // is not `Send`.
    let pending = StoredValue::new_local(None::<Timeout>);
    // True from the first keystroke until the debounce fires. While true the
    // input is ahead of `value`, so syncing back would overwrite newer text.
    let typing = StoredValue::new(false);

    // An Effect re-runs when the signals it reads change. Here it keeps the
    // input in step when the URL changes from outside (Back button, "reset").
    Effect::new(move |_| {
        let committed = value.get();
        if should_sync(&text.get_untracked(), &committed, typing.get_value()) {
            text.set(committed);
        }
    });

    let on_input = move |ev| {
        let next = event_target_value(&ev);
        text.set(next.clone());
        typing.set_value(true);
        pending.set_value(Some(Timeout::new(DEBOUNCE_MS, move || {
            typing.set_value(false);
            on_change.run(next);
        })));
    };

    view! {
        <input
            type="search"
            placeholder=placeholder
            prop:value=move || text.get()
            on:input=on_input
        />
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_sync_while_typing() {
        assert!(!should_sync("ab", "a", true));
    }

    #[test]
    fn keeps_trailing_space_of_trimmed_commit() {
        assert!(!should_sync("war ", "war", false));
    }

    #[test]
    fn syncs_external_changes() {
        assert!(should_sync("war", "", false));
        assert!(should_sync("war", "peace", false));
    }
}
