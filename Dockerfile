# syntax=docker/dockerfile:1
#
# Three stages: build the React app, build the Rust backend, then copy both
# into a small runtime image. Build:  docker build -t library-web-app .
# Run:  docker run -p 4000:4000 -v library-data:/data library-web-app

# ---- 1. React build -> /app/frontend-react/dist --------------------------------
FROM node:22-bookworm-slim AS react
WORKDIR /app/frontend-react
# Dependencies first, so editing source does not reinstall them.
COPY frontend-react/package.json frontend-react/package-lock.json ./
RUN npm ci
# `src/generated/` (TypeScript types from the shared crate) is committed, so
# the build needs no Rust toolchain.
COPY frontend-react/ ./
RUN npm run build

# ---- 2. Rust build -> /app/target/release/library-backend ----------------------
FROM rust:1-bookworm AS backend
WORKDIR /app
# The compile-time checked SQL macros read the committed `backend/.sqlx/`
# metadata instead of a live database.
ENV SQLX_OFFLINE=true
# `rust-toolchain.toml` is deliberately not copied: it asks for the wasm32
# target, which the server build does not need. The Leptos crate is a
# workspace member, so its manifest must exist, but it is never compiled.
COPY Cargo.toml Cargo.lock ./
COPY shared/ shared/
COPY backend/ backend/
COPY frontend-leptos/ frontend-leptos/
RUN cargo build --release --locked -p library-backend

# ---- 3. Runtime ----------------------------------------------------------------
FROM debian:bookworm-slim
# curl is only here for the HEALTHCHECK (the slim image has no HTTP client).
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home-dir /nonexistent --shell /usr/sbin/nologin app \
    && mkdir /data \
    && chown app:app /data

COPY --from=backend /app/target/release/library-backend /usr/local/bin/library-backend
COPY --from=react /app/frontend-react/dist /srv/frontend

ENV DATABASE_URL=sqlite:///data/library.db \
    BIND_ADDR=0.0.0.0:4000 \
    FRONTEND_DIR=/srv/frontend \
    COOKIE_SECURE=false \
    LOG_FORMAT=json

USER app
# The named volume keeps the SQLite file across container restarts.
VOLUME /data
EXPOSE 4000

HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl --fail --silent http://127.0.0.1:4000/api/health || exit 1

# No arguments means `serve`. Other subcommands are passed straight through, e.g.
# `docker run --rm -v library-data:/data library-web-app seed`
ENTRYPOINT ["library-backend"]
