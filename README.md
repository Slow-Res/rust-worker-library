# Library Web App

A small library-management app (books, members, loans) built as a **learning
project for Rust web development**. One Rust backend serves a JSON API; two
frontends with identical features and look consume it, so you can compare the
approaches side by side:

- `frontend-react/`: React 19 + TypeScript (Vite)
- `frontend-leptos/`: Leptos 0.8, Rust compiled to WebAssembly (Trunk)

Librarians log in, manage books and members, lend books (14 days, at most 5
active loans per member) and take them back. Anyone can browse the catalogue
without logging in.

Where to read next:

| Doc | What it teaches |
|---|---|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | How the backend is layered, with one request ("Borrow a book") traced through the real code, plus auth, errors, transactions, tests, and a glossary of Rust web idioms |
| [docs/REACT-VS-LEPTOS.md](docs/REACT-VS-LEPTOS.md) | The same features in both frontends, side by side, with bundle sizes and the gotchas we actually hit |
| [docs/superpowers/specs/2026-10-10-library-web-app-design.md](docs/superpowers/specs/2026-10-10-library-web-app-design.md) | The design spec (rules, schema, API) |

## Stack

| Part | Tech |
|---|---|
| Backend | Rust, [Axum](https://docs.rs/axum) 0.8, [SQLx](https://docs.rs/sqlx) 0.9 with SQLite, Tokio, tower-http, tracing |
| Shared types | `shared/` crate (`library-shared`): the API request/response structs, used by the backend and by Leptos; TypeScript types for React are generated from them with ts-rs |
| React app | Vite, React 19, React Router 7, TanStack Query 5, Pico CSS, Vitest, oxlint |
| Leptos app | Leptos 0.8 (client-side rendered), leptos_router, gloo-net, Pico CSS, Trunk |

## Prerequisites

- **Rust** via [rustup](https://rustup.rs). `rust-toolchain.toml` selects stable and adds the `wasm32-unknown-unknown` target automatically.
- **sqlx-cli** (creates the dev database): `cargo install sqlx-cli --no-default-features --features sqlite,rustls`
- **Node 22 + npm** (React app).
- **Trunk** (Leptos app only): `cargo install trunk --locked`
- **Docker** (optional, for the production image).

`make setup` checks for these, copies `.env.example` to `.env` if you have no `.env`, runs `npm ci` and creates the database.

## Quick start

```sh
git clone <this repo> library-web-app && cd library-web-app

cp .env.example .env     # DATABASE_URL=sqlite://data/dev.db, BIND_ADDR=127.0.0.1:4000, ...
make db                  # create data/dev.db and apply the migration
make seed                # 12 sample books, 6 members, 3 loans
make user NAME=admin     # create a librarian; prompts for the password twice (min 8 chars)

make dev-backend         # terminal 1: API on http://127.0.0.1:4000
```

Then, in a second terminal, one of the frontends:

```sh
(cd frontend-react && npm ci)   # first time only
make dev-react                  # http://localhost:5173
# or
make dev-leptos                 # http://localhost:8080  (needs trunk)
```

Open the URL, browse the books, then log in as the user you created.

Why `make db` comes first: the backend checks its SQL at **compile time**
(SQLx macros). With `DATABASE_URL` set it connects to that database while
compiling, so the file must exist. If you would rather not install sqlx-cli,
run commands with `SQLX_OFFLINE=true` (for example
`SQLX_OFFLINE=true cargo run -p library-backend`), which uses the committed
metadata in `backend/.sqlx/` instead; the server creates and migrates the
database itself on startup.

Both dev servers proxy `/api` to the backend, so the browser only ever talks
to one origin and the session cookie just works. The React proxy target can be
changed with `API_PROXY_TARGET=http://127.0.0.1:4100 npm run dev`.

## Makefile targets

| Target | Does |
|---|---|
| `make setup` | Check tools, create `.env`, `npm ci`, `make db` |
| `make db` | Create and migrate the dev database (`sqlx database create`, `sqlx migrate run`) |
| `make dev-backend` | `cargo run -p library-backend` (port 4000) |
| `make dev-react` | Vite dev server on 5173 |
| `make dev-leptos` | `trunk serve` on 8080 |
| `make seed` | Insert sample data into an empty database |
| `make user NAME=alice` | Create a librarian (prompts for the password; `NAME` defaults to `admin`) |
| `make test` | `cargo test --workspace` |
| `make lint` / `make fmt` | clippy with `-D warnings` / `cargo fmt` |
| `make types` | Regenerate `frontend-react/src/generated/*.ts` from the shared crate |
| `make sqlx-prepare` | Refresh `backend/.sqlx/` after changing any `query!` macro (commit the result) |
| `make build` | Release backend, React build (`frontend-react/dist`), Leptos build (`frontend-leptos/dist`) |
| `make check` | Everything CI runs: fmt, clippy (host and wasm), tests, offline build, React lint/typecheck/tests |

## The backend CLI

```
library-backend [serve]                              run the server (default)
library-backend create-user <USERNAME> [--password-stdin]
library-backend seed
```

`create-user` prompts for the password twice, or reads one line from stdin with
`--password-stdin` (for scripts). There is no sign-up endpoint: librarian
accounts exist only because someone ran this command. `seed` does nothing if
the database already has books or members.

## Configuration

Environment variables, read at startup (a `.env` file is loaded if present).
Invalid values stop the server with a message naming the variable.

| Variable | Default | Notes |
|---|---|---|
| `DATABASE_URL` | `sqlite://data/library.db` | Parent directory and file are created if missing. Use `sqlite:///abs/path.db` for an absolute path. |
| `BIND_ADDR` | `127.0.0.1:4000` | Port 4000, because 3000 is often taken. |
| `FRONTEND_DIR` | unset | If set, the backend also serves this directory as the web app, falling back to `index.html` for client-side routes. |
| `COOKIE_SECURE` | `false` | Set `true` when served over HTTPS (adds `Secure` to the session cookie). With `true` over plain http, browsers drop the cookie and login appears to do nothing. |
| `SESSION_TTL_HOURS` | `168` | Session lifetime (7 days). |
| `LOG_FORMAT` | `pretty` | `pretty` or `json`. |
| `RUST_LOG` | `info,library_backend=debug,tower_http=info` | Standard `tracing` filter. |

## Tests

```sh
make test                              # backend (unit + integration), shared, Leptos pure logic
cd frontend-react && npm test          # Vitest + Testing Library
make check                             # all of it plus lint, typecheck and the offline build
```

Backend integration tests (`backend/tests/`) start the real router against a
fresh, migrated SQLite database per test (`#[sqlx::test]`) and a controllable
clock. See [ARCHITECTURE.md](docs/ARCHITECTURE.md#testing-strategy).

## Production build and Docker

The backend can serve a built frontend itself, so production is one process:

```sh
make build
FRONTEND_DIR=frontend-react/dist COOKIE_SECURE=false cargo run --release -p library-backend
# or serve the Leptos build instead:
FRONTEND_DIR=frontend-leptos/dist cargo run --release -p library-backend
```

Anything that is not `/api/*` and not a file in `FRONTEND_DIR` returns
`index.html`, so refreshing on `/loans` works.

### Docker

The image is a three-stage build (React build, Rust release build with
`SQLX_OFFLINE=true`, `debian:bookworm-slim` runtime). It serves the **React**
build, runs as a non-root user (uid 10001), keeps the database in the `/data`
volume, and has a `HEALTHCHECK` that calls `GET /api/health`.

```sh
docker build -t library-web-app .

# one-off commands run the CLI against the same volume
docker run --rm -v library-data:/data library-web-app seed
printf 'a-long-password\n' | docker run --rm -i -v library-data:/data library-web-app create-user admin --password-stdin

docker run -p 4000:4000 -v library-data:/data library-web-app
curl http://localhost:4000/api/health        # {"status":"ok"}
```

Then open http://localhost:4000.

- The image sets `DATABASE_URL=sqlite:///data/library.db`, `BIND_ADDR=0.0.0.0:4000`, `FRONTEND_DIR=/srv/frontend`, `LOG_FORMAT=json` and `COOKIE_SECURE=false`, so plain `http://localhost` works. **Behind HTTPS, run with `-e COOKIE_SECURE=true`.**
- To serve the Leptos build instead, build it (`cd frontend-leptos && trunk build --release`) and either mount it (`-v $PWD/frontend-leptos/dist:/srv/frontend:ro`) or change the first stage of the `Dockerfile` to a Trunk build and copy `frontend-leptos/dist` to `/srv/frontend`.
- `curl` is installed in the runtime image only for the healthcheck; the slim base has no HTTP client, and adding it is simpler than adding a special subcommand to the app.

## CI

`.github/workflows/ci.yml` runs four jobs: Rust (fmt, clippy, tests, offline
build), generated types (regenerate with ts-rs, then `git diff --exit-code
frontend-react/src/generated`), React (lint, typecheck, test, build) and Leptos
(clippy for `wasm32-unknown-unknown`, `trunk build --release`). If the types
job fails, run `make types` and commit the result.

## Project layout

```
Cargo.toml                workspace: shared, backend, frontend-leptos
shared/                   library-shared: API DTOs (serde), TypeScript export behind feature `ts`
backend/
  migrations/             SQL schema (embedded into the binary)
  .sqlx/                  offline query metadata (committed)
  src/
    main.rs, cli.rs, commands.rs, config.rs, server.rs, app.rs, state.rs
    error.rs, extract.rs, db.rs, clock.rs, pagination.rs, health.rs, seed.rs, telemetry.rs
    auth/ books/ members/ loans/     each: routes -> service -> repo (trait) <- sqlite (adapter)
  tests/                  integration tests driving the real router
frontend-react/src/       api/ auth/ components/ hooks/ pages/ generated/
frontend-leptos/src/      api/ auth.rs components/ pages/ (mirrors the React layout)
docs/                     ARCHITECTURE.md, REACT-VS-LEPTOS.md, spec and plan
Dockerfile, Makefile, .github/workflows/ci.yml
```

## Next steps (deliberately not built)

Rate limiting on login, roles and permissions, password reset, email
notifications, fines, reservations, full-text search (SQLite FTS5),
server-side rendering / Leptos server functions, i18n.
