# RustPress Architecture

## Overview

RustPress is a WordPress-compatible CMS written in Rust. It connects to an existing WordPress MySQL database and serves the same content with near-identical output.

```
                    HTTP Request
                         |
                         v
                  ┌─────────────┐
                  │   Axum       │  rustpress-server
                  │   Router     │  Routes, middleware, static files
                  └──────┬──────┘
                         |
          ┌──────────────┼──────────────┐
          v              v              v
   ┌────────────┐ ┌────────────┐ ┌────────────┐
   │ Frontend   │ │ REST API   │ │ Admin      │
   │ Routes     │ │ /wp-json/  │ │ /wp-admin/ │
   └─────┬──────┘ └─────┬──────┘ └─────┬──────┘
         |               |              |
         v               v              v
   ┌────────────┐ ┌────────────┐ ┌────────────┐
   │ rustpress- │ │ rustpress- │ │ rustpress- │
   │ themes     │ │ api        │ │ admin      │
   │ (Tera)     │ │ (WP v2)    │ │            │
   └─────┬──────┘ └─────┬──────┘ └─────┬──────┘
         |               |              |
         └───────────────┼──────────────┘
                         v
                  ┌─────────────┐
                  │ rustpress-  │  WP_Query-style query builder
                  │ query       │
                  └──────┬──────┘
                         v
                  ┌─────────────┐
                  │ rustpress-  │  SeaORM entities, options API
                  │ db          │
                  └──────┬──────┘
                         v
                  ┌─────────────┐
                  │   MySQL     │  WordPress-compatible schema
                  └─────────────┘
```

## Crate Map

| Crate | Role |
|-------|------|
| `rustpress-server` | HTTP server, routing, middleware, static files |
| `rustpress-db` | Database entities (SeaORM), wp_options API |
| `rustpress-core` | Shared types, formatting (wpautop, shortcodes) |
| `rustpress-query` | WP_Query-compatible query builder |
| `rustpress-themes` | Tera template rendering, template tags |
| `rustpress-api` | WordPress REST API v2 endpoints |
| `rustpress-admin` | Admin dashboard backend |
| `rustpress-auth` | Authentication, sessions, JWT, RBAC |
| `rustpress-plugins` | Plugin loading (Rust-native + WASM) |
| `rustpress-cache` | Moka-based caching layer |
| `rustpress-cron` | Scheduled tasks (wp-cron equivalent) |
| `rustpress-migrate` | Database migration tools |
| `rustpress-i18n` | Internationalization |
| `rustpress-blocks` | Gutenberg block parsing |
| `rustpress-e2e` | End-to-end visual comparison tests |
| `rustpress-cli` | CLI tool |

### Plugin Crates

| Crate | WordPress Equivalent |
|-------|---------------------|
| `rustpress-commerce` | WooCommerce |
| `rustpress-seo` | Yoast SEO |
| `rustpress-forms` | Contact Form 7 |
| `rustpress-fields` | Advanced Custom Fields |
| `rustpress-security` | Wordfence |
| `rustpress-multisite` | WordPress Multisite |

## Key Design Decisions

### SKIP_MIGRATIONS Mode
RustPress can share WordPress's existing database directly. When `SKIP_MIGRATIONS=true`, it reads from the same `wp_posts`, `wp_options`, etc. tables that WordPress uses.

### Template System
WordPress uses PHP templates; RustPress uses Tera (Jinja2-like). Theme conversion is required — PHP themes do not run directly. The `templates/` directory contains Tera templates organized by theme.

### Plugin System
Plugins are Rust-native or WASM. PHP plugins are not supported directly. A separate conversion service (`rustpress-convert`) provides AI-powered PHP-to-Rust plugin conversion.

### Authentication
- **Sessions**: UUID-based session cookies (HttpOnly, SameSite=Lax)
- **API**: JWT tokens (built-in, no plugin required)
- **Passwords**: WordPress-compatible phpass verification + argon2 for new passwords

## Directory Structure

```
RustPress/
├── crates/              # All Rust crates
│   ├── rustpress-server/    # Main binary
│   └── ...
├── templates/           # Tera templates
│   └── admin/           # Admin panel templates
├── themes/              # Theme assets and templates
│   └── twentytwentyfive/
├── static/              # Static files (CSS, JS, images)
├── docs/                # Documentation
│   ├── instructions/    # Developer instruction docs (#01-#09)
│   └── adr/             # Architecture Decision Records
├── reviews/             # QA review results
└── tests/               # Test scripts
```

## Development Workflow

See [docs/instructions/00-workflow.md](instructions/00-workflow.md) for the full PR-based workflow with QA review.
