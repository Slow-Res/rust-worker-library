# Library Web App — Implementation Plan

Spec (binding authority): `docs/superpowers/specs/2026-10-10-library-web-app-design.md`
Repo root: `/home/slowres/rust/library-web-app`

## Global Constraints

- The spec is the contract. Section numbers below (e.g. "spec §5.6") point
  into it — read the referenced sections before coding. Do not invent
  endpoints, fields, error codes, or behavior not in the spec; if the spec is
  silent or contradictory, stop and report NEEDS_CONTEXT.
- Toolchain: Rust 1.96 stable, Node 22 / npm 10, sqlx-cli 0.9 (`sqlx`),
  Trunk 0.21 (`trunk`), target `wasm32-unknown-unknown` installed.
- Versions: spec §4. Before using an API you are unsure of for these exact
  versions (axum 0.8, sqlx 0.9, tower-http 0.7, leptos 0.8, ts-rs 12,
  argon2 0.6), check the crate source in `~/.cargo/registry/src/` or docs.rs —
  several of these changed APIs recently (e.g. sqlx 0.9 `SqlSafeStr`, axum 0.8
  `{param}` path syntax).
- Code quality: spec §5.12. All host Rust code must pass
  `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings`.
  No `unwrap`/`expect` outside tests (startup invariants excepted, with a comment).
- Teaching comments: explain each Rust/Axum idiom briefly the first time it
  appears; module `//!` docs on every module; no noise comments.
- Layering rules: spec §5.1 (routes never touch SQL; services never touch
  axum/sqlx; repo traits mention neither).
- Developer DB for SQLx macros: `DATABASE_URL=sqlite://data/dev.db` in `.env`
  (git-ignored). After changing any `query!` macro, run
  `cd backend && cargo sqlx prepare` and commit `backend/.sqlx/`.
  `SQLX_OFFLINE=true cargo build -p library-backend` must succeed.
- Commit with clear conventional messages (`feat(backend): ...`). Commit only
  files your task owns. Never commit `.env`, `data/`, `target/`, `node_modules/`, `dist/`.
- Do not modify files owned by other tasks except where your task says so.

## Task 1: Workspace scaffold, shared crate, backend foundation

**Goal:** A compiling Cargo workspace with the `library-shared` crate (all API
DTOs, spec §6) and the backend foundation: config, telemetry, DB pool +
migrations, errors, extractors, clock, state, pagination, router with
middleware and static serving, health endpoint, CLI skeleton, graceful
shutdown, and the integration-test harness.

**Create:**
- Root: `Cargo.toml` (workspace; `members = ["shared", "backend"]` for now,
  `default-members` same; `resolver = "3"`; `[workspace.package]` edition 2024,
  license MIT; `[workspace.dependencies]` for shared deps; `[workspace.lints]`
  per spec §5.12), `rust-toolchain.toml`, `rustfmt.toml`
  (`edition = "2024"`, keep defaults otherwise), `clippy.toml`,
  `.cargo/config.toml` (`TS_RS_EXPORT_DIR` → `frontend-react/src/generated`,
  `relative = true`), `.gitignore` (target, data, .env, node_modules, dist,
  `.superpowers/`), `.env.example` (all spec §5.3 vars with defaults, plus the
  dev `DATABASE_URL`), `Makefile` (targets: `db` = create+migrate dev DB with
  sqlx-cli, `dev-backend`, `test`, `lint`, `fmt`, `types`, `sqlx-prepare`,
  `check` = fmt-check + clippy + test + offline build).
- `shared/`: `Cargo.toml`, `src/lib.rs` (module per area: `auth`, `books`,
  `members`, `loans`, `common` with `Page`, `ApiError`, `Health`), re-exported
  at crate root. ts-rs is an **optional** dependency behind feature `ts`;
  derive via `#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]` so the
  WASM frontend does not compile ts-rs. Integer fields
  `#[cfg_attr(feature = "ts", ts(type = "number"))]`. Unit tests: JSON
  round-trip for one type per module, enum serialization (`"on_loan"`,
  `"overdue"`), and that `ApiErrorBody` omits `fields` when `None`.
  Generate TS: `cargo test -p library-shared --features ts`, commit
  `frontend-react/src/generated/*.ts`.
