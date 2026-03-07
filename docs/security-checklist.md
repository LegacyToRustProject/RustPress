# RustPress Security Checklist (OWASP Top 10)

Status: **Implemented** | Last updated: 2026-03-07

## OWASP A01: Broken Access Control

- [x] RBAC with 5 roles (administrator, editor, author, contributor, subscriber) and 73 capabilities
- [x] All admin routes require `require_admin_session` + capability-based middleware
- [x] Plugin admin routes require `require_admin` (manage_options capability)
- [x] JWT tokens include actual user role from DB (never hardcoded)
- [x] Default role for unknown users: "subscriber" (least privilege)
- [x] User API endpoints hide sensitive fields (email, login) from public responses
- [x] CORS middleware restricts cross-origin requests to configured site URL
- [x] Sensitive file blocking middleware (`.env`, `.git`, `wp-config.php`, etc.)
- [x] Directory listing disabled (no static file listing)

## OWASP A02: Cryptographic Failures

- [x] New passwords hashed with Argon2id (default parameters)
- [x] Legacy WordPress passwords verified via bcrypt and PHPass (with Argon2 rehash flag)
- [x] JWT secret sourced from `JWT_SECRET` env var (256+ bit random fallback)
- [x] JWT secret minimum length enforced (32 bytes / 256 bits, panics if shorter)
- [x] Warning emitted when using fallback JWT secret
- [x] PHPass hash comparison uses constant-time comparison (timing attack prevention)
- [x] Session cookies use HttpOnly + SameSite=Lax + Secure (when HTTPS)
- [x] Session cookies have Max-Age (24h expiry)
- [x] No passwords logged in audit entries or error messages
- [x] Internal errors return generic messages, never DB details or stack traces

## OWASP A03: Injection

- [x] SeaORM parameter binding for all DB queries (no raw SQL with user input in web server)
- [x] WAF rules block SQL injection patterns (UNION SELECT, OR 1=1, DROP TABLE, comment bypass, etc.)
- [x] WAF rules block XSS patterns (script tags, event handlers, javascript: URIs)
- [x] WAF rules block command injection (shell metacharacters, pipe/backtick)
- [x] WAF rules block directory traversal (../ patterns)
- [x] WAF inspects URL path, query string, request body, and headers
- [x] Tera templates use auto-escaping by default
- [x] `escape_html()` used for all dynamic content in manually-built HTML strings
- [x] Comment author_url restricted to http/https schemes (javascript: XSS prevention)
- [x] 86 OWASP-specific security tests covering injection vectors

## OWASP A04: Insecure Design

- [x] Login returns identical error messages for invalid user and wrong password (anti-enumeration)
- [x] API login uses same error message pattern
- [x] Password strength validation enforced (PasswordPolicy: 8+ chars, 3 character classes)
- [x] Common passwords rejected (dictionary check)
- [x] Session IDs generated with UUID v4 (cryptographically random)
- [x] File upload MIME type validation — `infer` crate checks magic bytes, rejects spoofed Content-Type
- [x] Password reset token expiry enforcement — 24h TTL stored in wp_usermeta, checked in `validate_reset_token()`

## OWASP A05: Security Misconfiguration

- [x] Security scanner detects debug mode, default admin, weak DB prefix
- [x] Sensitive file blocking middleware (blocks .env, .git, backup files, PHP files, logs)
- [x] Error messages never expose DB connection strings or stack traces
- [x] Warnings emitted for insecure default configurations (DATABASE_URL, JWT_SECRET)
- [x] No hardcoded admin password — generated randomly or from ADMIN_PASSWORD env var
- [x] Security headers applied to all responses (X-Content-Type-Options, X-Frame-Options, etc.)
- [x] Permissions-Policy restricts camera, microphone, geolocation
- [x] `.php` file requests return 404 (except allowed WordPress-compatible routes)

## OWASP A06: Vulnerable and Outdated Components

- [x] `cargo audit` executed — known issues documented:
  - `rsa` v0.9.10: Timing side-channel (medium, via sqlx-mysql dependency — no fix available)
  - `wasmtime` v29.0.1: 4 advisories (resource exhaustion, panic, segfault) — upgrade planned
  - `fxhash` / `paste`: Unmaintained warnings (non-security)
