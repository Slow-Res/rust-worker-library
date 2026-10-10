# Library Web App — Design Spec

Date: 2026-10-10
Status: approved by owner ("build all of it")

## 1. Purpose

A library management web app built as a **learning project for Rust web
development**, written to production-grade patterns and code quality. The
owner is new to Rust web, so the code must be idiomatic, readable, and
explain Rust/Axum idioms the first time they appear.

One Rust backend (JSON API over SQLite) is consumed by **two functionally
identical frontends**: React (TypeScript) and Leptos (Rust → WebAssembly), so
the owner can compare the two approaches side by side.

## 2. Product rules

| Rule | Value |
|---|---|
| Loan period | 14 days (`LOAN_PERIOD_DAYS = 14`) |
| Max active loans per member | 5 (`MAX_ACTIVE_LOANS_PER_MEMBER = 5`) |
| Copies | Each book has `total_copies` (1..=1000). `available_copies = total_copies - active loans`. A loan needs `available_copies >= 1`. |
| Overdue | A loan is overdue when `returned_at IS NULL AND due_at < now`. |
| Deleting | A book or member with **active** loans cannot be deleted (409). Deleting one with only returned loans also deletes those loan records (ON DELETE CASCADE). |
| Reducing copies | `total_copies` cannot be set below the book's current active loan count (409). |
| Returning | A loan can be returned once; returning again is 409. |
| Public access | `GET /api/books`, `GET /api/books/{id}`, `GET /api/health`, `POST /api/auth/login` are public. Everything else requires a logged-in librarian. |
| Librarian accounts | Created only from the CLI (`create-user`). No sign-up endpoint. All librarians have equal rights (no roles). |

## 3. Repository layout

```
library-web-app/
├── Cargo.toml              # workspace: members shared, backend, frontend-leptos; default-members shared, backend
├── Cargo.lock
├── rust-toolchain.toml     # channel = "stable", targets = ["wasm32-unknown-unknown"]
├── rustfmt.toml
├── clippy.toml             # allow-unwrap-in-tests = true, allow-expect-in-tests = true
├── .cargo/config.toml      # [env] TS_RS_EXPORT_DIR -> frontend-react/src/generated (relative = true)
├── Makefile
├── .env.example
├── .gitignore
├── shared/                 # crate `library-shared`: API DTOs (serde + ts-rs), no IO
├── backend/                # crate `library-backend`: Axum + SQLx (lib + bin)
│   ├── migrations/
│   ├── .sqlx/              # SQLx offline query metadata (committed)
│   ├── src/
│   └── tests/
├── frontend-react/         # Vite + React + TypeScript
├── frontend-leptos/        # crate `library-frontend-leptos`: Leptos CSR, built with Trunk
└── docs/
    ├── ARCHITECTURE.md
    └── REACT-VS-LEPTOS.md
```

## 4. Technology (pin these major versions)

Backend: `axum 0.8` (features `macros`), `tokio 1` (`full`), `sqlx 0.9`
(`runtime-tokio`, `sqlite`, `chrono`, `migrate`, `macros`), `tower 0.5`,
`tower-http 0.7` (`trace`, `fs`, `timeout`, `request-id`, `set-header`,
`limit`, `compression-gzip`, `util` as needed), `axum-extra 0.12` (`cookie`),
`serde 1`, `serde_json 1`, `thiserror 2`, `anyhow 1`, `tracing 0.1`,
`tracing-subscriber 0.3` (`env-filter`, `json`), `chrono 0.4` (`serde`),
`argon2 0.6`, `rand` (OS RNG for tokens), `sha2` + `hex` (token hashing),
`async-trait`, `clap 4` (`derive`, `env`), `dotenvy`, `rpassword`.
Dev: `http-body-util`, `tower` (`util`).

Shared: `serde`, `chrono` (`serde`), `ts-rs 12` (features for chrono and
serde compat). **Every integer field must export to TS `number`** (ts-rs maps
`i64`/`u64` to `bigint` by default — override with `#[ts(type = "number")]`).

