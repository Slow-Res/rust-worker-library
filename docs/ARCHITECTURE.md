# Architecture

This document explains how the backend is put together and then follows one
real request, "Borrow a book", from a button click in the React app down to
the SQL statement and back. It is written for someone who knows web
development but is new to Rust web idioms. Every `path:line` reference points
at the code as it is now; if you edit a file, the numbers drift, the function
names do not.

Contents: [The big picture](#the-big-picture) ·
[Layers](#layers) ·
[Walkthrough: Borrow a book](#walkthrough-borrow-a-book) ·
[Error handling](#error-handling) ·
[Authentication and sessions](#authentication-and-sessions) ·
[Transactions and races](#transactions-and-races) ·
[Testing strategy](#testing-strategy) ·
[Glossary of Rust web idioms](#glossary-of-rust-web-idioms)

## The big picture

```
 Browser
 ┌─────────────────────────┐        same origin, cookies included
 │ React app  or  Leptos   │ ──────────────────────────────────────┐
 │ (SPA, JSON over fetch)  │                                       │
 └─────────────────────────┘                                       ▼
                                            ┌──────────────────────────────────────┐
  dev: Vite (5173) / Trunk (8080)  ──proxy─▶│ library-backend (Axum, port 4000)    │
  prod: the backend serves the build itself │  /api/*    JSON API                  │
        (FRONTEND_DIR)                      │  anything else: static SPA files     │
                                            └───────────────┬──────────────────────┘
                                                            │ SQLx
                                                            ▼
                                                   SQLite file (WAL mode)

 Cargo workspace
   shared/            library-shared    request/response structs (serde). Used by the backend
                                        and by Leptos; ts-rs turns them into TypeScript for React.
   backend/           library-backend   everything server side (library + binary)
   frontend-leptos/   library-frontend-leptos   Rust -> WebAssembly
```

The key idea: **the shared crate is the API contract.** `NewLoan`, `Loan`,
`ApiError` and friends are defined once in
[shared/src/loans.rs](../shared/src/loans.rs) and
[shared/src/common.rs](../shared/src/common.rs). The backend serializes them,
Leptos deserializes the very same types, and
[`make types`](../Makefile) generates
[frontend-react/src/generated/NewLoan.ts](../frontend-react/src/generated/NewLoan.ts)
from them (CI fails if the generated files are stale). A field renamed in Rust
breaks the build of both frontends instead of breaking at runtime.

## Layers

Each backend feature (`auth`, `books`, `members`, `loans`) is a module with the
same core files (`mod.rs`, `routes.rs`, `service.rs`, `repo.rs`, `sqlite.rs`),
plus a few extras: `books` and `members` add a `domain.rs` (validation and
normalization of their input), and `auth` adds `extractor.rs` (the `AuthUser`
extractor) and `password.rs` (Argon2 hashing). `loans` is the canonical,
unadorned example:

| Layer | File | Job | May mention | Must not mention |
|---|---|---|---|---|
| Routes | [backend/src/loans/routes.rs](../backend/src/loans/routes.rs) | HTTP in, HTTP out: extractors, status codes | axum, the service | SQL, `sqlx` |
| Service | [backend/src/loans/service.rs](../backend/src/loans/service.rs) | Business rules and validation (14-day period, 5-loan limit, overdue) | the repo **trait**, `Clock` | axum, `sqlx` |
| Port | [backend/src/loans/repo.rs](../backend/src/loans/repo.rs) | `trait LoanRepository`: what the service needs from storage | plain types | axum, `sqlx` |
| Adapter | [backend/src/loans/sqlite.rs](../backend/src/loans/sqlite.rs) | `SqliteLoanRepository`: the trait implemented with SQL | `sqlx` | axum |
| Wiring | [backend/src/state.rs](../backend/src/state.rs) | Builds the concrete repositories and services once at startup | everything | |

```
HTTP ─▶ routes.rs ─▶ service.rs ─▶ repo.rs (trait) ◀─ sqlite.rs ─▶ SQLite
        extractors    rules         the "port"         the "adapter"
        status codes  Arc<dyn …>
```

Why bother with the trait? It is a dependency boundary. The service holds an
`Arc<dyn LoanRepository>` ([loans/service.rs:23](../backend/src/loans/service.rs#L23)),
so the same rules run against SQLite in production and against an in-memory
fake in unit tests ([loans/service.rs:133](../backend/src/loans/service.rs#L133)).
This is sometimes called "ports and adapters" or "light hexagonal"; here it
simply means *the rules do not know what database they sit on*, and *the SQL
does not know about HTTP*.

Cross-cutting pieces live next to `main.rs`:

- [app.rs](../backend/src/app.rs): builds the router and the middleware stack.
- [state.rs](../backend/src/state.rs): `AppState`, the shared handles every handler can ask for.
- [error.rs](../backend/src/error.rs): `AppError` and how it becomes an HTTP response.
- [extract.rs](../backend/src/extract.rs): `ApiJson`, `ApiQuery`, `ApiPath`.
- [db.rs](../backend/src/db.rs): connection pool, pragmas, migrations, `RepoError`.
- [clock.rs](../backend/src/clock.rs): the `Clock` trait so time is injectable.
- [server.rs](../backend/src/server.rs), [cli.rs](../backend/src/cli.rs), [commands.rs](../backend/src/commands.rs), [config.rs](../backend/src/config.rs): startup and the CLI.

## Walkthrough: Borrow a book

Scenario: a logged-in librarian is on the Books page, clicks **Lend** next to
"Dune", picks a member, and presses **Lend book**. Two pages are involved
because "Lend" on the books table is only a link that pre-fills the form; the
actual borrowing is the form submit.

### Part A: the browser

1. **Click "Lend"** on a book row.
   [frontend-react/src/pages/BooksPage.tsx:105](../frontend-react/src/pages/BooksPage.tsx#L105)
   is a router `<Link to={`/loans/new?book_id=${book.id}`}>`. No request to
   the backend yet; the router navigates to `/loans/new?book_id=1`.

2. **The form page loads.**
   [frontend-react/src/pages/NewLoanPage.tsx:27](../frontend-react/src/pages/NewLoanPage.tsx#L27)
   reads `book_id` from the URL, and lines 40-49 fetch that book and the member
   list so the two `<select>` pickers have options. The route is inside the
   `RequireAuth` guard
   ([frontend-react/src/App.tsx:23](../frontend-react/src/App.tsx#L23)), which
   checks the cached current user first
   ([frontend-react/src/auth/RequireAuth.tsx:6](../frontend-react/src/auth/RequireAuth.tsx#L6)).

3. **Submit.** Pressing "Lend book"
   ([NewLoanPage.tsx:112](../frontend-react/src/pages/NewLoanPage.tsx#L112))
   fires `handleSubmit`
   ([NewLoanPage.tsx:76](../frontend-react/src/pages/NewLoanPage.tsx#L76)),
   which calls `lend.mutate({ book_id, member_id })` (line 79). `lend` is a
   TanStack Query *mutation* ([NewLoanPage.tsx:67](../frontend-react/src/pages/NewLoanPage.tsx#L67))
   whose function is `createLoan`.

4. **API call.** `createLoan`
   ([frontend-react/src/api/loans.ts:10](../frontend-react/src/api/loans.ts#L10))
   calls the shared `request` wrapper
   ([frontend-react/src/api/client.ts:59](../frontend-react/src/api/client.ts#L59)),
   which runs `fetch('/api/loans', { method: 'POST', credentials: 'same-origin', body: '{"book_id":1,"member_id":2}', headers: {'Content-Type': 'application/json'} })`
   ([client.ts:64](../frontend-react/src/api/client.ts#L64)). The path is
   relative, so the request goes to whatever origin served the page; the
   browser attaches the `library_session` cookie by itself.
   - In development the origin is Vite (5173), which proxies `/api` to the
     backend ([frontend-react/vite.config.ts:12](../frontend-react/vite.config.ts#L12)).
   - In production the origin *is* the backend.
   - The body type `NewLoan` is the generated
     [frontend-react/src/generated/NewLoan.ts](../frontend-react/src/generated/NewLoan.ts).

### Part B: the server

5. **Listener and router.** `axum::serve` accepts the connection
   ([backend/src/server.rs:27](../backend/src/server.rs#L27)) and hands the
   request to the router built by
   [`build_router`](../backend/src/app.rs#L34). The `/api` router is nested at
   [app.rs:44](../backend/src/app.rs#L44); `loans::routes()` is merged in at
   [app.rs:41](../backend/src/app.rs#L41), and
   [loans/routes.rs:17](../backend/src/loans/routes.rs#L17) maps
   `POST /loans` to the `create` handler. (The `/api` prefix is added by the
   `nest`, which is why the route says `/loans`.)

6. **Middleware (outermost first)** from [app.rs:57-87](../backend/src/app.rs#L57):
   request id (`x-request-id` is set if missing and echoed on the response) →
   `TraceLayer` (one log span per request with method, path and request id) →
   JSON error bodies (see [Error handling](#error-handling)) → 15 s timeout → gzip compression → security headers → **64 KiB body limit**.
   A `ServiceBuilder` lists layers top-to-bottom in the order a request meets
   them. A body over the limit is refused with 413 here, before any handler
   code sees it.

7. **Extractors run, left to right.** The handler signature is the contract:

   ```rust
   // backend/src/loans/routes.rs:31
   async fn create(
       State(loans): State<LoanService>,
       _user: AuthUser,
       ApiJson(input): ApiJson<NewLoan>,
   ) -> Result<(StatusCode, ApiJson<Loan>), AppError>
   ```

   Axum builds each argument from the request, in order, and stops at the first
   failure (its error becomes the response and the handler body never runs):

   1. `State<LoanService>`: pulls the service out of `AppState` using the
      `FromRef` impls generated at [state.rs:21](../backend/src/state.rs#L21)
      (`AppState` is built at [state.rs:35](../backend/src/state.rs#L35)).
   2. `AuthUser`: [auth/extractor.rs:30](../backend/src/auth/extractor.rs#L30).
      It reads the `library_session` cookie (line 32-36; a missing cookie is
      `AppError::Unauthorized`), then calls
      `AuthService::authenticate` (line 37 → [auth/service.rs:146](../backend/src/auth/service.rs#L146)),
      which hashes the token with SHA-256 and runs
      `SELECT … FROM sessions s JOIN users u … WHERE s.token_hash = ? AND s.expires_at > ?`
      ([auth/sqlite.rs:79](../backend/src/auth/sqlite.rs#L79)). No row means 401, and the
      JSON body is **not even parsed**, because extractor 3 never runs.
   3. `ApiJson<NewLoan>`: [extract.rs:17](../backend/src/extract.rs#L17) is a thin
      wrapper over `axum::Json` whose rejection type is `AppError`
      (the `#[from_request(via(axum::Json), rejection(AppError))]` attribute on
      line 16). Wrong content type, malformed JSON or a missing field becomes
      `400 bad_request` in the usual error shape instead of axum's plain-text
      default. This extractor consumes the body, so it must be last.

8. **The handler** is one line
   ([loans/routes.rs:36](../backend/src/loans/routes.rs#L36)): call
   `loans.create(input).await?`, and wrap the success value as
   `(StatusCode::CREATED, ApiJson(loan))`. Handlers hold no business logic;
   the `?` forwards any `AppError` unchanged.

9. **The service** [`LoanService::create`](../backend/src/loans/service.rs#L47):
   - validates ids are positive and collects *field* errors
     (lines 48-55; `FieldErrors` is in [error.rs:17](../backend/src/error.rs#L17));
   - reads the clock (`self.clock.now()`) and computes
     `due_at = now + 14 days` (line 62, constant at [loans/mod.rs:23](../backend/src/loans/mod.rs#L23));
   - calls the repository **through the trait**, passing the policy value
     `MAX_ACTIVE_LOANS_PER_MEMBER` (line 66, [loans/mod.rs:25](../backend/src/loans/mod.rs#L25)):
     `self.repo.create_checked(record, MAX_ACTIVE_LOANS_PER_MEMBER)`;
   - maps the typed outcome to a result (lines 69-80). `Created` becomes a
     `Loan` (with `is_overdue` computed against `now`); the others become
     `NotFound("book")`, `NotFound("member")`, or `Conflict` with code
     `no_copies_available` / `loan_limit_reached`.

   The service owns the *policy* (what the numbers are and what each outcome
   means); the repository owns *atomic enforcement*.

10. **The port.** [`LoanRepository::create_checked`](../backend/src/loans/repo.rs#L75)
    returns a `CreateLoanOutcome` enum ([repo.rs:45](../backend/src/loans/repo.rs#L45))
    rather than a bare `Result`. Expected business outcomes ("no copies left")
    are enum variants; only genuine failures (disk, SQL) are `Err(RepoError)`.
    Because the method is `async` and the service calls it via `dyn`, the trait
    carries `#[async_trait]` ([repo.rs:60](../backend/src/loans/repo.rs#L60)).

11. **The adapter** [`SqliteLoanRepository::create_checked`](../backend/src/loans/sqlite.rs#L148):
    1. `BEGIN IMMEDIATE` ([sqlite.rs:158](../backend/src/loans/sqlite.rs#L158))
       takes SQLite's write lock before reading anything (why: see
       [Transactions and races](#transactions-and-races)).
    2. Checks in a fixed order, returning early on the first failure
       (dropping `tx` rolls the transaction back):
       book exists (line 162) → member exists (line 171) →
       free copy: `COUNT(*)` of unreturned loans of this book `>= total_copies`
       (lines 181-190) → member under the limit (lines 192-201).
    3. `INSERT INTO loans …` ([sqlite.rs:206](../backend/src/loans/sqlite.rs#L206)),
       then re-reads the row with its joined book title and member name
       (`fetch_loan`, [sqlite.rs:97](../backend/src/loans/sqlite.rs#L97)), and
       `COMMIT` ([sqlite.rs:218](../backend/src/loans/sqlite.rs#L218)).
    4. Timestamps are stored as integer Unix seconds and converted at this
       boundary (`to_timestamp` / `from_timestamp`, [db.rs:82](../backend/src/db.rs#L82)).
       The queries are `sqlx::query!` macros, checked against the schema at
       **compile time**; `backend/.sqlx/` holds the metadata that lets CI build
       without a database.

### Part C: back up the stack

12. **Success.** `Created(details)` → `to_loan` → `Ok(Loan)` → the handler
    returns `(201, ApiJson(loan))`. `ApiJson`'s `IntoResponse` impl
    ([extract.rs:19](../backend/src/extract.rs#L19)) serializes it to JSON
    with `Content-Type: application/json`. The response travels back out
    through the middleware (request-id header added, headers set, body
    gzipped).

13. **Failure.** Any `Err(AppError)` is turned into a response by
    [`impl IntoResponse for AppError`](../backend/src/error.rs#L138): status
    from [error.rs:103](../backend/src/error.rs#L103) and body
    `{"error":{"code":"no_copies_available","message":"…"}}` in the shape of
    `library_shared::ApiError`. For example borrowing the last copy twice gives
    `409 {"error":{"code":"no_copies_available", …}}`.

14. **The browser parses the result.** In
    [client.ts:73](../frontend-react/src/api/client.ts#L73) a non-2xx response
    is converted into an `ApiRequestError` carrying `status`, `code`,
    `message` and `fields` (`parseError`, [client.ts:43](../frontend-react/src/api/client.ts#L43)).
    A 201 body is parsed as `Loan` ([client.ts:75](../frontend-react/src/api/client.ts#L75)).

15. **UI update.**
    - Success: the mutation's `onSuccess`
      ([NewLoanPage.tsx:69](../frontend-react/src/pages/NewLoanPage.tsx#L69))
      calls `invalidateLibraryData` for `loans`, `books`, `book`, `members`,
      `member` ([api/invalidate.ts:4](../frontend-react/src/api/invalidate.ts#L4)).
      *Invalidating* marks cached query results stale, and any mounted
      query refetches. That is why the book's "Available" count and the
      member's "Active loans" are right the next time they are shown, even
      though neither number is stored: both are computed by SQL from the loans
      table. Then `navigate('/loans')`.
    - Failure: `lend.error` is an `ApiRequestError`. Field errors
      (`fields.book_id`) go under the picker ([NewLoanPage.tsx:96](../frontend-react/src/pages/NewLoanPage.tsx#L96));
      anything else such as the 409 message is shown by `<ErrorMessage>`
      ([NewLoanPage.tsx:111](../frontend-react/src/pages/NewLoanPage.tsx#L111)).
    - A **401** from any request is handled globally: the query client clears
      the cached user and redirects to `/login?next=…`
      ([frontend-react/src/queryClient.ts:15](../frontend-react/src/queryClient.ts#L15), wired up in
      [main.tsx:12](../frontend-react/src/main.tsx#L12)).

The Leptos frontend takes the same trip with different front-end code; see
[REACT-VS-LEPTOS.md](REACT-VS-LEPTOS.md). From step 5 on the two are
indistinguishable to the server.

The same flow is exercised end to end, without a browser, by
[backend/tests/loans.rs:80](../backend/tests/loans.rs#L80) and the 404 / 409
variants after it.

## Error handling

There are two error types, one per side of the service boundary.

- **`RepoError`** ([db.rs:53](../backend/src/db.rs#L53)) is what repositories
  return: `Conflict { constraint }` (a UNIQUE violation, with the constraint
  name such as `members.email`) or `Unexpected(anyhow::Error)`. It contains no
  `sqlx` types in its public shape, so the service layer stays storage
  agnostic. `impl From<sqlx::Error> for RepoError`
  ([db.rs:63](../backend/src/db.rs#L63)) is what lets repository code use `?`
  on any SQLx call.
- **`AppError`** ([error.rs:53](../backend/src/error.rs#L53)) is what services
  and handlers return, and it knows how to become an HTTP response.

  | Variant | Status | `code` |
  |---|---|---|
  | `Validation(FieldErrors)` | 422 | `validation_error` (+ `fields`) |
  | `BadRequest(String)` | 400 | `bad_request` (bad JSON, query or path) |
  | `Unauthorized` / `InvalidCredentials` | 401 | `unauthorized` |
  | `NotFound(&'static str)` | 404 | `not_found` ("book not found") |
  | `Conflict { code, message }` | 409 | the given code (`isbn_taken`, `no_copies_available`, …) |
  | `Internal(anyhow::Error)` | 500 | `internal` (generic message) |

Things worth noticing:

- `impl From<RepoError> for AppError` ([error.rs:76](../backend/src/error.rs#L76))
  makes `self.repo.something().await?` in a service convert automatically.
  A `Conflict` that reaches that conversion unhandled is treated as a bug
  (500), because services are expected to map the unique constraints they know
  about (for example `isbn_taken`) themselves.
- The three `From<…Rejection>` impls ([error.rs:84-100](../backend/src/error.rs#L84))
  are what turn axum's extractor failures into `BadRequest`.
- **Internal errors never leak.** `into_response` logs the real error with
  `tracing::error!` and sends only "Internal server error"
  ([error.rs:138-145](../backend/src/error.rs#L138)).
- Unknown `/api/*` paths hit `api_not_found`
  ([app.rs:138](../backend/src/app.rs#L138)) so even a 404 is JSON.
- Some errors are produced *outside* our handlers: axum's 405 Method Not
  Allowed, the timeout layer's 408 and the body-limit layer's 413 arrive with
  empty or plain-text bodies. A small `from_fn` layer
  (`json_error_bodies`, [app.rs:99](../backend/src/app.rs#L99)) rewrites those
  three statuses on `/api/*` into the standard JSON shape
  (`method_not_allowed`, `request_timeout`, `payload_too_large`) and keeps the
  status. It sits just above the timeout layer so it sees the 408 too.
- The error body type is the shared `ApiError`, so each frontend decodes it
  with the same shape: React in `parseError`
  ([client.ts:43](../frontend-react/src/api/client.ts#L43)), Leptos in
  `ApiRequestError::from_body`
  ([frontend-leptos/src/api/mod.rs:45](../frontend-leptos/src/api/mod.rs#L45)).
- Why services return *outcome enums* for business cases (instead of
  `Err(...)`): "no copies left" is a normal result of a correct check, and a
  `match` on an enum is exhaustive. Add a variant and the compiler lists every
  place that must handle it.

## Authentication and sessions

**Accounts.** Librarians are created only by the CLI
(`library-backend create-user`, [commands.rs:19](../backend/src/commands.rs#L19)).
There is no sign-up endpoint, so there is nothing to attack there. All
librarians have equal rights.

**Passwords.** Argon2id via the `argon2` crate defaults, stored as a PHC
string (`$argon2id$v=19$…`) containing algorithm, parameters, salt and hash
([auth/password.rs](../backend/src/auth/password.rs)). Hashing is deliberately
slow, so both `hash_password` ([password.rs:18](../backend/src/auth/password.rs#L18))
and `verify_password` ([password.rs:31](../backend/src/auth/password.rs#L31))
run inside `tokio::task::spawn_blocking`, which moves the CPU-heavy work off
the async executor so other requests keep being served.

**Login** ([auth/routes.rs:33](../backend/src/auth/routes.rs#L33) →
[auth/service.rs:92](../backend/src/auth/service.rs#L92)):

1. Validate that username and password are non-empty (422 otherwise).
2. Look up the user. **Always run one password verification**, using a fixed
   dummy hash when the user does not exist ([password.rs:15](../backend/src/auth/password.rs#L15)),
   so response time does not reveal which usernames exist. Both failure cases
   return the identical message "invalid username or password".
3. Create a session: 32 random bytes from the OS RNG, hex-encoded (64
   characters) ([service.rs:159](../backend/src/auth/service.rs#L159)).
   **The database stores only `hex(SHA-256(token))`**
   ([service.rs:171](../backend/src/auth/service.rs#L171)); the raw token exists
   only in the cookie. If the database leaks, the stored hashes cannot be used
   as cookies. (A fast hash is fine for tokens, unlike passwords, because a
   token is already 256 random bits.)
4. Sweep expired sessions (housekeeping; a failure only logs a warning).
5. The route sets the cookie `library_session` with `HttpOnly`,
   `SameSite=Lax`, `Path=/`, `Max-Age` = the session TTL, and `Secure` when
   `COOKIE_SECURE=true` ([auth/routes.rs:40-46](../backend/src/auth/routes.rs#L40)).
   `HttpOnly` means page JavaScript cannot read the cookie, so an XSS bug
   cannot steal the session token (it could still make requests).

**Using a session.** The `AuthUser` extractor
([auth/extractor.rs:23](../backend/src/auth/extractor.rs#L23)) is the **only**
auth mechanism. A handler that takes `AuthUser` as an argument requires a
login; a handler that does not, is public. There is no middleware to forget to
apply: the requirement is visible in the function signature. See
[backend/tests/loans.rs:68](../backend/tests/loans.rs#L68)
(`every_endpoint_requires_auth`).

**Logout** deletes the session row and sends a removal cookie
([auth/routes.rs:50](../backend/src/auth/routes.rs#L50)). Because sessions live
in the database, logging out genuinely invalidates the token everywhere (unlike
a stateless signed token).

### CSRF reasoning

CSRF is an attack where another website makes the victim's browser send an
authenticated request to ours. The defence here has two parts, both
deliberate:

1. **`SameSite=Lax` cookie.** Browsers do not attach the cookie to cross-site
   `POST`/`PUT`/`DELETE` requests (forms, `fetch`), only to top-level
   navigations that use safe methods like `GET`.
2. **Mutating endpoints accept only JSON.** `ApiJson` (via `axum::Json`)
   rejects any request whose `Content-Type` is not `application/json`. A
   cross-site HTML form can only send form-encoded or plain text, and a
   cross-site `fetch` with `application/json` triggers a CORS preflight, which
   the server never approves (it sends no CORS headers at all).

Safe methods (`GET`) never change state in this API, so the cookie being sent
on a cross-site navigation does no harm. The frontends and the API share an
origin (dev proxy / `FRONTEND_DIR`), so no CORS configuration is needed.
What this does *not* give you: rate limiting on login, or protection if you
ever add a state-changing `GET`, or a form-encoded endpoint. There is no CSRF
token; add one before changing either assumption.

`X-Frame-Options: DENY` ([app.rs:84](../backend/src/app.rs#L84)) stops the app
being framed (clickjacking).

## Transactions and races

SQLite allows **one writer at a time**. The adapter is configured in
[db.rs:17-41](../backend/src/db.rs#L17): WAL journal mode (readers do not block
the writer), `foreign_keys = ON` on every connection, `busy_timeout = 5s`
(a connection that needs the write lock waits up to five seconds instead of
failing immediately), and a pool of 8 connections.

The problem: "borrow" is *check, then write*. Two requests for the last copy
could both check ("1 copy free") before either inserts. The rules therefore
run inside one transaction **in the repository**, not in the service, because
only the adapter knows what its database can guarantee.

The detail that matters is `BEGIN IMMEDIATE`
([loans/sqlite.rs:158](../backend/src/loans/sqlite.rs#L158)). SQLite's
default `BEGIN` is *deferred*: the write lock is taken at the first write, so
another request's insert could slip in between our reads and our write.
`BEGIN IMMEDIATE` takes the write lock first. The second concurrent borrow
waits (up to `busy_timeout`) for the first to commit, then runs its checks
against the updated data, and sees `NoCopiesAvailable`. The test
`concurrent_borrows_never_exceed_total_copies`
([tests/loans.rs:372](../backend/tests/loans.rs#372)) races 8 borrows for a
2-copy book through one file-backed database to prove it. The same technique
protects:

- `mark_returned` ([loans/sqlite.rs:222](../backend/src/loans/sqlite.rs#L222)): "returning twice" cannot both succeed;
- `BookRepository::update_checked` / `delete_checked` ([books/sqlite.rs:205](../backend/src/books/sqlite.rs#L205), [books/sqlite.rs:240](../backend/src/books/sqlite.rs#L240)): copies cannot drop below the active loans, and a book with active loans cannot be deleted;
- `MemberRepository::delete_checked` ([members/sqlite.rs:170](../backend/src/members/sqlite.rs#L170)).

Rust specifics: `self.pool.begin_with("BEGIN IMMEDIATE")` returns a
`Transaction`. Statements run on it with `&mut *tx`. If the function returns
early (`return Ok(CreateLoanOutcome::NoCopiesAvailable)` or `?`), `tx` is
dropped and SQLx **rolls back automatically**; only an explicit
`tx.commit()` persists anything.

Derived numbers are never stored. `available_copies` is
`total_copies - (unreturned loans)` and a member's `active_loans` is a count,
both computed in SQL, so there is no counter that can drift out of sync.
Timestamps are integer Unix seconds, and `SystemClock` truncates "now" to whole
seconds ([clock.rs:20](../backend/src/clock.rs#L20)) so in-memory comparisons
(`due_at < now`) agree with the SQL filter.

## Testing strategy

| Layer | Where | How |
|---|---|---|
| Pure logic | `#[cfg(test)]` modules next to the code: ISBN checksum, email validation, pagination, config parsing, `escape_like` | Plain `#[test]`, no I/O |
| Service rules | [loans/service.rs:133](../backend/src/loans/service.rs#L133) | The service runs against `FakeLoanRepository`, an in-memory implementation of the trait, plus `FixedClock`. No database. This is what the port buys you |
| Time | [clock.rs:37](../backend/src/clock.rs#L37) `FixedClock` | `set` / `advance` make "14 days later" and the overdue boundary (`due_at == now` is *not* overdue) deterministic |
| Whole HTTP stack | [backend/tests/](../backend/tests/) (`auth`, `books`, `members`, `loans`, `health`, `middleware`) | `#[sqlx::test]` gives each test a fresh, already-migrated SQLite database; the test builds the **real router** with `build_router` and sends requests with `tower::ServiceExt::oneshot` (no sockets). Helpers in [tests/common/mod.rs](../backend/tests/common/mod.rs): `TestApp`, `login()`, JSON request helpers |
| Middleware and SPA serving | [tests/middleware.rs](../backend/tests/middleware.rs) | security headers, request-id propagation, 413 body limit, JSON 405/413 bodies, SPA fallback vs. JSON 404 |
| DB mapping | [db.rs:129](../backend/src/db.rs#L129) | a UNIQUE violation becomes `RepoError::Conflict` with the constraint name |
| React | `frontend-react/src/**/*.test.tsx` | Vitest + Testing Library: client, route guard, forms, pagination, search debounce |
| Leptos | `#[cfg(test)]` in `frontend-leptos/src` | Only pure logic (URL parsing, redirect target, API error decoding); the UI itself is not unit-tested |
| Browser | manual smoke tests with Playwright, run for both frontends against a real backend | Login, guards, CRUD, loan rules, logout, pagination, error states. They found the Leptos redirect loop described in [REACT-VS-LEPTOS.md](REACT-VS-LEPTOS.md#gotchas-we-actually-hit). They are not part of CI |

Every endpoint has a happy-path test and a test per documented error status.
`#[sqlx::test]` creates throwaway databases under `target/sqlx/` (which is why
`.gitignore` ignores `target/` at any depth).

The atomicity guarantee is covered twice: by sequential tests of each rule (for
example `no_free_copy_is_409`, [tests/loans.rs:160](../backend/tests/loans.rs#L160)),
and by a concurrent one, `concurrent_borrows_never_exceed_total_copies`
([tests/loans.rs:372](../backend/tests/loans.rs#372)), which fires 8 simultaneous borrows
at a book with 2 copies and asserts exactly 2 succeed, the rest get 409
`no_copies_available`, and the database holds exactly 2 active loans.

CI ([.github/workflows/ci.yml](../.github/workflows/ci.yml)) runs formatting,
clippy with warnings denied, all tests, an offline build, the generated-types
check, React lint/typecheck/test/build, and Leptos wasm clippy and
`trunk build`.

## Glossary of Rust web idioms

Each entry says what the idiom is and where to see it in this repo.

**Extractor.** A handler argument that axum builds from the request (body,
query, path, headers, cookies). Extractors run left to right; if one fails its
rejection becomes the response. A type becomes an extractor by implementing
`FromRequestParts` (no body) or `FromRequest` (consumes the body, so it must be
last). See [loans/routes.rs:31](../backend/src/loans/routes.rs#L31) and the
custom [`AuthUser`](../backend/src/auth/extractor.rs#L23).

**`IntoResponse`.** The trait that turns a handler's return value into an HTTP
response. Tuples compose: `(StatusCode, ApiJson<Loan>)` is "status plus body".
`AppError` implements it ([error.rs:138](../backend/src/error.rs#L138)), which
is why handlers can return `Result<T, AppError>`.

**`State` and `FromRef`.** Shared application data (the DB pool, services,
config) is put in one `AppState` and attached with `Router::with_state`
([app.rs:54](../backend/src/app.rs#L54)). `#[derive(FromRef)]`
([state.rs:21](../backend/src/state.rs#L21)) generates "get field X out of the
state", so a handler can write `State(loans): State<LoanService>` instead of
taking the whole state. The `AuthUser` extractor uses `AuthService: FromRef<S>`
as a generic bound ([extractor.rs:23-27](../backend/src/auth/extractor.rs#L23))
so it works with any state that can provide an `AuthService`.

**`Arc<dyn Trait>` + `#[async_trait]`.** `dyn Trait` is a trait object:
"some value implementing this trait, chosen at runtime"; `Arc` makes it cheap
to clone and share across tasks. The services hold `Arc<dyn …Repository>` and
`Arc<dyn Clock>` ([loans/service.rs:22-25](../backend/src/loans/service.rs#L22)).
Async methods in traits used through `dyn` need the `async_trait` macro
([loans/repo.rs:60](../backend/src/loans/repo.rs#L60)), which boxes the
returned future.

**`Send + Sync` bounds.** `trait LoanRepository: Send + Sync`
([repo.rs:61](../backend/src/loans/repo.rs#L61)) promises the implementation can
be moved to, and shared between, threads, which Tokio's multi-threaded runtime
requires of anything held across `.await`.

**`?` and `From`.** `?` on a `Result` returns early with the error, converting
it with `From` first. `impl From<sqlx::Error> for RepoError`
([db.rs:63](../backend/src/db.rs#L63)) and
`impl From<RepoError> for AppError` ([error.rs:76](../backend/src/error.rs#L76))
form a chain: SQL error → `RepoError` → `AppError` → HTTP response, written as
just `?` at each step.

**`thiserror` vs `anyhow`.** `thiserror` derives `Display`/`Error` for an
error *type callers match on* (`AppError`, `RepoError`). `anyhow::Error` is a
catch-all box for "something unexpected, with context", used in
`Internal(anyhow::Error)` and in startup code (`main`, `server::serve`).

**`spawn_blocking`.** Async tasks must not hog a thread, or every other request
on it stalls. CPU-heavy or blocking work (Argon2) is handed to a separate pool
([password.rs:19](../backend/src/auth/password.rs#L19)).

**Tower layers.** Middleware in axum is a `tower` `Layer` wrapping a `Service`.
`ServiceBuilder` composes them, listed outermost-first
([app.rs:57](../backend/src/app.rs#L57)): request id, tracing, JSON error
bodies, timeout, compression, headers, body limit.

**Router composition.** `Router::merge` combines routers, `nest("/api", …)`
adds a prefix, `fallback` / `fallback_service` handle everything unmatched
([app.rs:36-50](../backend/src/app.rs#L36)). `ServeDir` plus `ServeFile` serve
the React/Leptos build with the SPA `index.html` fallback.

**Compile-time checked SQL.** `sqlx::query!` verifies the SQL and the column
types against a real schema while compiling. `cargo sqlx prepare` saves that
information to `backend/.sqlx/` so builds with `SQLX_OFFLINE=true` (CI,
Docker) need no database. Dynamic queries (search filters) use `QueryBuilder`
with `push_bind`, never string interpolation
([loans/sqlite.rs:70](../backend/src/loans/sqlite.rs#L70)).

**Newtype wrappers with derives.** `ApiJson<T>(pub T)`
([extract.rs:17](../backend/src/extract.rs#L17)) is a tuple struct that exists
to attach behaviour (our rejection type) to an existing extractor via
`#[derive(FromRequest)]`.

**Interior mutability.** `FixedClock(Mutex<DateTime>)`
([clock.rs:37](../backend/src/clock.rs#L37)) can be changed through a shared
`&self`/`Arc` handle because the `Mutex` guards the mutation at runtime.

**Graceful shutdown.** `axum::serve(...).with_graceful_shutdown(signal)`
([server.rs:27-30](../backend/src/server.rs#L27)) stops accepting new
connections on Ctrl-C or SIGTERM (what Docker sends), finishes in-flight
requests, then closes the pool.

**Workspace and feature flags.** One `Cargo.lock` and shared dependency
versions for `shared`, `backend` and `frontend-leptos`. The `ts` feature of
`library-shared` ([shared/Cargo.toml:9](../shared/Cargo.toml#L9)) keeps the
TypeScript generator out of the WebAssembly build.
