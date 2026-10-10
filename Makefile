.PHONY: setup db dev-backend dev-react dev-leptos seed user test lint fmt types sqlx-prepare build check

# One-time developer setup. The Rust tools are not installed for you: the
# targets below only print a hint when one is missing.
setup: ## Check tools, install npm deps, create the dev database
	@command -v sqlx >/dev/null || echo "missing sqlx-cli: cargo install sqlx-cli --no-default-features --features sqlite,rustls"
	@command -v trunk >/dev/null || echo "missing trunk (Leptos only): cargo install trunk --locked"
	@rustup target list --installed | grep -q wasm32-unknown-unknown || echo "missing wasm target (Leptos only): rustup target add wasm32-unknown-unknown"
	@test -f .env || { cp .env.example .env; echo "created .env from .env.example"; }
	cd frontend-react && npm ci
	$(MAKE) db

db: ## Create and migrate the dev database
	sqlx database create
	sqlx migrate run --source backend/migrations

dev-backend:
	cargo run -p library-backend

dev-react: ## React dev server on :5173 (proxies /api to :4000)
	cd frontend-react && npm run dev

dev-leptos: ## Leptos dev server on :8080 (proxies /api to :4000)
	cd frontend-leptos && trunk serve

seed: ## Insert sample data into an empty database
	cargo run -p library-backend -- seed

# Usage: make user NAME=alice   (prompts for the password twice)
user: ## Create a librarian account (prompts for the password)
	cargo run -p library-backend -- create-user $(or $(NAME),admin)

test:
	cargo test --workspace

lint:
	cargo clippy --workspace --all-targets -- -D warnings

fmt:
	cargo fmt

types: ## Regenerate TypeScript types from the shared crate
	cargo test -p library-shared --features ts

# sqlx-cli runs from backend/, so give it an absolute path to the dev DB.
sqlx-prepare: ## Refresh committed offline query metadata
	cd backend && DATABASE_URL=sqlite://$(CURDIR)/data/dev.db cargo sqlx prepare

build: ## Release backend + both frontends
	cargo build --release -p library-backend
	cd frontend-react && npm run build
	cd frontend-leptos && trunk build --release

check: ## Every gate CI runs
	cargo fmt --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo clippy -p library-frontend-leptos --target wasm32-unknown-unknown -- -D warnings
	cargo test --workspace
	SQLX_OFFLINE=true cargo build -p library-backend
	cd frontend-react && npm run lint && npm run typecheck && npm test
