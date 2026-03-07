# WordPress Theme Compatibility Matrix

Status: **In Progress** | Last updated: 2026-03-07

## Legend

- **OK**: Renders correctly | **Partial**: Minor visual issues | **Broken**: Major issues
- **Untested**: Not yet tested | **N/A**: Requires page builder plugin
- **Block**: FSE theme | **Classic**: PHP templates | **Hybrid**: Classic + theme.json

---

## Top 100 Themes

### Tier 1: Default WordPress Themes (1-10)

| # | Theme | Type | Status |
|---|-------|------|--------|
| 1 | Twenty Twenty-Five | Block | Partial (98%) |
| 2 | Twenty Twenty-Four | Block | Untested |
| 3 | Twenty Twenty-Three | Block | Untested |
| 4 | Twenty Twenty-Two | Block | Untested |
| 5 | Twenty Twenty-One | Classic | Untested |
| 6 | Twenty Twenty | Classic | Untested |
| 7 | Twenty Nineteen | Classic | Untested |
| 8 | Twenty Seventeen | Classic | Untested |
| 9 | Twenty Sixteen | Classic | Untested |
| 10 | Twenty Fifteen | Classic | Untested |

### Tier 2: Most Popular Third-Party (11-40)

| # | Theme | Type | Status |
|---|-------|------|--------|
| 11 | Astra | Hybrid | Untested |
| 12 | Hello Elementor | Hybrid | N/A |
| 13 | OceanWP | Classic | Untested |
| 14 | Neve | Hybrid | Untested |
| 15 | Kadence | Hybrid | Untested |
| 16 | GeneratePress | Classic | Untested |
| 17 | Storefront | Classic | Untested |
| 18 | Sydney | Classic | Untested |
| 19 | Hestia | Classic | Untested |
| 20 | Blocksy | Hybrid | Untested |
| 21 | Divi | Classic | Untested |
| 22 | Avada | Classic | Untested |
| 23 | Rown | Block | Untested |
| 24 | Ollie | Block | Untested |
| 25 | Skate | Block | Untested |
| 26 | Solar | Classic | Untested |
| 27 | BlockBase | Block | Untested |
| 28 | Zakra | Block | Untested |
| 29 | PopularFX | Classic | Untested |
| 30 | Spectra | Classic | Untested |
| 31 | Prime | Classic | Untested |
| 32 | Exmage | Block | Untested |
| 33 | GoTheme | Block | Untested |
| 34 | Appat | Block | Untested |
| 35 | Twentig | Block | Untested |
| 36 | Amelia | Block | Untested |
| 37 | Armada | Block | Untested |
| 38 | Inspiro | Classic | Untested |
| 39 | Cosmos | Classic | Untested |
| 40 | Zerif | Block | Untested |

### Tier 3: Popular Niche Themes (41-70)

| # | Theme | Type | Status |
|---|-------|------|--------|
| 41 | Mesmerized | Classic | Untested |
| 42 | Ninja | Classic | Untested |
| 43 | Luxeritas | Classic | Untested |
| 44 | Impreza | Classic | Untested |
| 45 | Point | Block | Untested |
| 46 | OnePress | Classic | Untested |
| 47 | Salient | Classic | Untested |
| 48 | TheGem | Classic | Untested |
| 49 | Uncode | Classic | Untested |
| 50 | Newspaper | Classic | Untested |
| 51-70 | (remaining themes) | Various | Untested |

### Tier 4: Long Tail (71-100)

| 71-100 | (remaining themes) | Various | Untested |

---

## Feature Requirements Summary

### Implemented
- theme.json parsing (CSS variables, font-face, body/element/block styles, borders, outlines, shadows)
- Template hierarchy (single, page, archive, category, tag, author, search, 404, home, front-page, attachment)
- Template tags: the_title, the_content, the_excerpt, the_permalink, the_date, the_id
- wpautop, wptexturize, block layout classes
- wp_nav_menu() with DB-backed menus and location resolution
- Widget system (10 types)
- Theme switching mechanism (DB + env var)
- Page cache (moka, 5min TTL)
- body_class() / post_class() generation
- Featured image support (bulk loading for archives)
- get_search_form() HTML generation
- wp_enqueue_style() / wp_enqueue_script() (AssetManager)
- wp_link_pages() for multi-page posts
- i18n functions (__, _n)
- Shortcode processing (caption, audio, video, gallery, embed)

### Needed for Block Themes (Tier 1 priority)
- [ ] Style variations (theme.json alternates)
- [ ] Block patterns
- [ ] Navigation block (wp:navigation)
- [ ] Template parts as blocks
- [ ] wp-container-core-* CSS generation

### Needed for Classic Themes (Tier 2 priority)
- [ ] comments_template() Tera function
- [ ] the_post_thumbnail() with registered sizes
- [ ] Custom header/logo support
- [ ] Customizer API (partial)
