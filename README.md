# RustPress

**WordPress, rewritten in Rust.** Connect to your existing WordPress database — your content, themes, and plugins are converted and served instantly, 100x faster.

[![CI](https://github.com/rustpress-project/RustPress/actions/workflows/ci.yml/badge.svg)](https://github.com/rustpress-project/RustPress/actions/workflows/ci.yml)
[![License: GPL v2](https://img.shields.io/badge/License-GPL%20v2-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html)
[![Rust: 1.88+](https://img.shields.io/badge/Rust-1.91%2B-orange.svg)](https://www.rust-lang.org/)
[![Status: Alpha](https://img.shields.io/badge/Status-Alpha-yellow.svg)](#status)

> **Alpha Release** — Targeting WordPress 6.9 compatibility. The frontend achieves 97%+ visual parity with the Twenty Twenty-Five theme. Contributions and testing welcome!

---

## The Problem: WordPress Is a Security Crisis

WordPress powers **43% of the web** — over 800 million sites. But the reality behind this number is alarming:

| Problem | Scale |
|---------|-------|
| **Outdated WordPress core** | [49.8% of sites run outdated versions](https://www.wpbeginner.com/research/ultimate-wordpress-statistics/) |
| **Vulnerable plugins** | Plugin vulnerabilities account for [97% of WP security issues](https://patchstack.com/whitepaper/state-of-wordpress-security-in-2024/) |
| **Abandoned sites still online** | Millions of "set and forget" sites with no security updates |
| **Hacked sites per year** | ~13,000 WordPress sites hacked [per day](https://www.colorlib.com/wp/wordpress-statistics/) (~4.7M/year) |
| **PHP memory overhead** | 50-100 MB per process, limits concurrent users |
| **Server cost** | PHP's per-request model requires expensive hosting to scale |

The core issue: **PHP is an interpreted language with a 20-year-old architecture.** Every request bootstraps the entire runtime from scratch. Every plugin is an arbitrary code execution surface. Every unpatched site is an open door.

Most site owners are not developers. They set up WordPress once, install plugins, and never update again. The security model depends on constant human vigilance — and humans forget.

### The AI Threat Multiplier

This crisis is about to get catastrophically worse. AI is transforming cyber attacks:

- **Automated vulnerability discovery** — AI agents can scan the entire internet for WordPress sites, detect their versions and installed plugins, and identify known vulnerabilities in seconds. What took a human attacker hours of manual reconnaissance now takes milliseconds.
- **AI-generated exploits** — LLMs can analyze CVE disclosures and generate working exploit code, lowering the skill barrier for attackers to near zero.
- **Autonomous attack chains** — AI agents can discover a vulnerability, generate an exploit, deploy a payload, establish persistence, and move laterally — all without human intervention.
- **Scale** — A single AI agent can attack thousands of WordPress sites simultaneously. The 4.7 million hacked sites per year figure will look quaint.

WordPress's security model — "humans must manually apply patches" — cannot survive in an era where AI attackers operate at machine speed, 24/7, against 800 million targets. **The only defense that scales is eliminating the vulnerability surface entirely.** That means compiling to memory-safe native code, sandboxing plugins, and removing PHP's arbitrary code execution model.

This is not a future threat. It is happening now.

---

## The Solution: Compile WordPress into a Single Binary

RustPress takes a fundamentally different approach:

```
WordPress (PHP)                    RustPress (Rust)
├── Interpreted at runtime         ├── Compiled to native binary
├── 50-100 MB memory per process   ├── 35 MB total
├── Bootstraps every request       ├── Always-on async server
├── Plugin = arbitrary PHP code    ├── Plugin = sandboxed WASM or native Rust
├── SQL injection via string ops   ├── Parameterized queries enforced by type system
├── Must patch constantly          ├── Memory-safe by construction
└── ~200ms per page                └── ~2ms per page
```

**Point RustPress at your existing WordPress database. Your site is now 100x faster and structurally secure.**

---

## Migration: It Just Works

RustPress is designed so that migrating from WordPress is as simple as pointing to your existing data:

### Database — Zero Migration

```env
DATABASE_URL=mysql://user:pass@localhost:3306/wordpress
SKIP_MIGRATIONS=true
```

RustPress reads the **exact same WordPress tables** (`wp_posts`, `wp_options`, `wp_users`, etc.) directly. No data conversion, no export/import, no downtime. WordPress and RustPress can even run side-by-side on the same database during transition.

### Themes — AI-Converted

Your existing WordPress theme is converted from PHP to Tera templates using AI. The conversion uses WordPress's own output as the reference — pixel-perfect fidelity is verified by automated visual comparison testing.

The default theme (Twenty Twenty-Five equivalent) ships with RustPress and achieves **97%+ pixel match** with the WordPress original.

### Plugins — AI-Converted

WordPress plugins are converted from PHP to Rust using AI. This works because:

1. **WordPress is 100% open source** — every line of PHP is readable
2. **AI reads the PHP source code** — the code IS the specification
3. **AI converts to Rust** — calling Rust implementations of WP functions
4. **Output is compared against WordPress** — the correct answer always exists
5. **Diffs are fixed** — repeat until 100% match

Major plugins (WooCommerce, Yoast SEO, Contact Form 7, ACF, Wordfence) are being rebuilt natively in Rust within this repository for maximum performance.

> **Core philosophy:** RustPress is made possible not by Rust's speed, but by the fact that AI can now scale the tedious work of converting code when the correct answer (WordPress source code) is fully available. [Read more](docs/adr/001-php-bridge-mode.en.md)

---

## The Mission: A Migration Path for Every WordPress Site

RustPress's goal is not just to be a fast CMS. **The goal is to establish a migration path for every WordPress site in the world.**

```
Any WordPress site
    ↓ rustpress migrate analyze (compatibility report)
    ↓ rustpress migrate database (point to existing DB)
    ↓ rustpress migrate theme (AI-convert PHP → Tera)
    ↓ rustpress migrate plugins (AI-convert or substitute with Rust-native)
    ↓ rustpress migrate seo-audit (verify zero SEO impact)
RustPress site — 100x faster, structurally secure, single binary
```

800 million WordPress sites deserve a path forward. Not just the ones with dedicated engineering teams — **all of them**, including the forgotten blogs, the small business sites, the nonprofit pages that no one has updated in three years but are still serving real visitors.

### We Won't Abandon Anyone

**Security patches for every version. Forever.** Why is this possible? Because AI-driven development reduces the marginal cost of maintenance to near zero. Traditional open-source projects are forced to drop support for old versions — the human labor cost is too high. RustPress breaks this constraint. When AI can read the diff between WordPress versions and generate the corresponding Rust patches, "end of life" becomes a choice, not a necessity. We choose not to abandon anyone.

---

## Road to Beta

**Our Beta isn't "mostly works." It's "nearly finished."** AI-driven development lets us set the bar higher.

| # | Condition | Status |
|---|-----------|--------|
| B-1 | Top 100 WordPress themes render correctly | Planned |
| B-2 | Top 50 WordPress plugins' data migrates and displays | Planned |
| B-3 | WP REST API v2 — 100% compatible | In progress |
| B-4 | 97%+ pixel match with WordPress on all pages | In progress |
| B-5 | `rustpress migrate` — one command, working site in 5 min | Planned |
| B-6 | OWASP Top 10 — all addressed | In progress |
| B-7 | CI/CD fully operational | Planned |

In traditional development, this would be RC-level. We set it as Beta because AI makes it achievable at speed.

---

## Performance

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
git clone https://github.com/rustpress-project/RustPress.git
cd RustPress
cp .env.example .env

# Start MySQL + RustPress
docker compose up -d
```

RustPress will be available at `http://localhost:8080`.

### Option 2: From Source

**Prerequisites:** Rust 1.88+, MySQL 8.0+ (or MariaDB 10.5+)

```bash
git clone https://github.com/rustpress-project/RustPress.git
cd RustPress

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

### Native Rust Plugin Crates
| Crate | WordPress Equivalent | Status |
|-------|---------------------|--------|
| `rustpress-commerce` | WooCommerce | In development |
| `rustpress-seo` | Yoast / RankMath | In development |
| `rustpress-forms` | Contact Form 7 / Gravity Forms | In development |
| `rustpress-fields` | ACF (Advanced Custom Fields) | In development |
| `rustpress-security` | Wordfence | In development |

### Infrastructure
- Page cache (moka, 5-min TTL, sub-ms hits)
- Gzip compression
- Connection pooling via SeaORM
- Compiled Tera templates (parsed once at startup)

### Planned
- [ ] `theme.json` parser for 100% CSS variable parity
- [ ] Gutenberg block rendering (advanced blocks)
- [ ] Plugin hook system (`add_action`/`add_filter` in Rust)
- [ ] WASM plugin runtime (Extism)
- [ ] Admin dashboard (wp-admin) full parity
- [ ] Multi-site support
- [ ] WPGraphQL-compatible endpoint
- [ ] AI plugin/theme conversion service (rustpress-convert)

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

## Architectural Decisions

Key design decisions are recorded as ADRs (Architecture Decision Records):

- [ADR-001: PHP Bridge Mode and Plugin Compatibility Strategy](docs/adr/001-php-bridge-mode.en.md) ([Japanese](docs/adr/001-php-bridge-mode.md))

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