- [x] Cargo.lock pins all dependency versions
- [ ] Dependabot/Renovate configuration (CI team responsibility)

## OWASP A07: Identification and Authentication Failures

- [x] Rate limiting: Login=60/min, API=300/min, General=600/min per IP
- [x] Brute force auto-lockout via LoginProtection
- [x] JWT tokens have 24h expiry with server-side validation
- [x] Session cookies: HttpOnly, SameSite=Lax, Secure (HTTPS), Max-Age=86400
- [x] Password strength policy: 8+ chars, 3 of 4 character classes, common password rejection
- [x] WordPress legacy hash verification (PHPass, bcrypt, MD5) with Argon2 rehash
- [x] CSRF nonce system (WordPress-compatible wp_create_nonce/wp_verify_nonce)
- [x] CSRF nonce check middleware for admin POST/PUT/DELETE endpoints
- [x] TOTP 2FA — RFC 6238 HMAC-SHA1, 30-second window, ±1 drift tolerance; enrollment at `/wp-admin/profile-2fa`; login redirects to `/wp-login.php?action=2fa` when `_totp_secret` set; intermediate state uses 5-minute "2FA pending" JWT (role=`2fa_pending`); code entry required before session is created
- [x] JWT token blacklist on logout — `jti` claim added to all tokens; logout via `POST /api/auth/logout` blacklists `jti` in Moka cache (24h TTL); `validate_token()` rejects blacklisted tokens

## OWASP A08: Software and Data Integrity Failures

- [x] CSRF protection via nonce system for browser-based state changes
- [x] API endpoints use JWT Bearer tokens (immune to CSRF)
- [x] WASM plugins run in sandboxed environment
- [ ] Plugin/theme signature verification (planned)

## OWASP A09: Security Logging and Monitoring

- [x] Audit log system (`AuditLog`) records security events in memory ring buffer
- [x] Events logged: login success/failure, WAF blocks, rate limiting, brute force detection
- [x] All audit entries include timestamp, IP address (extracted from X-Forwarded-For/X-Real-IP), user ID, event type
- [x] Audit events emitted via `tracing` for external log collection (stdout, file, ELK)
- [x] Severity levels: Info, Warning, Critical
- [x] No sensitive data (passwords, tokens) in log entries
- [x] Convenience methods for common event types

## OWASP A10: Server-Side Request Forgery (SSRF)

- [x] SSRF protection module (`ssrf.rs`) validates outbound URLs
- [x] Blocks private IP ranges (RFC 1918, link-local, loopback, shared address space)
- [x] Blocks AWS metadata endpoint (169.254.169.254)
- [x] Blocks non-HTTP schemes (ftp, file, gopher, etc.)
- [x] Blocks internal hostnames (.local, .internal, localhost)
- [x] IPv6 private addresses blocked (loopback, unique local, link-local)
- [x] IPv4-mapped IPv6 addresses checked

## WordPress-Specific Attack Vectors

| Attack | RustPress Defense |
|--------|------------------|
| Plugin vulnerabilities (97% of WP CVEs) | WASM sandbox + Rust type safety |
| xmlrpc.php abuse | Rate limited, WAF-protected |
| wp-login.php brute force | Rate limiting (60/min) + auto-lockout |
| REST API user enumeration | Auth required for user listing, no email/login exposed |
| File upload RCE | Rust binary — uploaded files cannot execute |
| wp-config.php leak | Blocked by middleware, config via env vars |
| Directory traversal | WAF rules + path validation |

## Test Coverage

- **Total workspace tests**: 1098 (all passing, +200 added in beta-sprint-03)
- **Security crate tests**: 116 (OWASP-focused, +29)
- **Auth crate tests**: 154 (including TOTP, JWT pending token, blacklist, password policy, roles, +89)
- **Migrate crate tests**: 70 (analyze + rewrites, +59)
- **All tests pass with `cargo test --workspace --lib --bins`**
- **`cargo clippy --workspace -- -D warnings` passes clean**
