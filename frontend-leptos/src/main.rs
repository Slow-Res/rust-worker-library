//! Entry point of the Leptos (WebAssembly, client-side rendered) frontend.

mod api;
mod app;
mod auth;
mod browser;
mod components;
mod format;
mod pages;
mod url;

fn main() {
    // `mount_to_body` appends the app to <body>; Leptos then keeps the DOM in sync.
    leptos::mount::mount_to_body(app::App);
}