React: latest stable Vite + React 19 + TypeScript (strict), `react-router`
(v7, declarative/library mode), `@tanstack/react-query` v5, `@picocss/pico` v2,
Vitest + Testing Library, ESLint (from the Vite template).

Leptos: `leptos 0.8` (`csr`), `leptos_router 0.8`, `gloo-net 0.7`
(`http`, `json`), `serde`, `serde_urlencoded`, `web-sys` (as needed),
`library-shared`. Built with **Trunk 0.21.x**. Pico CSS v2 vendored as
`frontend-leptos/assets/pico.min.css` (same version as React).

## 5. Backend architecture

### 5.1 Layers ("light hexagonal", per feature module)

```
routes.rs   HTTP only: extractors in, status + JSON out. Calls the service.
service.rs  Business rules + validation. Depends only on the repo TRAIT.
repo.rs     The port: `#[async_trait] pub trait XRepository: Send + Sync`.
sqlite.rs   The adapter: `SqliteXRepository { pool: SqlitePool }` implements the trait.
domain.rs   (where needed) domain types and pure validation (e.g. `Isbn`).
```

Rules:
- `routes.rs` never touches SQL; `service.rs` never touches HTTP types or
  `sqlx`; `repo.rs` mentions neither `sqlx` nor `axum`.
- Services hold `Arc<dyn XRepository>` (+ `Arc<dyn Clock>` where time matters).
- Repositories return `Result<_, RepoError>`; services return
  `Result<_, AppError>`.
- Multi-step invariants that must be atomic (see 5.6) are enforced **inside
  the adapter in one transaction** and reported back as a typed outcome enum;
  the service owns the policy values and maps outcomes to errors.

### 5.2 Source layout (`backend/src/`)

```
main.rs            # thin: load .env, parse CLI, init tracing, run subcommand
lib.rs             # module declarations + `pub use app::build_router`
cli.rs             # clap: `serve` (default), `create-user <USERNAME>`, `seed`
config.rs          # Config::from_env()
telemetry.rs       # tracing subscriber init (pretty or JSON)
db.rs              # connect(SqliteConnectOptions w/ pragmas) + migrate; RepoError + sqlx error mapping
error.rs           # AppError, IntoResponse, FieldErrors, rejection conversions
clock.rs           # trait Clock, SystemClock
state.rs           # AppState (+ FromRef sub-states) and its constructor from (pool, config, clock)
extract.rs         # ApiJson<T>, ApiQuery<T>, ApiPath<T> (rejections -> AppError)
pagination.rs      # PageParams normalization (page >= 1, per_page 1..=100, default 20) + offset
app.rs             # build_router(state): /api routes, middleware, static frontend fallback
server.rs          # serve(): bind, axum::serve, graceful shutdown (Ctrl-C + SIGTERM)
seed.rs            # sample data
health.rs          # GET /api/health
auth/   mod.rs routes.rs service.rs repo.rs sqlite.rs extractor.rs password.rs
books/  mod.rs routes.rs service.rs repo.rs sqlite.rs domain.rs
members/ mod.rs routes.rs service.rs repo.rs sqlite.rs domain.rs
loans/  mod.rs routes.rs service.rs repo.rs sqlite.rs
```

Each module starts with a `//!` doc comment saying what it is and which layer
it belongs to.

### 5.3 Configuration (env vars; `.env` loaded via dotenvy if present)

| Var | Default | Notes |
|---|---|---|
| `DATABASE_URL` | `sqlite://data/library.db` | Parent dir created if missing; file created if missing. |
| `BIND_ADDR` | `127.0.0.1:4000` | |
| `FRONTEND_DIR` | unset | If set, serve that directory as static files with SPA fallback to `index.html`. |
| `COOKIE_SECURE` | `false` | `true` in production (HTTPS). |
| `SESSION_TTL_HOURS` | `168` | 7 days. |
| `LOG_FORMAT` | `pretty` | `pretty` or `json`. |
| `RUST_LOG` | `info,library_backend=debug,tower_http=info` | |

Invalid config fails fast at startup with a clear message.

### 5.4 Database

