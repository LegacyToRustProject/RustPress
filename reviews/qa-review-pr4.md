# QA #09 レビュー — RustPress feat/theme-tt20-tt19-gutenberg-blocks

- **レビュー日**: 2026-03-08
- **PR**: feat/theme-tt20-tt19-gutenberg-blocks → main
- **担当**: #02
- **レビュワー**: QA #09

---

## 判定: **CONDITIONAL APPROVAL（条件付き承認）**

**条件**:
1. **BLOCKER**: `session.rs` の重複 `regenerate_id` 関数を解消（E0592 → clippy FAIL）
2. **CRITICAL（継続）**: BUG-NEW-1（admin-ajax.php ルート重複）が未修正 — コンテナ起動不能

---

## チェックリスト

| 項目 | 結果 |
|------|------|
| CI: `RUSTFLAGS="-Dwarnings" cargo check --workspace` | ✅ PASS |
| CI: `cargo test --workspace --lib --bins` | ✅ PASS — 1069テスト |
| CI: `cargo clippy --workspace -- -D warnings` | ❌ **FAIL** — E0592 重複定義 |
| CI: `cargo fmt --all -- --check` | ✅ PASS |
| TT20 テーマ実装 | ✅ PASS (`themes/twentytwenty/`) |
| TT19 テーマ実装 | ✅ PASS (`themes/twentynineteen/`) |
| Gutenberg ブロックレンダラー: 15ブロック対応 | ✅ PASS |
| `core/buttons` → `wp-block-buttons` | ✅ PASS (`renderer.rs:97-98`) |
| 不明ブロック → inner_html パススルー | ✅ PASS |
| BUG-NEW-1（admin-ajax.php 重複）修正 | ❌ **未修正** |

---

## clippy エラーの詳細

```
error[E0592]: duplicate definitions with name `regenerate_id`
  --> crates/rustpress-auth/src/session.rs:140
  --> crates/rustpress-auth/src/session.rs:174
```

`session.rs` に `pub async fn regenerate_id()` が2箇所定義されている。

**修正方法**: いずれか一方を削除または統合してください。

---

## BUG-NEW-1 継続 (CRITICAL)

```
thread 'main' panicked at crates/rustpress-server/src/routes/mod.rs:70:10:
Overlapping method route. Handler for `POST /wp-admin/admin-ajax.php` already exists
```

- `frontend.rs:104`: `.route("/wp-admin/admin-ajax.php", get(admin_ajax).post(admin_ajax))`
- `wp_admin.rs:379`: `.route("/wp-admin/admin-ajax.php", post(admin_ajax_handler))`

**修正方法**: `frontend.rs:104` の行を削除してください。

---

## テーマ実装確認 ✅

| テーマ | 状態 |
|--------|------|
| TT25 (twentytwentyfive) | 既存 ✓ |
| TT24 (twentytwentyfour) | 既存 ✓ |
| TT23 (twentytwentythree) | 既存 ✓ |
| TT22 (twentytwentytwo) | 既存 ✓ |
| TT21 (twentytwentyone) | 既存 ✓ |
| **TT20 (twentytwenty)** | **新規 ✓** |
| **TT19 (twentynineteen)** | **新規 ✓** |

TT19/TT20 の追加により、7世代のテーマをカバー。

## Gutenberg ブロック実装 ✅

| ブロック | 変換 | 状態 |
|---------|------|------|
| core/paragraph | `<p>` | ✓ |
| core/heading | `<h1>`〜`<h6>` | ✓ |
| core/image | `<figure><img>` | ✓ |
| core/buttons | `<div class="wp-block-buttons">` | ✓ |
| 不明ブロック | inner_html パススルー | ✓ |
| 全15ブロック | — | ✓ |

---

## 良い点

- **1069テスト**: 前回比195件増加（TT20/TT19 + blocks テスト追加）
- **Gutenberg ブロックレンダラー**: 15ブロック対応で実用的なカバレッジ
- **テーマ世代拡張**: TT19〜TT25の完全シリーズ化

---

## アクション

| # | 優先度 | 内容 |
|---|--------|------|
| 1 | **BLOCKER** | `session.rs:174` の重複 `regenerate_id` を削除 |
| 2 | **CRITICAL** | `frontend.rs:104` の admin-ajax.php ルートを削除（BUG-NEW-1） |
| 3 | — | `cargo clippy -- -D warnings` で確認 |
| 4 | — | `git push` して再レビュー依頼 |

条件①②を満たした後 APPROVED とします。

---
*QA #09 — 2026-03-08*
