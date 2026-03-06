# ─────────────────────────────────────────────────────────────────────────────
#  RustPress Makefile
# ─────────────────────────────────────────────────────────────────────────────

.PHONY: dev build test e2e e2e-up e2e-down e2e-logs docker-build

# ── Development ──────────────────────────────────────────────────────────────

## Run RustPress locally (requires MySQL at localhost:3306)
dev:
	cargo run -p rustpress-server

## Build release binary
build:
	cargo build --release -p rustpress-server

## Run unit + integration tests
test:
	cargo test --workspace

# ── Docker E2E ────────────────────────────────────────────────────────────────

## Build RustPress Docker image
docker-build:
	docker compose build rustpress

## Start MySQL + WordPress + RustPress in background (for manual testing)
e2e-up:
	docker compose up -d db wordpress rustpress
	@echo ""
	@echo "  WordPress  → http://localhost:8081"
	@echo "  RustPress  → http://localhost:8080"
	@echo ""
	@echo "Wait 30-60s for WordPress to finish initializing."
	@echo "Then run: make e2e"

## Run E2E comparison tests (requires e2e-up to be running)
e2e:
	docker compose run --rm \
		-e WORDPRESS_URL=http://wordpress:80 \
		-e RUSTPRESS_URL=http://rustpress:3000 \
		-e ADMIN_USER=admin \
		-e ADMIN_PASSWORD=password \
		e2e

## Run everything end-to-end: build, start, test, tear down
e2e-all:
	docker compose --profile e2e up --build --abort-on-container-exit --exit-code-from e2e
	docker compose down

## Stop and remove all E2E containers + volumes
e2e-down:
	docker compose down -v

## Follow logs from all services
e2e-logs:
	docker compose logs -f
