.PHONY: db dev-backend test lint fmt types sqlx-prepare check

db: ## Create and migrate the dev database
	sqlx database create
	sqlx migrate run --source backend/migrations

dev-backend:
	cargo run -p library-backend

test:
	cargo test --workspace

lint:
	cargo clippy --workspace --all-targets -- -D warnings

fmt:
	cargo fmt

types: ## Regenerate TypeScript types from the shared crate
	cargo test -p library-shared --features ts

sqlx-prepare: ## Refresh committed offline query metadata
	# sqlx-cli runs from backend/, so give it an absolute path to the dev DB.
	cd backend && DATABASE_URL=sqlite://$(CURDIR)/data/dev.db cargo sqlx prepare

check:
	cargo fmt --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace
	SQLX_OFFLINE=true cargo build -p library-backend
