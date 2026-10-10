//! Search input that reports changes 300 ms after the user stops typing.

use std::time::Duration;

use gloo_timers::callback::Timeout;
use leptos::prelude::*;

const DEBOUNCE: Duration = Duration::from_millis(300);

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
        if !typing.get_value() && text.get_untracked() != committed {
            text.set(committed);
        }
    });

    let on_input = move |ev| {
        let next = event_target_value(&ev);
        text.set(next.clone());
        typing.set_value(true);
        pending.set_value(Some(Timeout::new(DEBOUNCE.as_millis() as u32, move || {
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