SQLite with pragmas set on every connection: `foreign_keys = ON`,
`journal_mode = WAL`, `synchronous = NORMAL`, `busy_timeout = 5s`. Pool max 8
connections. Migrations embedded with `sqlx::migrate!()` and run at startup
(serve, create-user, seed). **Timestamps are stored as INTEGER Unix seconds
(UTC)** and converted to/from `chrono::DateTime<Utc>` in the adapter.

Static queries use the compile-time checked macros (`sqlx::query!`,
`query_as!`, `query_scalar!`); dynamic filters (search/pagination) use
`sqlx::QueryBuilder` with bound parameters only (never string-interpolate
user input). `.sqlx/` offline data is committed; `SQLX_OFFLINE=true` must
build. Developer DB for macros: `DATABASE_URL=sqlite://data/dev.db`.

Schema (single initial migration `backend/migrations/20261010000000_init.sql`):

```sql
CREATE TABLE users (
    id            INTEGER PRIMARY KEY,
    username      TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    password_hash TEXT    NOT NULL,
    created_at    INTEGER NOT NULL
);

CREATE TABLE sessions (
    token_hash TEXT    PRIMARY KEY,          -- hex(SHA-256(raw token)); raw token only in the cookie
    user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);

CREATE TABLE books (
    id             INTEGER PRIMARY KEY,
    title          TEXT    NOT NULL,
    author         TEXT    NOT NULL,
    isbn           TEXT,                      -- normalized digits (+ 'X'), NULL if unknown
    published_year INTEGER,
    total_copies   INTEGER NOT NULL CHECK (total_copies BETWEEN 1 AND 1000),
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL
);
CREATE UNIQUE INDEX idx_books_isbn ON books(isbn) WHERE isbn IS NOT NULL;

CREATE TABLE members (
    id         INTEGER PRIMARY KEY,
    name       TEXT    NOT NULL,
    email      TEXT    NOT NULL UNIQUE,      -- stored lowercased
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE loans (
    id          INTEGER PRIMARY KEY,
    book_id     INTEGER NOT NULL REFERENCES books(id)   ON DELETE CASCADE,
    member_id   INTEGER NOT NULL REFERENCES members(id) ON DELETE CASCADE,
    borrowed_at INTEGER NOT NULL,
    due_at      INTEGER NOT NULL,
    returned_at INTEGER
);
CREATE INDEX idx_loans_active_book   ON loans(book_id)   WHERE returned_at IS NULL;
CREATE INDEX idx_loans_active_member ON loans(member_id) WHERE returned_at IS NULL;
CREATE INDEX idx_loans_active_due    ON loans(due_at)    WHERE returned_at IS NULL;
```

### 5.5 Errors

`RepoError` (in `db.rs`, adapter-agnostic):
`Conflict { constraint: String }` (unique violation) | `Unexpected(anyhow::Error)`.

`AppError` (in `error.rs`, `thiserror`):

| Variant | Status | `code` |
|---|---|---|
| `Validation(FieldErrors)` | 422 | `validation_error` |
| `BadRequest(String)` | 400 | `bad_request` (extractor rejections) |
| `Unauthorized` | 401 | `unauthorized` |
| `NotFound(&'static str)` | 404 | `not_found` (message: "<entity> not found") |
| `Conflict { code: &'static str, message: String }` | 409 | the given code |
| `Internal(anyhow::Error)` | 500 | `internal` (message is generic; real error logged with `tracing::error!`, never sent to client) |

Response body (always JSON, matches `shared::ApiError`):

```json
{ "error": { "code": "validation_error", "message": "Some fields are invalid", "fields": { "title": "must not be empty" } } }
```

`fields` is omitted when empty. `FieldErrors` is a `BTreeMap<String, String>`
builder. Conflict codes used: `isbn_taken`, `email_taken`,
`no_copies_available`, `loan_limit_reached`, `book_has_active_loans`,
`member_has_active_loans`, `copies_below_active_loans`,
`loan_already_returned`. Unknown `/api/*` routes return the JSON 404.

Custom extractors `ApiJson<T>`, `ApiQuery<T>`, `ApiPath<T>` convert axum
rejections into `AppError::BadRequest` so every error has the same shape.
`ApiJson` is also the response wrapper (implements `IntoResponse`).

