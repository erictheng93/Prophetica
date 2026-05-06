.PHONY: doctor bootstrap dev down test lint fmt proto migrate seed clean

BASH ?= bash

doctor:
	$(BASH) ./tools/scripts/check-versions.sh

bootstrap:
	$(BASH) ./tools/scripts/bootstrap.sh

dev:
	docker compose -f infra/docker-compose.local.yaml up -d

down:
	docker compose -f infra/docker-compose.local.yaml down -v

test:
	pnpm turbo run test
	cargo nextest run --workspace --locked

lint:
	pnpm turbo run lint
	cargo clippy --workspace --all-targets --locked -- -D warnings
	cargo fmt --all -- --check

fmt:
	cargo fmt --all
	pnpm turbo run format

proto:
	pnpm exec buf generate

migrate:
	$(BASH) ./tools/scripts/migrate.sh up

seed:
	$(BASH) ./tools/scripts/seed.sh

clean:
	rm -rf node_modules apps/*/.next packages/proto/gen crates/proto/src/gen target
	cargo clean
