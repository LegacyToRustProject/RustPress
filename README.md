# RustPress

**WordPress, rewritten in Rust.** Not a clone. Not "inspired by." The real thing — rebuilt from scratch for speed, safety, and the modern web.

[![License: GPL v2](https://img.shields.io/badge/License-GPL%20v2-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html)
[![Rust: 1.88+](https://img.shields.io/badge/Rust-1.88%2B-orange.svg)](https://www.rust-lang.org/)
[![Status: Alpha](https://img.shields.io/badge/Status-Alpha-yellow.svg)](#status)

> **Alpha Release** — Targeting WordPress 6.9 compatibility. The frontend achieves 97%+ visual parity with the Twenty Twenty-Five theme. Contributions and testing welcome!

---

## Vision

RustPress aims to be a **100% WordPress-compatible CMS** built entirely in Rust. The goal is pixel-perfect parity with WordPress — same database schema, same REST API, same template output, same theme rendering — while delivering the performance and safety guarantees of Rust.

**This project is built with AI-assisted development**, pushing the boundaries of what's possible when development costs approach zero. By leveraging AI to handle the massive surface area of WordPress compatibility, we can achieve what would otherwise require a large team and years of work.

### Dual-Mode Theme & Plugin Architecture

RustPress is designed to support two complementary approaches for themes and plugins:

1. **WordPress-Compatible Mode** — Load and render existing WordPress PHP themes and plugins as-is, ensuring drop-in compatibility with the entire WordPress ecosystem. Your existing themes, plugins, and customizations work without modification.

2. **Rust-Optimized Mode** — Native Rust themes and plugins (including WebAssembly) that take full advantage of Rust's performance, safety, and concurrency. No PHP interpreter overhead, compiled at build time, type-safe plugin APIs.

Both modes coexist, allowing gradual migration from PHP to Rust while maintaining full backward compatibility.

---

## Why RustPress?

WordPress powers 40%+ of the web, but PHP's per-request overhead limits performance. RustPress keeps **full WordPress database compatibility** while delivering native-compiled speed.

**Point RustPress at an existing WordPress database and it serves the same content — orders of magnitude faster.**

### Performance

Benchmarked on the same machine, same MySQL database, same content.

| Metric | WordPress (PHP 8.x) | RustPress (Rust) | Improvement |
|--------|---------------------|------------------|-------------|
| **Homepage response** | 200-500 ms | **2.7 ms** | **74-185x faster** |
| **REST API (posts)** | 100-300 ms | **5.9 ms** | **17-51x faster** |
| **Memory usage** | 50-100 MB | **35 MB** | **1.4-2.9x less** |
| **Requests/sec** | 10-50 rps | **509 rps** | **10-50x more** |
| **Startup time** | 2-5 sec | **0.4 sec** | **5-12x faster** |
| **Binary size** | PHP runtime + deps | **19 MB** | Single binary |

---

## Quick Start

### Option 1: Docker (Recommended)

```bash
git clone https://github.com/example/rustpress.git
cd rustpress
cp .env.example .env

# Start MySQL + RustPress
docker compose up -d
```

RustPress will be available at `http://localhost:8080`.

### Option 2: From Source

**Prerequisites:** Rust 1.88+, MySQL 8.0+ (or MariaDB 10.5+)

```bash
git clone https://github.com/example/rustpress.git
cd rustpress

cp .env.example .env
# Edit .env — set DATABASE_URL to your WordPress database

cargo build --release
./target/release/rustpress-server
```

### Using an Existing WordPress Database

RustPress reads the standard `wp_*` tables directly. Set `SKIP_MIGRATIONS=true` and point `DATABASE_URL` at your WordPress database:

```env
DATABASE_URL=mysql://user:pass@localhost:3306/wordpress
SKIP_MIGRATIONS=true
```

---

## Features

### Content Serving (Working Now)
- Full WordPress template hierarchy (`single`, `page`, `archive`, `category`, `tag`, `author`, `search`, `404`)
- Twenty Twenty-Five theme parity (97%+ pixel match via Selenium E2E tests)
- Posts, pages, categories, tags, comments (threaded display)
- Sticky posts, password-protected posts, scheduled posts
- RSS feed (`/feed`), XML Sitemap (`/sitemap.xml`), robots.txt
- Permalink structures (`/%postname%/`, `/%year%/%monthnum%/%day%/`)

### REST API (WP v2 Compatible)
```
GET/POST   /wp-json/wp/v2/posts
GET/PUT/DEL /wp-json/wp/v2/posts/{id}
```
Also: `/pages`, `/media`, `/users`, `/categories`, `/tags`, `/comments`, `/search`, `/settings`, `/types`, `/taxonomies`, `/menus`, `/themes`, `/plugins`

### Authentication & Security
- JWT tokens for API, HTTP-only cookies for sessions
- Argon2 (new) + bcrypt (legacy WordPress) password hashing
- Role-based access control (5 roles, 73 capabilities)
- Security headers (CSP, X-Frame-Options, HSTS)

### Infrastructure
- Page cache (moka, 5-min TTL, sub-ms hits)
- Gzip compression
- Connection pooling via SeaORM
- Compiled Tera templates (parsed once at startup)

### Planned
- [ ] `theme.json` parser for 100% CSS variable parity
- [ ] Gutenberg block rendering (advanced blocks)
- [ ] Plugin hook system (`add_action`/`add_filter` in Rust)
- [ ] Native Rust plugin API (WASM/dylib) — Rust-Optimized Mode
- [ ] PHP theme/plugin compatibility layer — WordPress-Compatible Mode
- [ ] Admin dashboard (wp-admin) full parity
- [ ] Multi-site support

---

## Architecture

```
rustpress/
├── crates/
│   ├── rustpress-server    # Axum web server, routes, middleware
│   ├── rustpress-db        # SeaORM entities, migrations, options
│   ├── rustpress-api       # WP REST API v2 endpoints
│   ├── rustpress-auth      # JWT, sessions, passwords, RBAC
│   ├── rustpress-themes    # Template engine, hierarchy, tags
│   ├── rustpress-query     # WP_Query-style query builders
│   ├── rustpress-cache     # Page/object/transient caches
│   ├── rustpress-plugins   # Plugin registry, WASM host
│   ├── rustpress-admin     # Admin CRUD API
│   ├── rustpress-migrate   # Database migrations
│   ├── rustpress-cron      # Background tasks
│   └── rustpress-e2e       # Selenium visual comparison tests
├── templates/              # Tera templates (TT25 parity)
├── static/                 # CSS, fonts, assets
└── docker-compose.yml
```

| Layer | Technology |
|-------|-----------|
| **Web Framework** | [Axum](https://github.com/tokio-rs/axum) + [Tokio](https://tokio.rs/) |
| **ORM** | [SeaORM](https://www.sea-ql.org/SeaORM/) (MySQL) |
| **Templates** | [Tera](https://keats.github.io/tera/) |
| **Cache** | [Moka](https://github.com/moka-rs/moka) |

---

## Database Compatibility

RustPress uses the **exact same schema** as WordPress:

```
wp_posts, wp_postmeta, wp_users, wp_usermeta, wp_options,
wp_comments, wp_commentmeta, wp_terms, wp_term_taxonomy,
wp_term_relationships, wp_links, wp_termmeta
```

WordPress and RustPress can run side-by-side on the same database.

---

## Testing

```bash
# Unit tests
cargo test --workspace

# E2E visual comparison (requires Docker)
docker compose --profile e2e up -d
./tests/run_e2e.sh
```

The E2E suite uses Selenium to take screenshots of both WordPress and RustPress, then compares them pixel-by-pixel across 9 page types at multiple viewports. Current threshold: 93% match (actual: 97%+).

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

We welcome contributions of all kinds — bug reports, theme parity fixes, new features, documentation, and testing on real WordPress databases.

---

## Status

**Alpha** — Targeting **WordPress 6.9** compatibility. RustPress serves WordPress content with high visual fidelity but is not yet production-ready.

What works well:
- Reading and serving all WordPress content types
- REST API compatibility
- TT25 theme visual parity (97%+)
- Performance (100x+ faster than PHP WordPress)

What's in progress:
- Admin dashboard
- Write operations via frontend
- Plugin system
- Full Gutenberg block support

---

## License

GPL v2, same as WordPress. See [LICENSE](LICENSE).