- `backend/`: `Cargo.toml` (package `library-backend`, `[lib]` + `[[bin]]`
  named `library-backend`), `migrations/20261010000000_init.sql` (exact schema
  from spec §5.4), and `src/` files: `main.rs`, `lib.rs`, `cli.rs`,
  `config.rs`, `telemetry.rs`, `db.rs`, `error.rs`, `clock.rs`, `state.rs`,
  `extract.rs`, `pagination.rs`, `app.rs`, `server.rs`, `health.rs`.
  `clock.rs` provides `trait Clock: Send + Sync { fn now(&self) -> DateTime<Utc>; }`,
  `SystemClock`, and a public `FixedClock` (interior-mutable, `set`/`advance`)
  documented as a test helper. `db.rs` provides `connect(&Config) -> SqlitePool`
  (pragmas from spec §5.4, creates parent dir), `migrate(&pool)`,
  `RepoError` (spec §5.5) and `impl From<sqlx::Error> for RepoError`
  (unique-violation → `Conflict { constraint }`), plus
  `to_timestamp(DateTime<Utc>) -> i64` / `from_timestamp(i64) -> Result<DateTime<Utc>, RepoError>`.
  `AppState` for now holds `pool`, `config: Arc<Config>`, `clock: Arc<dyn Clock>`;
  constructor `AppState::new(pool, config, clock)`. Later tasks add services.
  `cli.rs`: clap with `serve` (default) and placeholder-free structure ready
  for `create-user` and `seed` (add those subcommands in later tasks — do not
  stub them now).
- `backend/tests/common/mod.rs`: `TestApp { router, clock: Arc<FixedClock>, pool }`
  built from a `#[sqlx::test]` pool; helpers `get`, `post_json`, `put_json`,
  `delete`, each returning `(StatusCode, HeaderMap, serde_json::Value)`
  (empty body → `Value::Null`), plus an optional cookie argument.
- `backend/tests/health.rs`: health 200 `{"status":"ok"}`; unknown `/api/nope`
  → 404 JSON with `code = "not_found"`; malformed JSON body handling is
  covered in later tasks.

**Unit tests:** `pagination.rs` (defaults, clamping, offset math),
`error.rs` (status + JSON body for each variant; `Internal` does not leak the
inner message; `fields` omitted when empty).

**Verify:**
```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p library-shared --features ts && git status --short frontend-react/src/generated
cd backend && cargo sqlx prepare && cd .. && SQLX_OFFLINE=true cargo build -p library-backend
cargo run -p library-backend   # then: curl -i localhost:3000/api/health ; curl -i localhost:3000/api/nope ; Ctrl-C shuts down cleanly
```
(If no `query!` macros exist yet, `cargo sqlx prepare` may produce nothing —
that is fine; make sure `backend/.sqlx/` exists with a `.gitkeep` if empty.)

## Task 2: Authentication

**Goal:** Librarian login/logout/me, session cookies, `AuthUser` extractor,
and the `create-user` CLI subcommand. Spec §5.7, §5.8 (auth rows), §5.10.

**Create/modify:**
- `backend/src/auth/mod.rs`, `repo.rs` (`#[async_trait] trait AuthRepository`:
  `create_user`, `find_user_by_username` → `Option<UserRecord { id, username, password_hash }>`,
  `create_session`, `find_session_user(token_hash, now)` → `Option<SessionUser { id, username }>`,
  `delete_session(token_hash)`, `delete_expired_sessions(now)`),
  `sqlite.rs` (`SqliteAuthRepository`), `password.rs` (`hash_password`,
  `verify_password` — both async via `spawn_blocking`; dummy-hash constant
  for unknown users), `service.rs` (`AuthService`: `create_user`, `login` →
  `LoginSession { user: CurrentUser, token: String, expires_at }`, `logout`,
  `authenticate(token)`), `extractor.rs` (`AuthUser` via `FromRequestParts`
  using `FromRef` to reach `AuthService`), `routes.rs` (login, logout, me;
  cookie built with axum-extra `CookieJar`/`Cookie`).
- `state.rs`: add `auth: AuthService` and `impl FromRef<AppState> for AuthService`.
- `app.rs`: mount `/api/auth` routes.
- `cli.rs` / `main.rs`: `create-user <USERNAME> [--password-stdin]` per spec §5.10.
- `tests/common/mod.rs`: `TestApp::login(username, password) -> cookie header value`
  (creates the user through `AuthService`, then calls `POST /api/auth/login`).

