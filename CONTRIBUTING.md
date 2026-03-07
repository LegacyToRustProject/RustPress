# Contributing to RustPress

Thank you for your interest in contributing to RustPress! This project aims to build a 100% WordPress-compatible CMS in Rust, and every contribution helps us get closer to that goal.

## Getting Started

1. Fork and clone the repository
2. Copy `.env.example` to `.env` and configure your MySQL connection
3. Run `cargo build` to verify everything compiles
4. Run `cargo test --workspace` to verify tests pass

## Development Setup

### Requirements

- Rust 1.91+ (stable)
- MySQL 8.0+ or MariaDB 10.5+
- Docker (optional, for E2E testing)

### Running Locally

```bash
cp .env.example .env
# Edit .env with your database credentials
cargo run -p rustpress-server
```

### Running E2E Tests

E2E tests compare RustPress output against a real WordPress instance:

```bash
# Start WordPress + MySQL + Selenium via Docker
docker-compose up -d

# Run E2E tests
./tests/run_e2e.sh
```

## Workflow

### For Team Members (#01-#09)

Each team member works in a **separate clone** of the repository:

```bash
git clone https://github.com/LegacyToRustProject/RustPress.git ~/RustPress-<role>
cd ~/RustPress-<role>
git checkout -b feat/<feature-name>
```

**Rules:**

1. **Never push directly to `main`.** All changes go through PRs.
2. **Work on feature branches.** e.g., `feat/theme-compat`, `fix/logout-session`
3. **PRs require two approvals:** QA review (#09) + project owner.
4. **`main` must always be green.** CI (check, test, fmt, clippy) must pass.
5. **Pull before you test.** Always `git pull origin main` to get the latest code.

### For External Contributors

1. Fork the repository
2. Create a branch from `main`
3. Make your changes
4. Ensure CI checks pass (see below)
5. Submit a pull request using the PR template

### CI Requirements

All PRs must pass these checks before merge:

```bash
cargo check --workspace
cargo test --workspace --lib --bins
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

## Reporting Bugs

- Use the [bug report template](.github/ISSUE_TEMPLATE/bug_report.md)
- Check existing issues first
- Include steps to reproduce
- If it's a WordPress parity issue, include the WordPress output for comparison

## Feature Requests

- Use the [feature request template](.github/ISSUE_TEMPLATE/feature_request.md)
- Explain the motivation and WordPress behavior if applicable

## WordPress Parity Testing

One of the most valuable contributions is testing RustPress against WordPress and reporting differences. If you find a page, API response, or behavior that doesn't match WordPress, please file an issue with:

- The URL/endpoint tested
- WordPress output (screenshot or response body)
- RustPress output (screenshot or response body)
- WordPress version and theme used

## Areas Where Help is Needed

- **Theme compatibility** - Testing more WordPress themes beyond TT25
- **Plugin compatibility** - Identifying commonly-used plugin APIs to implement
- **REST API parity** - Comparing response formats with WordPress
- **CSS/visual parity** - Finding and fixing rendering differences
- **Performance benchmarks** - Testing under various workloads

## Code Style

- Follow standard Rust conventions (`cargo fmt`)
- Use `cargo clippy` to catch common issues
- Keep functions focused and small
- Prefer descriptive names over comments
- Don't add unnecessary abstractions

## Crate Structure

Each crate has a specific responsibility. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full architecture overview.

| Crate | Role |
|-------|------|
| `rustpress-server` | HTTP routes, middleware, server startup |
| `rustpress-db` | Database entities, queries, options |
| `rustpress-api` | REST API endpoints (WP v2 compatible) |
| `rustpress-admin` | Admin dashboard backend |
| `rustpress-auth` | Authentication, sessions, RBAC |
| `rustpress-themes` | Template rendering, template tags |
| `rustpress-plugins` | Plugin loading and execution |
| `rustpress-cache` | Caching layers |
| `rustpress-query` | WP_Query-style query building |
| `rustpress-e2e` | End-to-end tests |

## License

By contributing, you agree that your contributions will be licensed under GPL v2.