### 5.6 Atomic operations (adapter-side transactions)

SQLite has one writer; use `BEGIN IMMEDIATE` transactions
(`pool.begin_with("BEGIN IMMEDIATE")` or equivalent) for check-then-write:

- `LoanRepository::create_checked(NewLoanRecord { book_id, member_id, borrowed_at, due_at }, max_active_per_member) -> CreateLoanOutcome`
  = `Created(LoanDetails) | BookNotFound | MemberNotFound | NoCopiesAvailable | LoanLimitReached`
  (checks in that order).
- `LoanRepository::mark_returned(id, returned_at) -> ReturnLoanOutcome`
  = `Returned(LoanDetails) | NotFound | AlreadyReturned`.
- `BookRepository::update_checked(id, BookFields) -> UpdateBookOutcome`
  = `Updated(BookRecord) | NotFound | CopiesBelowActiveLoans`.
- `BookRepository::delete_checked(id) -> DeleteOutcome` and
  `MemberRepository::delete_checked(id) -> DeleteOutcome`
  = `Deleted | NotFound | HasActiveLoans`.

The service validates input, computes `due_at = now + 14 days`, passes policy
values in, and maps outcomes to `AppError`.

### 5.7 Authentication

- Passwords: Argon2id (`argon2` crate defaults), PHC string stored. Hash and
  verify run in `tokio::task::spawn_blocking` (CPU heavy). Min password length
  8 (enforced in `create-user`).
- Login with unknown username still performs a verification against a fixed
  dummy hash so timing does not reveal which usernames exist. Failure message
  is always "invalid username or password" (401).
- Session token: 32 random bytes from the OS RNG, hex-encoded (64 chars) in
  the cookie; DB stores `hex(sha256(token))`. Expiry = now + `SESSION_TTL_HOURS`.
  Expired sessions are deleted on each successful login.
- Cookie `library_session`: `HttpOnly`, `SameSite=Lax`, `Path=/`,
  `Max-Age` = TTL, `Secure` iff `COOKIE_SECURE`. Logout deletes the session
  row and sends a removal cookie. CSRF: mitigated by `SameSite=Lax` plus
  JSON-only mutating endpoints; documented in ARCHITECTURE.md.
- `AuthUser { id, username }` extractor (`FromRequestParts`) reads the cookie,
  looks up an unexpired session, else `AppError::Unauthorized`. Handlers that
  need auth take `AuthUser` as an argument; that is the only auth mechanism.

### 5.8 HTTP API (all JSON, prefix `/api`)

Pagination query params on list endpoints: `page` (default 1, min 1),
`per_page` (default 20, clamped to 1..=100). List responses are
`Page<T> { items, page, per_page, total }`.

| Method | Path | Auth | Body → Response |
|---|---|---|---|
| GET | `/api/health` | – | → 200 `{ "status": "ok" }` (runs `SELECT 1`; 503 `{status:"unavailable"}` if DB fails) |
| POST | `/api/auth/login` | – | `LoginRequest` → 200 `CurrentUser` + Set-Cookie; 401 |
| POST | `/api/auth/logout` | ✔ | → 204 + removal cookie |
| GET | `/api/auth/me` | ✔ | → 200 `CurrentUser`; 401 |
| GET | `/api/books` | – | `BookQuery` → 200 `Page<Book>` |
| POST | `/api/books` | ✔ | `BookInput` → 201 `Book`; 422; 409 `isbn_taken` |
| GET | `/api/books/{id}` | – | → 200 `Book`; 404 |
| PUT | `/api/books/{id}` | ✔ | `BookInput` → 200 `Book`; 404; 422; 409 `isbn_taken`/`copies_below_active_loans` |
| DELETE | `/api/books/{id}` | ✔ | → 204; 404; 409 `book_has_active_loans` |
| GET | `/api/members` | ✔ | `MemberQuery` → 200 `Page<Member>` |
| POST | `/api/members` | ✔ | `MemberInput` → 201 `Member`; 422; 409 `email_taken` |
| GET | `/api/members/{id}` | ✔ | → 200 `Member`; 404 |
| PUT | `/api/members/{id}` | ✔ | `MemberInput` → 200 `Member`; 404; 422; 409 `email_taken` |
| DELETE | `/api/members/{id}` | ✔ | → 204; 404; 409 `member_has_active_loans` |
| GET | `/api/loans` | ✔ | `LoanQuery` → 200 `Page<Loan>` |
| POST | `/api/loans` | ✔ | `NewLoan` → 201 `Loan`; 404 (book/member, message names which); 409 `no_copies_available`/`loan_limit_reached` |
| POST | `/api/loans/{id}/return` | ✔ | → 200 `Loan`; 404; 409 `loan_already_returned` |

