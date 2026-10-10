# React vs Leptos

The app ships two frontends with the same routes, the same features and the
same look: [frontend-react/](../frontend-react/) (React 19 + TypeScript +
Vite) and [frontend-leptos/](../frontend-leptos/) (Leptos 0.8, client-side
rendered Rust compiled to WebAssembly, built by Trunk). Both talk to the same
backend through the same JSON API, so the comparison is about the frontend
technology and nothing else.

This is a comparison of *this* code, written by the same hands for the same
spec. It is not a general verdict on either framework.

Contents: [Orientation](#orientation) ·
[Side by side](#side-by-side) ·
[Bundle size](#bundle-size) ·
[Gotchas we actually hit](#gotchas-we-actually-hit) ·
[Takeaways](#takeaways)

## Orientation

The Leptos tree deliberately mirrors the React one, so you can open the same
feature in both:

| Concern | React (`frontend-react/src/`) | Leptos (`frontend-leptos/src/`) |
|---|---|---|
| Entry | `main.tsx` | `main.rs` |
| Routes + layout | `App.tsx`, `components/Layout.tsx` | `app.rs`, `components/layout.rs` |
| API client | `api/client.ts`, `api/{auth,books,members,loans}.ts` | `api/mod.rs`, `api/{auth,books,members,loans}.rs` |
| Auth state + guard | `auth/useCurrentUser.ts`, `auth/RequireAuth.tsx`, `queryClient.ts` | `auth.rs` |
| Pages | `pages/*Page.tsx` | `pages/*.rs` |
| Shared components | `components/{Pagination,SearchBox,ErrorMessage,FieldError,...}.tsx` | `components/{pagination,search_box,error_message,field_error}.rs` |
| Types from the API | `generated/*.ts` (generated from Rust by ts-rs) | `library_shared::*` (the Rust structs themselves) |

The last row is the biggest structural difference. React needs a generated
copy of every API type (`make types`, checked in CI). Leptos imports the
backend's own structs, so a field change is a compile error rather than a
regenerate-and-commit step.

## Side by side

### State and the URL

Both apps keep list state (search text, filter, page) **in the URL**, so
refresh and the Back button work.

React reads and writes it through a hook that wraps `useSearchParams`
([hooks/useListParams.ts:8](../frontend-react/src/hooks/useListParams.ts#L8)).
The page re-renders whenever the URL changes and simply recomputes
`const q = get('q')` ([pages/BooksPage.tsx:30](../frontend-react/src/pages/BooksPage.tsx#L30)).

Leptos derives *reactive values* from the URL
([pages/books.rs:28-31](../frontend-leptos/src/pages/books.rs#L28)):
`Memo::new(move |_| query.read().get("q")...)`. A `Memo` re-computes only when
something it reads changes and notifies dependants only when its own value
changes. The component function runs **once**; only the closures that read
signals re-run. In React the whole component function re-runs on every render.

### Debounced search

Both implement "search 300 ms after typing stops".

- React: a generic `useDebounce` hook with `setTimeout` in an effect
  ([hooks/useDebounce.ts:4](../frontend-react/src/hooks/useDebounce.ts#L4)),
  used by [components/SearchBox.tsx:15](../frontend-react/src/components/SearchBox.tsx#L15).
  Extra effort goes into not clobbering text the user is mid-typing when the
  URL changes (`shouldSync`).
- Leptos: store the timer handle in a `StoredValue`; **dropping a `Timeout`
  cancels it**, so replacing it on each keystroke is the whole debounce
  ([components/search_box.rs:28-50](../frontend-leptos/src/components/search_box.rs#L28)).
  An `Effect` keeps the input in sync with the URL, with the same
  "do not overwrite while typing" rule (`should_sync`, line 11, unit tested).

### Data fetching

| | React | Leptos |
|---|---|---|
| Fetch + cache | TanStack Query: `useQuery({ queryKey, queryFn })` ([BooksPage.tsx:33](../frontend-react/src/pages/BooksPage.tsx#L33)) | `LocalResource::new(move \|\| async { ... })` ([books.rs:40](../frontend-leptos/src/pages/books.rs#L40)) |
| Refetch when inputs change | The query key includes the query object, so a new key means a new request | The closure reads signals (`q.get()`, `page.get()`), so changing one re-runs it |
| After a mutation | `invalidateLibraryData(queryClient, 'books', 'loans', ...)` marks cached keys stale; mounted queries refetch ([api/invalidate.ts:4](../frontend-react/src/api/invalidate.ts#L4)) | Bump a `version` signal that the resource `track()`s ([books.rs:34](../frontend-leptos/src/pages/books.rs#L34), [books.rs:103](../frontend-leptos/src/pages/books.rs#L103)) |
| Caching across pages | Yes: shared cache, stale-while-revalidate, retry policy ([queryClient.ts:30](../frontend-react/src/queryClient.ts#L30)) | No cache; each page owns its resources. Simpler, and more refetching |

"Local" in `LocalResource` means the future does not have to be `Send`; the
browser is single-threaded and `gloo-net`'s futures are not `Send`.

The HTTP clients do the same job. React:
[`request()` in api/client.ts:59](../frontend-react/src/api/client.ts#L59).
Leptos: `execute()` in [api/mod.rs:108](../frontend-leptos/src/api/mod.rs#L108),
with typed helpers (`get_json`, `send_json`) and per-resource modules such as
[api/loans.rs](../frontend-leptos/src/api/loans.rs). A subtle difference: the
Leptos client sends and receives the *same Rust structs the server uses*
(`NewLoan`, `Page<Book>`) through `serde`; React trusts that the generated
TypeScript types match, and the `as T` cast in
[client.ts:75](../frontend-react/src/api/client.ts#L75) is where that trust
lives.

### Routing

Both use a nested route table with a pathless "guard" parent.

- React Router, a data structure of `RouteObject`s ([App.tsx:15](../frontend-react/src/App.tsx#L15)); the guarded group is `{ element: <RequireAuth />, children: [...] }` (line 23).
- `leptos_router`, written as components ([app.rs:22](../frontend-leptos/src/app.rs#L22)); the guarded group is `<ParentRoute path=path!("") view=RequireAuth>` (line 33), and paths are checked by the `path!` macro.

### Forms and field errors

The API answers a bad form with `422` and `fields: { title: "must not be
empty" }`. Both frontends show each message under its input.

- React: controlled inputs (`useState` + `onChange`) in
  [components/BookForm.tsx:41](../frontend-react/src/components/BookForm.tsx#L41);
  the error comes from `fieldErrorsOf(mutation.error)`
  ([api/client.ts:79](../frontend-react/src/api/client.ts#L79)); submission is a
  `useMutation` ([pages/LoginPage.tsx:17](../frontend-react/src/pages/LoginPage.tsx#L17)).
- Leptos: an `RwSignal` per input with `prop:value=` and `on:input=` (the
  hand-written two-way binding) and `spawn_local` to run the async call
  ([pages/login.rs:25-49](../frontend-leptos/src/pages/login.rs#L25),
  inputs at [login.rs:62-78](../frontend-leptos/src/pages/login.rs#L62)). Field
  messages come from `ApiRequestError::field(name)`
  ([api/mod.rs:63](../frontend-leptos/src/api/mod.rs#L63)).

Both forms need `noValidate` / `novalidate` (see gotcha e below).

### Auth guard

The same behaviour, quite different code. When the user is logged out and
visits `/members?x=1`, both end up at `/login?next=%2Fmembers%3Fx%3D1`.

React ([auth/RequireAuth.tsx:6](../frontend-react/src/auth/RequireAuth.tsx#L6)):

```tsx
const { user, isLoading, error } = useCurrentUser()
const location = useLocation()
if (isLoading) return <p aria-busy="true">Loading…</p>
if (user === null) {
  return <Navigate to={`/login?next=${encodeURIComponent(location.pathname + location.search)}`} replace />
}
return <Outlet />
```

Leptos ([auth.rs:100-137](../frontend-leptos/src/auth.rs#L100)) builds a reactive
`move ||` block that re-runs when `auth.ready` / `auth.user` change, reads the
location with `get_untracked`, and renders `<Redirect>`; the pure helper
`login_redirect_target` ([auth.rs:86](../frontend-leptos/src/auth.rs#L86)) is
unit tested.

A global "any 401 means the session is gone" rule exists in both:
React in [queryClient.ts:15](../frontend-react/src/queryClient.ts#L15)
(a `QueryCache`/`MutationCache` `onError` that clears the user and navigates);
Leptos in `notify_unauthorized` ([api/mod.rs:90](../frontend-leptos/src/api/mod.rs#L90))
calling the handler registered by `AuthProvider`
([auth.rs:54](../frontend-leptos/src/auth.rs#L54)), which clears the user and,
like React, navigates to `/login?next=<current path>` (also from public pages;
when a `RequireAuth` guard is mounted it does the redirect itself, so the
handler skips it to avoid redirecting twice). Login and `me` are exempt in
both, because a 401 there is a normal answer.

Logout is the same story in a different shape. React clears the whole query
cache and navigates ([components/Layout.tsx:12](../frontend-react/src/components/Layout.tsx#L12)).
Leptos has to coordinate with the mounted guard
([components/layout.rs:16-27](../frontend-leptos/src/components/layout.rs#L16),
[auth.rs:123](../frontend-leptos/src/auth.rs#L123)), see gotcha c.

### Error handling

Both decode the backend's `ApiError` envelope into one error type carrying
`status`, `code`, `message` and `fields`: `ApiRequestError` in
[client.ts:4](../frontend-react/src/api/client.ts#L4) (a class) and
[api/mod.rs:26](../frontend-leptos/src/api/mod.rs#L26) (a struct implementing
`Display`). Non-JSON error bodies (a proxy's HTML 502) fall back to a generic
message in both. Transport failures become a status-0 error in Leptos
([api/mod.rs:34](../frontend-leptos/src/api/mod.rs#L34)); in React a failed
`fetch` rejects with the browser's `TypeError`, which `<ErrorMessage>` shows
as its own message.

Rust's type system adds one thing here: `ApiResult<T> = Result<T, ApiRequestError>`
([api/mod.rs:74](../frontend-leptos/src/api/mod.rs#L74)) makes the failure path
part of every function's signature. In TypeScript a function that throws looks
identical to one that does not.

### Build and tooling

| | React | Leptos |
|---|---|---|
| Tooling | Node 22, npm, Vite 8 | Rust toolchain, Trunk, `wasm32-unknown-unknown` target |
| Dev server | `npm run dev`, port 5173, hot module replacement keeps component state | `trunk serve`, port 8080, recompiles to WASM and reloads the page |
| Proxy to API | [vite.config.ts:12](../frontend-react/vite.config.ts#L12) | [Trunk.toml](../frontend-leptos/Trunk.toml) |
| Type check | `tsc -b` | the compiler (every `cargo clippy --target wasm32-unknown-unknown`) |
| Lint | **oxlint** (what the current Vite React template ships), `--deny-warnings` | clippy with `-D warnings` |
| Tests | Vitest + Testing Library: 58 tests in 13 files, rendering components in jsdom | 22 `#[test]`s of pure logic (URL parsing, redirect target, error decoding). The UI is not unit-tested |
| Production build time here | `npm run build`: about 2 s | `trunk build --release`: about 10 s after touching one file, about 1 minute from a cold dependency cache |
| Source size (excluding tests and generated code) | about 1,500 lines of TS/TSX | about 2,500 lines of Rust, including its inline unit tests |

React's feedback loop is faster, and its testing story for UI is more
mature. Leptos's compile-time guarantees are stronger (shared types,
exhaustive `match`, no `undefined`), but you pay in compile times and
framework-specific rules, such as the ones below.

## Bundle size

Measured on this repo's release builds, 2026-10-10. Sizes are bytes of the
files in `dist/`; gzip is `gzip -9` of each file (a close approximation of what
a compressing server sends; the backend's `CompressionLayer` gzips responses).

React: `cd frontend-react && npm run build` (Vite, minified, tree-shaken).
Leptos: `cd frontend-leptos && trunk build --release` (`opt-level` 3 release
profile, no extra size tuning in `Cargo.toml`, `wasm-opt -Oz` requested in
[index.html](../frontend-leptos/index.html) and run by Trunk).

| File | Raw | Gzip |
|---|---:|---:|
| **React** `assets/index-*.js` (app + React + Router + TanStack Query) | 371,351 | 113,786 |
| React `assets/index-*.css` (Pico CSS + app.css) | 85,429 | 11,585 |
| **React total** | **456,780** | **125,371** |
| **Leptos** `*_bg.wasm` (app + Leptos + serde + std pieces) | 1,281,699 | 429,136 |
| Leptos `*.js` (wasm-bindgen glue) | 48,125 | 8,307 |
| Leptos `pico.min-*.css` | 83,319 | 11,647 |
| Leptos `app-*.css` | 652 | 354 |
| **Leptos total** | **1,413,795** | **449,444** |

The CSS is the same Pico CSS v2 in both, so the real difference is the code:
about 114 KB gzip of JavaScript against about 437 KB gzip of WebAssembly plus
glue, roughly 3.8 times as much. The whole Leptos app is about 3.6 times the
React app on the wire.

What the numbers do and do not say:

- A Rust/WASM app ships its own runtime pieces (the reactive system, the
  router, `serde` for JSON, formatting, allocation) inside the `.wasm`, whereas
  React's cost is mostly React plus libraries. Both grow with features, at
  different rates; two screens would not show the same ratio.
- Size tuning was not attempted. `[profile.release]` settings such as
  `opt-level = "z"`, `lto = true` and `codegen-units = 1` normally shrink WASM
  noticeably; this was not measured here, so no figure is claimed.
- WASM must be compiled by the browser before the app starts (the engine
  streams and compiles it, which is fast but not free); the JS bundle must be
  parsed and run. Time-to-interactive was not measured.
- Sizes are transfer sizes only: both apps are tiny next to typical page
  weight, and a library admin tool is not bandwidth sensitive.

## Gotchas we actually hit

All of these were found by browser smoke tests of the Leptos app and fixed
(reports and commits: `fix/leptos-integration`). Each has a React contrast.

### a) A `>` inside a `view!` attribute closes the tag

In Leptos's `view!` macro an attribute such as `disabled=page >= pages` ends
the tag at the `>`: the rest becomes text in the page, and the button rendered
with a garbage label. Wrapping the expression in **a `move ||` closure does not
help** (it still contains the `>`; the browser still showed the broken label).
Only **braces** do:

```rust
// frontend-leptos/src/components/pagination.rs:28-30
// `view!` parses `>` as the end of the tag, even inside a closure,
// so a comparison in an attribute must be wrapped in braces.
disabled={page >= pages}
```

(`page <= 1` on line 20 is braced for symmetry.) Audit tip: grep `view!`
blocks for comparison operators in attributes.

React contrast: JSX attribute values are always a `{ ... }` JavaScript
expression, so `disabled={page >= pageCount}`
([components/Pagination.tsx:23](../frontend-react/src/components/Pagination.tsx#L23))
is the *normal* syntax and `>` has no special meaning there. The braces Leptos
needs are the braces React always requires.

### b) Reading the location reactively in a guard made an infinite redirect

The first version of `RequireAuth` read the current URL with a tracked
`location.pathname.get()` inside the reactive block. After it redirected to
`/login?next=%2Fmembers`, the location changed, the block re-ran (it
subscribed to the location), found the user still logged out, and built a
**new** redirect with `next` pointing at the login URL:
`/login?next=%2Flogin%3Fnext%3D...`, forever (the URL grew past 1 MB and
Chrome hung). The fix has two parts:

- read the location with **`get_untracked`** so the guard captures "where the
  user was" exactly once, when they lost access
  ([auth.rs:129-132](../frontend-leptos/src/auth.rs#L129) and the comment above it);
- build the target in a pure function, `login_redirect_target`
  ([auth.rs:86](../frontend-leptos/src/auth.rs#L86)), which refuses to nest a
  `/login` URL and re-adds the `?` that Leptos's `location.search` leaves out.
  It has unit tests ([auth.rs:143-158](../frontend-leptos/src/auth.rs#L143)).

React contrast: `<Navigate>` is rendered once and runs its redirect in an
effect; after it navigates, `RequireAuth` is unmounted because the `/login`
route is outside the guard
([App.tsx:22-24](../frontend-react/src/App.tsx#L22)), so nothing re-runs.
In Leptos the lesson is *which reads subscribe*: `.get()` subscribes the
surrounding closure, `.get_untracked()` does not. In React everything you read
during render re-runs on every render, so this class of loop looks different
(a missing dependency in a `useEffect` array is the usual cause).

### c) Logout raced the guard

Logging out clears the user, and `RequireAuth` reacts to "no user" by
redirecting to `/login?next=<current page>`. So logging out from a protected
page redirected to the login form (with an extra history entry) instead of
landing on `/books`. Navigating first and clearing the user after a 0 ms timer
relied on timing. The deterministic fix is a flag: Layout sets `logging_out` (only when a
guard is actually mounted) before clearing the user; the guard sees the flag,
renders nothing, resets it, and does not redirect
([layout.rs:16-27](../frontend-leptos/src/components/layout.rs#L16),
[auth.rs:21-25](../frontend-leptos/src/auth.rs#L21),
[auth.rs:123-127](../frontend-leptos/src/auth.rs#L123)). The guard also resets
the flag in `on_cleanup` so it can never get stuck
([auth.rs:105-109](../frontend-leptos/src/auth.rs#L105)). Verified by recording
`history.pushState` calls: logout produces exactly one, to `/books`.

React contrast: [Layout.tsx:12-20](../frontend-react/src/components/Layout.tsx#L12)
does `queryClient.clear()`, sets the cached user to `null` and navigates in
`onSettled`; the 401 handler is told to ignore this request with
`meta: { skipUnauthorizedRedirect: true }`. React Router's `Navigate`-based guard did not show this race in the
smoke test, so no flag was needed.

### d) `<select prop:value=...>` before the options exist

Setting `prop:value` on a `<select>` does nothing on first render because
properties are applied before the child `<option>`s are created, so the value
matches no option. Put a reactive **`selected=`** on each `<option>` instead:

```rust
// frontend-leptos/src/pages/books.rs:119-122
// `selected` on each option (not `prop:value` on the select):
// props are applied before children exist...
<option value="" selected=move || availability.get() == Availability::All>
```

The same technique makes the new-loan pickers work when options arrive after a
fetch ([pages/loan_new.rs:241-258](../frontend-leptos/src/pages/loan_new.rs#L241)).

React contrast: `<select value={x}>` is handled by React itself, which applies
the value after the options are in place
([components/Picker.tsx:41-55](../frontend-react/src/components/Picker.tsx#L41)).

### e) Forms need `novalidate` so server field errors show

The backend validates and answers `422` with per-field messages. A browser
validates first when inputs carry attributes like `required` or `type="email"`
and shows its own bubble, so the server's message (the one the product wants)
never appears and the request is never sent. Both frontends therefore turn
native validation off. Leptos had forgotten (the smoke test found it); the fix
is in all four forms
([login.rs:59](../frontend-leptos/src/pages/login.rs#L59),
[member_form.rs:100](../frontend-leptos/src/pages/member_form.rs#L100),
[book_form.rs:179](../frontend-leptos/src/pages/book_form.rs#L179),
[loan_new.rs:157](../frontend-leptos/src/pages/loan_new.rs#L157)).

React contrast: the same fix, spelled `noValidate`
([LoginPage.tsx:36](../frontend-react/src/pages/LoginPage.tsx#L36),
[BookForm.tsx:40](../frontend-react/src/components/BookForm.tsx#L40),
[MemberForm.tsx:26](../frontend-react/src/components/MemberForm.tsx#L26),
[NewLoanPage.tsx:85](../frontend-react/src/pages/NewLoanPage.tsx#L85)).
Not a framework issue: it is a consequence of doing validation on the server.

### Smaller ones

- **Leptos `location.search` has no leading `?`**, so a naive `format!("{path}{search}")` produced `/membersx=1`. The helper re-adds it.
- **Reset `busy` before `navigate`.** After navigating, the page's reactive owner may already be disposed; setting its signals afterwards is unreliable ([login.rs:38-40](../frontend-leptos/src/pages/login.rs#L38)).
- **`expect_context` panics** if no provider is above you ([auth.rs:30](../frontend-leptos/src/auth.rs#L30)). In React `useContext` just returns `undefined`/default. A WASM panic stops the app, which is why the comment calls it a startup invariant.
- **Back button.** Paging pushes a history entry and search/filter changes replace it, in both apps, so Back returns to the previous page rather than to a stale search ([books.rs:51-53](../frontend-leptos/src/pages/books.rs#L51)).

## Takeaways

- **Same architecture, different mental model.** React re-runs component
  functions and diffs a virtual DOM; Leptos runs each component once and wires
  signals directly to DOM nodes. In React the common bugs are stale closures
  and effect dependencies; in Leptos they are about *what subscribes to what*
  (gotcha b) and macro parsing (gotcha a).
- **Types across the wire are the big Rust win.** One `NewLoan` struct is
  used by the backend, by Leptos, and is the source for React's generated
  TypeScript. Nothing like `as T` exists on the Leptos side.
- **Ecosystem maturity is the big React win.** TanStack Query (caching,
  invalidation, retries), Testing Library, hot reload and a 125 KB gzip bundle
  came almost free; the Leptos equivalents are hand-rolled
  (`version` signals, thread-local 401 handler) or smaller.
- **Both frontends are tested against the same backend and the same smoke
  checklist.** That, not the framework, is what kept them identical.