**Tests (`backend/tests/auth.rs`):** login success (200, body `CurrentUser`,
`Set-Cookie` has `library_session`, `HttpOnly`, `SameSite=Lax`, `Path=/`,
`Max-Age`); wrong password 401 `unauthorized`; unknown user 401 (same
message); empty fields 422; `me` with cookie 200, without 401, with garbage
token 401; expired session 401 (advance `FixedClock` past TTL); logout 204 then
`me` 401 with the same cookie; malformed JSON body → 400 `bad_request`;
username uniqueness is case-insensitive (service-level test). Unit tests:
password hash/verify round trip; token hashing is deterministic and the raw
token is 64 hex chars.

**Verify:** fmt, clippy, `cargo test --workspace`, `cargo sqlx prepare`
committed, offline build; manual: `cargo run -p library-backend -- create-user admin`
then curl login/me/logout with a cookie jar.

## Task 3: Books feature

**Goal:** Books CRUD + search/filter/pagination with ISBN validation and
the copy/active-loan invariants. Spec §5.6 (book outcomes), §5.8 (books rows,
book search, book validation).

**Create/modify:**
- `backend/src/books/mod.rs`, `domain.rs` (`Isbn` newtype with
  `Isbn::parse(&str) -> Result<Isbn, &'static str>` — ISBN-10/13 checksum;
  `BookFields` validated struct and `BookFields::parse(BookInput, current_year) -> Result<BookFields, FieldErrors>`
  collecting all field errors at once), `repo.rs` (`BookRepository`: `list(filter, page)`
  → `(Vec<BookRecord>, total)`, `get`, `create`, `update_checked`,
  `delete_checked`; `BookRecord` includes `available_copies`; outcome enums
  from spec §5.6), `sqlite.rs` (search with `QueryBuilder`, LIKE escaping per
  spec; transactions with `BEGIN IMMEDIATE`), `service.rs` (`BookService`:
  `list`, `get`, `create`, `update`, `delete`; maps `RepoError::Conflict` on
  the isbn index → 409 `isbn_taken`), `routes.rs` (public GETs; mutations
  require `AuthUser`; 201 on create, 204 on delete).
- `From<BookRecord> for shared::Book`.
- `state.rs` (+ `FromRef`), `app.rs` (mount `/api/books`).
- The loans table already exists (Task 1 migration): availability and
  active-loan checks query it directly in SQL.

**Tests:** `backend/tests/books.rs` — public list/get without cookie;
create requires auth (401); create 201 with normalized ISBN
(`"978-0-13-110362-7"` → `"9780131103627"`); invalid fields 422 with
`fields` for every bad field at once; duplicate ISBN 409 `isbn_taken`;
search by title/author/isbn substring, case-insensitive, `%`/`_` treated
literally; availability filter (insert loans via SQL in the test to set up
active loans); pagination (`total`, `page`, `per_page`, clamping); update 200,
404, reduce copies below active loans 409; delete 204, 404, with active loan
409 `book_has_active_loans`, with only returned loans 204 and loans gone.
Unit tests in `domain.rs`: valid/invalid ISBN-10 (incl. `X`), ISBN-13,
hyphens/spaces, wrong length, bad checksum; `BookFields::parse` edge cases
(trim, lengths, year bounds, copies bounds).

**Verify:** fmt, clippy, tests, `cargo sqlx prepare` committed, offline build.

## Task 4: Members feature

**Goal:** Members CRUD + search/pagination. Spec §5.6 (member delete), §5.8
(member rows, search, validation).

**Create/modify:** `backend/src/members/` with `mod.rs`, `domain.rs`
(`Email` newtype with parse/normalize; `MemberFields::parse(MemberInput)`),
`repo.rs` (`MemberRepository`: `list`, `get`, `create`, `update`,
`delete_checked`; `MemberRecord` includes `active_loans`), `sqlite.rs`,
`service.rs` (409 `email_taken` from the unique constraint), `routes.rs`
(all endpoints require `AuthUser`). `From<MemberRecord> for shared::Member`.
`state.rs`, `app.rs`. Follow the exact patterns established in `books/`.

**Tests:** `backend/tests/members.rs` — every endpoint requires auth (401);
create 201 (email lowercased/trimmed); 422 with field errors; duplicate email
409 (case-insensitive because stored lowercased); search by name/email;
pagination; `active_loans` count reflects active loans only; update 200/404/
409; delete 204/404/409 `member_has_active_loans`, cascade of returned loans.
Unit tests for `Email` and `MemberFields::parse`.

