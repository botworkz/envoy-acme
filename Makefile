.PHONY: fmt clippy build test check up down logs

fmt:
	cargo fmt --all

clippy:
	cargo clippy --all-targets -- -D warnings

build:
	cargo build --release --target=x86_64-unknown-linux-gnu

test:
	cargo test

check: fmt clippy test

up:
	docker compose up --build -d

down:
	docker compose down -v

logs:
	docker compose logs -f --tail=200
