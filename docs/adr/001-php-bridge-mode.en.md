# ADR-001: PHP Bridge Mode and Plugin Compatibility Strategy

- **Status**: Accepted
- **Date**: 2026-03-07
- **Related**: [GitHub Discussion #1 (Japanese)](https://github.com/rustpress-project/RustPress/discussions/1)

---

## Conclusion (TL;DR)

**PHP Bridge Mode is rejected. Instead of running PHP plugins as-is, we convert them to Rust using AI.**

Why this works: WordPress is 100% open source. The "correct answer" is fully readable. Since the correct answer exists, iterating through AI conversion and comparison testing against the original guarantees that 100% compatibility is not "impossible" — it's a matter of work volume.

```
WordPress (PHP) = The correct answer (source code IS the specification)
        | AI reads and converts
RustPress (Rust) = Rust implementation with identical behavior
        | Compare outputs
If there's a diff, fix it (the correct answer always exists, so it's always fixable)
```

---

## Context

RustPress's ultimate goal is to "establish a migration path for every WordPress site in the world" (MASTERPLAN Phase 11 completion criteria).

The WordPress ecosystem has 59,000+ PHP plugins that sites depend on. How to achieve plugin compatibility is the most critical architectural decision for the project.

Two approaches were considered:
1. **PHP Bridge Mode**: Run PHP plugins as-is (call PHP-FPM from RustPress)
2. **Full AI Conversion**: Convert PHP code to Rust using AI (no PHP runtime needed)

---

## PHP Bridge Mode — Analysis and Rejection

### Why it was considered

Converting 59,000 PHP plugins to Rust seemed like an enormous amount of work. If PHP plugins could run as-is, the migration barrier would be zero.

### Option A: PHP-FPM call per hook — Rejected

```
RustPress -> [100 hook firings x FastCGI round-trip] -> Response
```

A typical page fires 100-300 hooks. Each FastCGI round-trip costs ~0.5-1ms. Additional overhead of 50-300ms makes it **slower than WordPress's ~200ms**.

### Option B: One PHP-FPM call per request — Rejected

```
RustPress -> [1 FastCGI call: WordPress bootstrap + all hooks] -> Response
```

~1.3x faster than WordPress for dynamic pages. However, this architecture is essentially "a Rust reverse proxy in front of WordPress" — reducing RustPress's value to a "fast proxy."

Fatal problem: Users settle into PHP Bridge Mode ("it works, why change?") and never migrate to Rust plugins. Historical precedent: Wine (run Windows apps on Linux) hasn't led to more Linux-native apps in 20+ years.

### Option C: Execute PHP plugin hooks without WordPress bootstrap — Rejected

PHP plugin callbacks depend on the entire WordPress execution context:
- They call WP functions: `get_option()`, `get_post_meta()`, `is_singular()`
- They reference globals: `$post`, `$wp_query`, `$wpdb`
- Plugin classes are defined only after `require_once` loads the plugin file
- Modern plugins depend on Composer PSR-4 autoloading

Without WordPress bootstrap, plugins cannot execute -> falls back to Option B.

Bridging WP functions to the Rust side was also considered, but each WP function call from within a PHP callback would require an IPC round-trip, creating a "ping-pong hell" that's 5-7x slower than WordPress.

### Option D: Hook Connector (RustPress primary, WordPress as hook backend) — Rejected

```
Client -> RustPress (primary) -> Hook Connector -> WordPress (plugin execution engine)
```

Hook-level connectors fall back to the same problems as Options A/C. Route-level (URL-based) connectors are practical but are essentially reverse proxies — same as Option B.

### Fundamental problem with all PHP Bridge approaches

To run PHP plugins, you need the WordPress runtime. If you're running WordPress, RustPress is just a cache/proxy layer. Nginx + Varnish already does this. There's no reason to build a new CMS in Rust.

---

## The Turning Point: "The Correct Source Code Exists"

During the PHP Bridge discussion, we nearly concluded that "PHP plugins depend on the WordPress runtime, so they can't work." But then a fundamental challenge arose:

**"WordPress's source code — the correct answer — is right there. It's absurd to say we can't perfectly reproduce it."**

WordPress is 100% open source:
- WordPress Core: ~400,000 lines of PHP — every line is readable
- 2,000 WP functions: all open source — the code IS the specification
- 59,000 plugins: all open source — every line is readable

"PHP plugins depend on WordPress" is true, but **WordPress itself is open-source PHP code**. It can be read. It can be converted.

```
Wrong framing:   "Running PHP plugins in Rust is technically difficult"
Correct framing: "Converting WordPress PHP code to Rust is a large volume of work"
```

Not "difficult." Just "a lot of work." And we have AI.

---

## Decision: Full AI Conversion Strategy

### Approach

```
Step 1: Convert WordPress Core (wp-includes/) to Rust
  get_option()     -> Rust implementation <- PHP source code is the spec
  WP_Query         -> Rust implementation <- PHP source code is the spec
  wp_insert_post() -> Rust implementation <- PHP source code is the spec
  ... all 2,000 functions

Step 2: Reproduce global state in Rust
  $post     -> AppState::current_post
  $wp_query -> AppState::main_query
  $wpdb     -> SeaORM DatabaseConnection

Step 3: Convert PHP plugins/themes to Rust using AI
  Converted code calls Step 1's Rust WP functions
  1:1 correspondence between PHP and Rust APIs -> maximizes AI conversion accuracy

Step 4: Comparison testing against the correct answer
  Same DB + same request -> compare WordPress (PHP) output vs RustPress (Rust) output
  If there's a diff, fix it -> the correct answer always exists, so it's always fixable
```

### Why this qualifies as "the perfect answer"

| Condition | Status |
|-----------|--------|
| Is the spec clear? | Yes — WordPress PHP source code IS the spec |
| Can AI convert it? | Yes — LLMs understand both PHP and Rust |
| Can we verify? | Yes — compare outputs against WordPress |
| Can we fix diffs? | Yes — read the PHP code to identify the cause |
| Can we reach 100%? | Yes — iterating the above loop converges to 100% |

### The core philosophy

**RustPress is not made possible by Rust's speed or Axum's performance. It's made possible by the fact that AI can now scale the tedious, mechanical work of converting code when the correct answer (source code) is fully available.** This must never be forgotten.

### WP Function Compatibility Layer

The AI conversion target is "WP-compatible API" (not a RustPress-original API):

```
PHP plugin:  get_option('key')      -> AI converts -> Rust plugin: get_option("key")
PHP plugin:  add_filter('hook', fn) -> AI converts -> Rust plugin: add_filter("hook", fn)
```

1:1 API name correspondence:
- Maximizes AI conversion accuracy (mainly function name replacement)
- Converted code is easy to compare with original PHP
- WP function implementation spec = PHP source code (zero ambiguity)

---

## WordPress Compatibility Scope

| Layer | Policy | Rationale |
|-------|--------|-----------|
| **Database** (wp_posts, etc.) | **Maintain compat** | Connecting to existing WP DB = RustPress's reason to exist. Without it, we're just "another CMS written in Rust" |
| **REST API** (/wp-json/) | **Maintain compat** | Existing clients and mobile apps work unchanged |
| **URL structure** | **Maintain compat** | Preserves SEO rankings. Non-negotiable for migration |
| **WP functions** (2,000) | **Reimplement in Rust** | Target API for AI-converted plugins. PHP source is the spec |
| **Plugins** | **AI conversion to Rust** | Not running PHP — converting to Rust |
| **Themes** | **AI conversion to Tera** | Not running PHP — converting to Tera templates |

---

## Impact

### Positive
- PHP runtime completely eliminated -> single binary distribution, security improvement, 100x faster
- "Correct answer exists" enables quality assurance -> comparison testing against WordPress output
- AI conversion scales -> 59,000 plugins are theoretically convertible

### Risks
- Reimplementing 2,000 WP functions is substantial work -> but AI can accelerate, and the spec is unambiguous
- AI conversion accuracy determines project success -> comparison testing ensures quality
- Must track WordPress version updates -> detect WP function diffs and update accordingly

### Rejected alternatives
- PHP Bridge Mode (Options A/B/C/D) — all have fundamental problems
- RustPress-original API — WP-compatible API yields higher AI conversion accuracy
- Dropping DB compatibility — would eliminate RustPress's reason to exist