**Verify:** fmt, clippy, tests, `cargo sqlx prepare` committed, offline build.

## Task 5: Loans feature and seed command

**Goal:** Borrow, return, list/filter loans with the business rules; plus the
`seed` CLI subcommand. Spec §2, §5.6 (loan outcomes), §5.8 (loan rows, loan
filters), §5.10 (seed).

**Create/modify:**
- `backend/src/loans/` with `mod.rs` (constants `LOAN_PERIOD_DAYS = 14`,
  `MAX_ACTIVE_LOANS_PER_MEMBER = 5`), `repo.rs` (`LoanRepository`: `list(filter, now, page)`,
  `get`, `create_checked`, `mark_returned`; `LoanDetails` record joined with
  book title + member name; outcome enums per spec §5.6), `sqlite.rs`
  (`BEGIN IMMEDIATE` transactions; `overdue` filter uses the bound `now`),
  `service.rs` (`LoanService` with `Arc<dyn LoanRepository>` + `Arc<dyn Clock>`;
  computes `due_at`, `is_overdue`; maps outcomes: `BookNotFound` → 404 "book not found",
  `MemberNotFound` → 404 "member not found", `NoCopiesAvailable` → 409,
  `LoanLimitReached` → 409, `AlreadyReturned` → 409), `routes.rs` (auth required).
- Service unit tests using an **in-memory fake `LoanRepository`** (defined in
  the test module) + `FixedClock`: due date = now + 14 days exactly; each
  outcome maps to the right `AppError`; `is_overdue` true/false at the
  boundary (due_at == now is not overdue).
- `backend/src/seed.rs` + `seed` subcommand: ~12 books (real titles with
  valid ISBNs, varied `total_copies` 1–3), ~6 members, and 3 loans (one
  overdue: borrowed 20 days before now) — only when books and members are
  both empty; otherwise print "database already has data; skipping". Inserts
  go through the repositories (not raw SQL) so validation-equivalent data is
  guaranteed; use `BookFields::parse`/`MemberFields::parse` to build inputs.
- `state.rs`, `app.rs`, `cli.rs`/`main.rs`.

**Tests:** `backend/tests/loans.rs` — auth required; borrow 201 with
`due_at = borrowed_at + 14d`, `book_title`, `member_name`; book's
`available_copies` decreases and member's `active_loans` increases; unknown
book 404 / unknown member 404; no copies 409 `no_copies_available`; 6th
active loan 409 `loan_limit_reached`; return 200 sets `returned_at`, second
return 409 `loan_already_returned`, unknown 404; list filters
`active`/`overdue`/`returned`/`all` (advance `FixedClock` 15 days to make a
loan overdue), `member_id` and `book_id` filters, ordering per spec;
pagination. Seed: running seed logic twice leaves counts unchanged
(test the seed function directly against a `#[sqlx::test]` pool).

**Verify:** fmt, clippy, tests, `cargo sqlx prepare` committed, offline build;
manual: `cargo run -p library-backend -- seed` then curl `/api/books`.

## Task 6: React frontend

**Goal:** `frontend-react/` implementing the full UX contract in spec §7 using
the generated types in `frontend-react/src/generated/` (do not edit them; do
not modify any Rust code).

**Create:** Vite React-TS app (`npm create vite@latest` equivalent, then
trim boilerplate), `vite.config.ts` (port 5173, proxy `/api` →
`http://127.0.0.1:3000`), strict `tsconfig`, ESLint config from template,
Vitest + Testing Library + jsdom setup, `src/` structure from spec §7, Pico
CSS imported once, `src/app.css` minimal. `package.json` scripts: `dev`,
`build` (`tsc -b && vite build`), `lint`, `typecheck`, `test` (`vitest run`),
`preview`.

**Behavior details:**
- `api/client.ts`: `request<T>(method, path, {query?, body?})` using
  `fetch` with `credentials: "same-origin"`, JSON headers; non-2xx → throws
  `ApiRequestError { status, code, message, fields }` parsed from the
  `ApiError` body (fallback message if body is not JSON); 204 → `undefined`.
  Query objects are serialized skipping `undefined`/empty values.
- A global 401 handler (TanStack Query `QueryCache`/`MutationCache` `onError`)
  clears the current user and navigates to `/login?next=...` — except for
  the `me` query and the login mutation themselves.