Search/filter semantics:
- Books: `q` (trimmed; empty = none) matches title, author or isbn,
  case-insensitive substring (`LIKE ... ESCAPE '\'`, with `%`, `_`, `\`
  escaped). `availability`: `all` (default) | `available`
  (available_copies > 0) | `on_loan` (active loans > 0). Order: title ASC, id ASC.
- Members: `q` matches name or email. Order: name ASC, id ASC.
- Loans: `status`: `active` (default; returned_at NULL) | `overdue` (active
  and due_at < now) | `returned` | `all`; optional `member_id`, `book_id`.
  Order: active/overdue/all by due_at ASC, id ASC; returned by returned_at DESC, id DESC.

Validation (422, field names as in the DTO, messages short lowercase):
- Book: `title` trimmed 1..=300 chars; `author` trimmed 1..=200;
  `isbn` optional — empty/whitespace = none; strip spaces and hyphens; must be
  a valid ISBN-10 (last char may be `X`/`x`, checksum mod 11) or ISBN-13
  (checksum mod 10); stored normalized (uppercase X); `published_year`
  optional 1..=(current year + 1); `total_copies` 1..=1000.
- Member: `name` trimmed 1..=200; `email` trimmed, lowercased, ≤ 254 chars,
  exactly one `@`, non-empty local part, domain contains a `.` not at either end.
- Loan: `book_id`, `member_id` positive.
- Login: `username`, `password` non-empty (else 422).

### 5.9 Middleware and serving (`app.rs`)

Order (outermost first): request-id (`x-request-id`, set if missing, and
propagated to the response) → `TraceLayer` (span includes method, path,
request id) → `TimeoutLayer` 15s → compression → security headers
(`X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`,
`Referrer-Policy: same-origin`) → body limit 64 KiB. API router is nested at
`/api` with a JSON 404 fallback. If `FRONTEND_DIR` is set, everything else is
served by `ServeDir` with fallback to `index.html`.

Graceful shutdown on Ctrl-C and SIGTERM.

### 5.10 CLI (`library-backend`)

- `library-backend` / `library-backend serve` — run server.
- `library-backend create-user <USERNAME> [--password-stdin]` — prompts twice
  (hidden input via rpassword) unless `--password-stdin`; rejects < 8 chars or
  mismatch; 409-equivalent message if username exists; exit code non-zero on error.
- `library-backend seed` — inserts ~12 sample books (real, valid ISBNs, mix of
  copies) and ~6 members, only if both tables are empty (prints what it did);
  idempotent.

### 5.11 Testing

- Integration tests in `backend/tests/` use `#[sqlx::test]` (fresh migrated
  DB per test) and drive the real router with `tower::ServiceExt::oneshot`.
  Shared helpers in `backend/tests/common/mod.rs` (build app with a
  controllable `FixedClock`, JSON request helpers, `login()` that creates a
  user via the auth service and returns the cookie).
- Unit tests next to code for pure logic (ISBN, email, validation,
  pagination) and for one service using an in-memory fake repo (loans) to
  demonstrate the port.
- Every endpoint: happy path + each documented error status.
- Gates: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`
  (for host crates), `cargo test`, `SQLX_OFFLINE=true cargo build`.

### 5.12 Code quality conventions

- `[workspace.lints]`: `rust.unsafe_code = "forbid"`, `clippy.unwrap_used = "warn"`,
  `clippy.expect_used = "warn"` (tests allowed via clippy.toml). No `unwrap()`
  /`expect()` in non-test code except for documented invariants at startup.
- Errors propagate with `?`; no swallowed errors; no `println!` in the server
  path (use `tracing`). CLI output via `println!`/`eprintln!` is fine.
- Teaching comments: short `//` comments the **first time** an idiom appears
  (e.g. why `spawn_blocking`, what `FromRef` does, why `Arc<dyn Trait>`,
  why `BEGIN IMMEDIATE`). Don't comment the obvious; don't repeat explanations.
