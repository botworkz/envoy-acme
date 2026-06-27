# envoy-acme developer Makefile.
#
# This is a prototype; targets wrap the usual cargo + docker compose workflow.

CARGO ?= cargo
COMPOSE ?= docker compose

.DEFAULT_GOAL := help

.PHONY: help
help: ## Show this help.
	@grep -hE '^[a-zA-Z0-9_-]+:.*?## ' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

.PHONY: build
build: ## Build the binary (debug).
	$(CARGO) build

.PHONY: release
release: ## Build the binary (release).
	$(CARGO) build --release

.PHONY: test
test: ## Run the test suite.
	$(CARGO) test

.PHONY: lint
lint: ## Run clippy with warnings denied.
	$(CARGO) clippy --all-targets -- -D warnings

.PHONY: fmt
fmt: ## Format the code.
	$(CARGO) fmt

.PHONY: fmt-check
fmt-check: ## Check formatting without writing changes.
	$(CARGO) fmt --check

.PHONY: check
check: fmt-check lint test ## Run fmt-check, lint and tests.

.PHONY: run
run: ## Run locally against config/example.yaml.
	$(CARGO) run -- --config config/example.yaml

.PHONY: docker-build
docker-build: ## Build the Docker image.
	docker build -t envoy-acme:dev .

.PHONY: up
up: ## Start the local compose stack.
	$(COMPOSE) up --build

.PHONY: down
down: ## Tear down the local compose stack.
	$(COMPOSE) down -v

.PHONY: clean
clean: ## Remove build artifacts.
	$(CARGO) clean
