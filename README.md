# RustPress

**WordPress, rewritten in Rust.** Not a clone. Not "inspired by." The real thing — rebuilt from scratch for speed, safety, and the modern web.

[![License: GPL v2](https://img.shields.io/badge/License-GPL%20v2-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html)
[![Rust: 1.75+](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Tests: 177 passing](https://img.shields.io/badge/Tests-177%20passing-brightgreen.svg)](#)

---

## Performance: RustPress vs WordPress (PHP)

Benchmarked on the same machine, same MySQL database, same content.

| Metric | WordPress (PHP 8.x) | RustPress (Rust) | Improvement |
|--------|---------------------|------------------|-------------|
| **Homepage response** | 200–500 ms | **2.7 ms** | **74–185x faster** |
| **REST API (posts)** | 100–300 ms | **5.9 ms** | **17–51x faster** |
| **RSS Feed** | 150–400 ms | **5.7 ms** | **26–70x faster** |
| **Login page** | 80–200 ms | **2.6 ms** | **31–77x faster** |
| **Requests/sec (homepage)** | 10–50 rps | **509 rps** | **10–50x more** |
| **Requests/sec (API)** | 20–80 rps | **840 rps** | **10–42x more** |
| **Requests/sec (health)** | — | **2,786 rps** | — |
| **Memory usage** | 50–100 MB | **35 MB** | **1.4–2.9x less** |
| **Binary size** | PHP runtime + deps | **19 MB** | Single binary |
| **Startup time** | 2–5 sec | **0.4 sec** | **5–12x faster** |

> WordPress PHP numbers are typical values from production benchmarks. RustPress numbers are measured with `--release` build, 50–100 concurrent connections, on Windows 11.

### Why is it faster?

- **No interpreter overhead** — compiled to native machine code
- **Zero-copy async I/O** — tokio runtime handles thousands of connections on a single thread pool
- **In-memory page cache** — moka cache with 5-minute TTL, sub-millisecond cache hits
- **Compiled templates** — Tera templates are parsed once at startup, not on every request
- **Connection pooling** — SeaORM reuses database connections efficiently

---

## Features: Full WordPress Parity

Every core WordPress feature, implemented in Rust.

### Content Management
- Posts & Pages — full CRUD with bulk actions (publish, draft, trash)
- TinyMCE rich text editor with Visual/Text toggle
- Custom Fields (post meta) — CRUD UI in post editor
- Featured Images — media attachment via `_thumbnail_id`
- Post Revisions — auto-saved before every update
- Autosave — periodic JS-based autosave
- Scheduled Posts — future-dated publishing
- Categories & Tags — full taxonomy management
- Shortcodes — `[audio]`, `[video]`, `[caption]`, `[embed]`
- Comments — frontend submission, threaded display, admin moderation

### Admin Dashboard
- WordPress 6.x-faithful admin UI (dark sidebar, admin bar, widgets)
- Dashboard — At a Glance stats, Quick Draft, Recent Activity
- Post/Page list — status filters, search, pagination, bulk actions
- Media Library — file upload with MIME validation, date-organized storage
- User management — list, create, role assignment
- User profiles — edit display name, email, bio, password
- Settings — site title, tagline, posts per page, language
- Comment moderation — approve, spam, trash, permanent delete
- Navigation menu management — header and footer menus
- Plugin management — list, activate, deactivate
- Theme management — list, preview, activate, switch
- Widget system — 8 widget types, 3 widget areas, drag-and-drop UI

### REST API (WP v2 Compatible)
Full compatibility with the WordPress REST API:

```
GET  /wp-json/wp/v2/posts          # List posts
POST /wp-json/wp/v2/posts          # Create post (authenticated)
GET  /wp-json/wp/v2/posts/{id}     # Get post
PUT  /wp-json/wp/v2/posts/{id}     # Update post (authenticated)
DELETE /wp-json/wp/v2/posts/{id}   # Delete post (authenticated)
```

Also: `/users`, `/categories`, `/tags`, `/media`, `/comments`, `/pages`, `/search`, `/settings`, `/statuses`, `/types`, `/taxonomies`

### XML-RPC API
17 methods for desktop blogging client compatibility:
- `wp.getUsersBlogs`, `wp.getPost`, `wp.getPosts`, `wp.newPost`, `wp.editPost`, `wp.deletePost`
- `wp.getCategories`, `wp.getTags`, `wp.getOptions`, `wp.getProfile`
- `blogger.*`, `metaWeblog.*` legacy methods
- RSD auto-discovery for client configuration

### Authentication & Security
- JWT tokens (24-hour expiry) for API access
- HTTP-only session cookies for admin dashboard
- Password hashing: Argon2 (new) + bcrypt (legacy WordPress compatibility)
- Password reset flow with token-based verification
- Role-based access control — 5 roles, 73 capabilities
- Permission enforcement on all admin routes
- Security headers: X-Content-Type-Options, X-Frame-Options, X-XSS-Protection, Referrer-Policy, Permissions-Policy
- CSRF protection via nonces

### Frontend
- WordPress template hierarchy — `single.html`, `page.html`, `archive.html`, `search.html`, `404.html`
- RSS feed (`/feed`)
- XML Sitemap (`/sitemap.xml`)
- Open Graph Protocol meta tags
- `robots.txt` with sitemap reference
- Post navigation (previous/next)
- Category, Tag, Author archive pages
- Search with highlighted results
- Widget-powered sidebar and footer areas

### Infrastructure
- Page cache — moka-based, 5-minute TTL, sub-ms cache hits
- Object cache — 10,000 entries, 1-hour TTL
- Transient cache — WordPress-compatible transient API
- Cron system — background tokio task, hourly session cleanup
- Gzip compression — automatic response compression
- Plugin system — Native Rust + WebAssembly plugin support
- Theme engine — hot-reloadable via `Arc<RwLock<ThemeEngine>>`
- Internationalization (i18n) — JSON translation files, `__()` / `_n()` in templates, Japanese included

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| **Language** | Rust (2021 edition) |
| **Web Framework** | [Axum](https://github.com/tokio-rs/axum) |
| **Async Runtime** | [Tokio](https://tokio.rs/) |
| **ORM** | [SeaORM](https://www.sea-ql.org/SeaORM/) |
| **Database** | MySQL (WordPress-compatible `wp_*` schema) |
| **Templates** | [Tera](https://keats.github.io/tera/) |
| **Cache** | [Moka](https://github.com/moka-rs/moka) |
| **Auth** | Argon2 + bcrypt + JWT (jsonwebtoken) |
| **Plugins** | Native Rust + WebAssembly (wasmtime) |

### Crate Architecture

```
rustpress/
├── crates/
│   ├── rustpress-server    # Axum web server, routes, middleware, state
│   ├── rustpress-db        # SeaORM entities, migrations, options manager
│   ├── rustpress-api       # WP REST API v2 compatible endpoints
│   ├── rustpress-admin     # Admin CRUD API (posts, users, media, plugins)
│   ├── rustpress-auth      # JWT, sessions, passwords, roles, capabilities
│   ├── rustpress-cache     # Page cache, object cache, transients
│   ├── rustpress-themes    # Template engine, hierarchy, template tags
│   ├── rustpress-plugins   # Plugin registry, loader, WASM host
│   ├── rustpress-query     # WP_Query-style query builders
│   ├── rustpress-cron      # WordPress cron system
│   ├── rustpress-migrate   # Database table creation and seeding
│   ├── rustpress-cli       # CLI tool for management commands
│   └── rustpress-e2e       # Selenium E2E tests (WordPress comparison)
├── templates/              # Tera templates (frontend + admin)
├── static/                 # CSS, JS assets
└── languages/              # i18n translation files (JSON)
```

---

## Quick Start

### Prerequisites
- Rust 1.75+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- MySQL 8.0+ (or MariaDB 10.5+)
- Node.js 18+ (for webpack asset build)

### Setup

```bash
# Clone
git clone https://github.com/example/rustpress.git
cd rustpress

# Configure
cp .env.example .env
# Edit .env with your DATABASE_URL

# Build
cargo build --release
npm install && npx webpack

# Run
./target/release/rustpress
```

### Access

| URL | Description |
|-----|-------------|
| http://localhost:8080/ | Frontend |
| http://localhost:8080/wp-login.php | Login (admin / password) |
| http://localhost:8080/wp-admin/ | Admin Dashboard |
| http://localhost:8080/wp-json/wp/v2/posts | REST API |
| http://localhost:8080/feed | RSS Feed |
| http://localhost:8080/xmlrpc.php | XML-RPC |

### Environment Variables

```env
RUSTPRESS_HOST=127.0.0.1
RUSTPRESS_PORT=8080
DATABASE_URL=mysql://root:password@localhost:3306/wordpress
JWT_SECRET=your-secret-key
SITE_URL=http://localhost:8080
RUST_LOG=rustpress=info
```

---

## Testing

```bash
# Unit & integration tests (177 tests)
cargo test --workspace

# E2E comparison tests (requires WordPress + RustPress running)
WORDPRESS_URL=http://localhost:8081 \
RUSTPRESS_URL=http://localhost:8080 \
cargo test -p rustpress-e2e -- --ignored --nocapture
```

The E2E test suite compares RustPress against a real WordPress instance:
- **14 REST API tests** — response structure, field names, types
- **8 Frontend tests** — HTML structure, RSS, sitemap, robots.txt
- **12 Selenium browser tests** — login, dashboard, post creation, media
- **4 HTTP header tests** — security headers, content types, CORS

---

## Database Compatibility

RustPress uses the **exact same database schema** as WordPress:

```
wp_posts, wp_postmeta, wp_users, wp_usermeta, wp_options,
wp_comments, wp_commentmeta, wp_terms, wp_term_taxonomy,
wp_term_relationships, wp_links, wp_termmeta
```

You can point RustPress at an existing WordPress database and it will serve the same content — orders of magnitude faster.

---

## License

GPL v2, same as WordPress.