- Public items get `///` docs when the name is not self-explanatory.

## 6. Shared crate (`library-shared`)

All types `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]`, plus
`#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]` (ts-rs is an
optional dependency behind feature `ts`, so the WASM frontend never compiles it), `#[serde(rename_all = "snake_case")]` on enums.
Timestamps are `chrono::DateTime<Utc>` (RFC 3339 strings in JSON, `string`
in TS). Generated TS goes to `frontend-react/src/generated/` (one file per
type, committed) by running `cargo test -p library-shared --features ts`.

```rust
pub struct Page<T> { pub items: Vec<T>, pub page: u32, pub per_page: u32, pub total: u64 }

pub struct LoginRequest { pub username: String, pub password: String }
pub struct CurrentUser  { pub id: i64, pub username: String }

pub enum Availability { All, Available, OnLoan }           // default All
pub struct BookQuery  { pub q: Option<String>, pub availability: Option<Availability>, pub page: Option<u32>, pub per_page: Option<u32> }
pub struct BookInput  { pub title: String, pub author: String, pub isbn: Option<String>, pub published_year: Option<i32>, pub total_copies: i64 }
pub struct Book       { pub id: i64, pub title: String, pub author: String, pub isbn: Option<String>, pub published_year: Option<i32>,
                        pub total_copies: i64, pub available_copies: i64, pub created_at: DateTime<Utc>, pub updated_at: DateTime<Utc> }

pub struct MemberQuery { pub q: Option<String>, pub page: Option<u32>, pub per_page: Option<u32> }
pub struct MemberInput { pub name: String, pub email: String }
pub struct Member      { pub id: i64, pub name: String, pub email: String, pub active_loans: i64,
                         pub created_at: DateTime<Utc>, pub updated_at: DateTime<Utc> }

pub enum LoanStatus { Active, Overdue, Returned, All }      // default Active
pub struct LoanQuery { pub status: Option<LoanStatus>, pub member_id: Option<i64>, pub book_id: Option<i64>, pub page: Option<u32>, pub per_page: Option<u32> }
pub struct NewLoan   { pub book_id: i64, pub member_id: i64 }
pub struct Loan      { pub id: i64, pub book_id: i64, pub book_title: String, pub member_id: i64, pub member_name: String,
                       pub borrowed_at: DateTime<Utc>, pub due_at: DateTime<Utc>, pub returned_at: Option<DateTime<Utc>>, pub is_overdue: bool }

pub struct ApiError     { pub error: ApiErrorBody }
pub struct ApiErrorBody { pub code: String, pub message: String,
                          #[serde(default, skip_serializing_if = "Option::is_none")] pub fields: Option<BTreeMap<String, String>> }

pub struct Health { pub status: String }
```

Optional query fields use `#[serde(default, skip_serializing_if = "Option::is_none")]`
so both frontends can serialize them to query strings cleanly.

## 7. Frontends (both identical in features, routes and look)

Shared UX contract:

| Route | Access | Content |
|---|---|---|
| `/` | – | redirect to `/books` |
| `/login` | – | username/password form; on success go to `?next=` or `/loans`; shows API error message |
| `/books` | – | search box (debounced 300 ms), availability select (All / Available / On loan), table: Title, Author, ISBN, Year, Available ("2 / 3"); when logged in: "Add book" button and per-row Edit, Lend (→ `/loans/new?book_id=`), Delete (confirm) |
| `/books/new`, `/books/:id/edit` | ✔ | form with field-level errors from `fields`, general error for 409 |
| `/members` | ✔ | search, table: Name, Email, Active loans; "Add member"; row View, Edit, Delete (confirm) |
| `/members/new`, `/members/:id/edit` | ✔ | form like books |
| `/members/:id` | ✔ | member info + their loans (status `all`) with Return buttons and "Lend a book" (→ `/loans/new?member_id=`) |
| `/loans` | ✔ | tabs Active / Overdue / Returned / All (state in `?status=`), table: Book, Member, Borrowed, Due, Status (Overdue rows visibly highlighted), Return button on unreturned rows; "New loan" button |
| `/loans/new` | ✔ | pick a book (search `availability=available`) and a member (search), prefilled from `?book_id=` / `?member_id=`; submit → `/loans`; shows 409 messages |

