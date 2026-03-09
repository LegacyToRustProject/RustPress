# Sprint #06 QA Results — Twenty Seventeen Theme

**Date**: 2026-03-09
**Branch**: feat/theme-classic-tt17
**Test type**: Visual pixel comparison (Selenium + thirtyfour)
**Threshold**: ≥93% pixel match per page

---

## Twenty Seventeen (TT17) — Classic Theme

### Results

| Page | Score | Status | Diff Pixels |
|------|-------|--------|-------------|
| home | 98.91% | PASS | 19,615 |
| single_post | 96.72% | PASS | 59,178 |
| sample_page | 93.47% | PASS | 118,022 |
| search | 100.00% | PASS | 0 |
| 404 | 99.92% | PASS | 1,361 |
| category | 98.04% | PASS | 35,384 |
| author | 99.94% | PASS | 1,083 |

**All 7 pages PASS** | Average: **98.14%**

---

## Issues Found & Fixed

### Bug: WAF false positive on image requests (sqli-003)

**Root cause**: The WAF rule `sqli-003` ("SQL Injection - Comment injection") matched Chrome's `Accept: image/webp,image/apng,image/*,*/*;q=0.8` header. The pattern `\/\*.*\*\/` (designed to catch SQL `/* comment */` injection) matched the glob wildcards in the media type string `image/*,*/*` as a false positive, causing all image requests from Chrome to return HTTP 403.

**Fix** (`crates/rustpress-server/src/middleware.rs`): Added `/wp-content/themes/` and `/wp-includes/` to the WAF bypass list for static assets:
```rust
if path.starts_with("/static/")
    || path.starts_with("/wp-content/uploads/")
    || path.starts_with("/wp-content/themes/")   // NEW
    || path.starts_with("/wp-includes/")          // NEW
    || path == "/favicon.ico"
{
    return next.run(request).await;
}
```

This is correct security behavior — theme static files (CSS, JS, images, fonts) are trusted assets compiled into the Docker image and don't require WAF inspection.

---

## Theme Implementation

**Templates created** (`themes/twentyseventeen/templates/`):
- `base.html` — Full HTML layout with TT17 custom-header structure
- `header.html` — Custom header media with parallax image support
- `footer.html` — Site info footer
- `home.html` — Blog listing with featured images
- `single.html` — Single post with post thumbnail
- `page.html` — Static pages
- `404.html` — Error page with search form
- `search.html` — Search results
- `category.html` — Category archive
- `author.html` — Author archive

**Theme assets** (`themes/twentyseventeen/static/`):
- `style.css`, `assets/css/blocks.css` — Theme stylesheets
- `assets/fonts/font-libre-franklin.css` + font files — Typography
- `assets/images/header.jpg` — Default header image (plant photo)

**Also fixed**: `Dockerfile` was missing `COPY --from=builder /build/themes /app/themes`, so theme files were never included in the Docker image.

---

## Compatibility Matrix Update

`docs/theme-compat-matrix.md` updated:
- Twenty Seventeen: `Untested` → `OK (≥93% all 7 pages, Sprint #06)`