- Query keys: `["me"]`, `["books", query]`, `["book", id]`, `["members", query]`,
  `["member", id]`, `["loans", query]`. Mutations invalidate the affected
  lists (e.g. borrowing invalidates books, members, loans).

**Tests (Vitest):** `api/client.test.ts` (error parsing, 204, query
serialization); `components/Pagination.test.tsx`; one page test
(`LoginPage` shows the API error message on 401, using a mocked `fetch`).

**Verify:** `npm ci` (or `npm install` first time), `npm run lint`,
`npm run typecheck`, `npm test`, `npm run build`. Manual smoke with the
backend running + seeded: log in, search books, lend, return, overdue tab.

## Task 7: Leptos frontend

**Goal:** `frontend-leptos/` implementing the same UX contract (spec §7) in
Leptos 0.8 CSR, using `library-shared` types directly. Mirror the React
structure file-for-file where it makes sense. Do not modify `shared/`,
`backend/`, or `frontend-react/`.

**Create:** `frontend-leptos/Cargo.toml` (package `library-frontend-leptos`,
`library-shared` path dep without the `ts` feature, workspace lints),
`index.html` (Trunk: `<link data-trunk rel="rust" data-wasm-opt="z" />`,
`<link data-trunk rel="css" href="assets/pico.min.css" />`, app css),
`Trunk.toml` (port 8080, proxy `/api` → `http://127.0.0.1:3000/api`),
`assets/pico.min.css` (Pico v2, same version as React's, copied from the npm
package or its CDN `https://cdn.jsdelivr.net/npm/@picocss/pico@2/css/pico.min.css`),
`assets/app.css` (same rules as React's `app.css`), `src/` per spec §7.
Add `"frontend-leptos"` to workspace `members` (NOT `default-members`) in the
root `Cargo.toml`.

**Behavior details:** same as Task 6: `api/mod.rs` with
`ApiRequestError { status, code, message, fields }` decoded from
`shared::ApiError`; query strings via `serde_urlencoded` from the shared query
structs; current user in a context (`RwSignal<Option<CurrentUser>>` loaded
from `/api/auth/me` at startup); 401 → clear user + navigate to
`/login?next=...`; debounced search (300 ms); resources refetch after
mutations.

**Tests:** native unit tests for pure helpers (query-string building, date
formatting, pagination math) — `cargo test -p library-frontend-leptos`.

**Verify:** `cargo clippy -p library-frontend-leptos --target wasm32-unknown-unknown -- -D warnings`,
`cargo test -p library-frontend-leptos`, `cd frontend-leptos && trunk build --release`,
and the root gates still pass (`cargo clippy --workspace --all-targets -- -D warnings`
must not break — if the wasm crate cannot build for the host, exclude it
from host clippy in the Makefile/CI and explain in the report). Manual smoke
like Task 6 on port 8080.

## Task 8: Docs, production packaging, CI

**Goal:** Everything in spec §8 that is not code: README, ARCHITECTURE,
REACT-VS-LEPTOS, Makefile completion, Dockerfile, .dockerignore, CI.

**Create/modify:** `README.md`, `docs/ARCHITECTURE.md`,
`docs/REACT-VS-LEPTOS.md`, `Makefile` (add `setup`, `dev-react`,
`dev-leptos`, `seed`, `user`, `build`; `check` also runs React lint/
typecheck/test and Leptos wasm clippy), `Dockerfile`, `.dockerignore`,
`.github/workflows/ci.yml` (also verifies `frontend-react/src/generated` is up
to date: regenerate then `git diff --exit-code`).

**Requirements:**
- ARCHITECTURE.md references real code with `path:line` — open the files and
  use the actual current line numbers. Trace "Borrow a book" from the React
  button click through `POST /api/loans` down to the SQL and back.
- REACT-VS-LEPTOS.md compares the same features with file references to
  both frontends, and includes measured release bundle sizes (gzip) for both.
- README quick start must work verbatim on a fresh clone (verify it).
- Docker image builds and runs: `docker build -t library-web-app .` and
  `docker run -p 3000:3000 -v library-data:/data library-web-app`, health
  endpoint OK (if Docker is unavailable, say so in the report; still keep the
  Dockerfile correct by inspection).

**Verify:** `make check` passes; README steps executed; links/paths in docs exist.