- Top nav: "Library" brand; Books; Members and Loans only when logged in;
  right side: username + "Log out", or "Log in".
- Protected routes redirect to `/login?next=<path>` when not logged in. Any
  401 from the API clears the current user and redirects to login.
- Lists show pagination (Prev / "Page X of Y" / Next) when `total > per_page`.
- Loading and empty states ("No books found.") and error messages from
  `ApiError.error.message` are shown.
- Dates displayed as `YYYY-MM-DD` (local time).
- Pico CSS v2 for styling; a tiny app stylesheet for overdue highlighting
  and layout tweaks only.
- Dev servers proxy `/api` → `http://127.0.0.1:4000`: React (Vite) on port
  5173, Leptos (Trunk) on port 8080. All requests same-origin with cookies.

React structure (`frontend-react/src/`): `main.tsx`, `App.tsx` (routes),
`api/` (`client.ts` fetch wrapper throwing `ApiRequestError`, plus
`auth.ts`, `books.ts`, `members.ts`, `loans.ts`), `auth/` (current-user
query + `RequireAuth`), `components/` (`Layout`, `Pagination`, `SearchBox`,
`ErrorMessage`, `FieldError`), `hooks/` (`useDebounce`), `pages/` (one file
per route), `generated/` (ts-rs output, never hand-edited). Server state via
TanStack Query (query keys per resource; mutations invalidate them).

Leptos structure (`frontend-leptos/src/`): `main.rs`, `app.rs` (router +
layout), `api/` (`mod.rs` client with `ApiRequestError`, plus `auth.rs`,
`books.rs`, `members.rs`, `loans.rs`), `auth.rs` (current-user context +
`RequireAuth`), `components/` (same set as React), `pages/` (same set as
React). Uses `library-shared` types directly. Mirrors the React structure on
purpose so the comparison doc can map file to file.

## 8. Docs, tooling, delivery

- `README.md`: what/why, stack, prerequisites, quick start (env, create user,
  seed, run backend + either frontend), Makefile targets, config table,
  tests, production build (`FRONTEND_DIR`), project layout.
- `docs/ARCHITECTURE.md`: layers; "Borrow a book" request traced through the
  real code with `file:line` references; error handling; auth/session flow
  and CSRF reasoning; transactions & races; testing strategy; glossary of
  Rust web idioms used (extractors, `State`/`FromRef`, `IntoResponse`,
  `Arc<dyn Trait>` + `async_trait`, `spawn_blocking`, `?` + `From`, etc.).
- `docs/REACT-VS-LEPTOS.md`: same feature side by side (state, data fetching,
  routing, forms, auth guard, error handling, build/tooling, bundle size),
  with file references to both.
- `Makefile`: `setup`, `db`, `dev-backend`, `dev-react`, `dev-leptos`,
  `seed`, `user`, `test`, `lint`, `fmt`, `types` (regenerate TS),
  `sqlx-prepare`, `build` (release backend + both frontends), `check` (all gates).
- `Dockerfile` (multi-stage: React build, Rust build with `SQLX_OFFLINE=true`,
  slim runtime as non-root, `FRONTEND_DIR` set, `/data` volume, healthcheck)
  and `.dockerignore`.
- `.github/workflows/ci.yml`: Rust (fmt, clippy, test, offline build),
  React (install, lint, typecheck, test, build), Leptos (clippy for wasm32,
  trunk build).

## 9. Out of scope (documented as "next steps" in README)

Rate limiting on login, roles/permissions, password reset, email
notifications, fines, reservations/holds, full-text search (FTS5), server-side
rendering / Leptos server functions, i18n.
