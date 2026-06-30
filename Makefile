.PHONY: fmt clippy build test coverage check up down logs

fmt:
	cargo fmt --all

clippy:
	cargo clippy --all-targets -- -D warnings

build:
	cargo build --release --target=x86_64-unknown-linux-gnu

test:
	cargo test

coverage:
	# --exclude-files: see CONTRIBUTING.md "Coverage exclusions" — reserved
	# for code that genuinely cannot be unit-tested (currently only FFI
	# entry points in src/lib.rs).
	cargo tarpaulin \
		--workspace \
		--exclude-files 'src/lib.rs' \
		--exclude-files 'src/test_stubs.rs' \
		--exclude-files 'src/acme/account_test_server.rs' \
		--timeout 180 \
		--out Html \
		--output-dir target/tarpaulin \
		--skip-clean

check: fmt clippy test

up:
	docker compose up --build -d

down:
	docker compose down -v

logs:
	docker compose logs -f --tail=200
