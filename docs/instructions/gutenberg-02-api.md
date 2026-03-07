# Gutenberg統合 — #02 API担当の作業

## 概要

WordPress Gutenberg エディタ（React）が RustPress の REST API を通じて正常に動作するよう、
足りないエンドポイントや応答形式の差異を修正する。

## 前提

- #01がGutenberg JSアセットの配信とテンプレート書き換えを担当
- 既存の REST API エンドポイントは大部分が実装済み
- Gutenbergが呼ぶ主要エンドポイントは全て存在するが、応答形式の調整が必要な可能性

## Gutenbergが呼ぶAPIエンドポイント一覧

### 既に実装済み（応答形式の確認が必要）

| エンドポイント | 用途 | 確認事項 |
|---|---|---|
| `/wp/v2/posts` | 投稿CRUD | `auto-draft` statusサポート |
| `/wp/v2/pages` | 固定ページCRUD | 同上 |
| `/wp/v2/categories` | カテゴリ | 応答形式OK |
| `/wp/v2/tags` | タグ | 応答形式OK |
| `/wp/v2/media` | メディア | 応答形式OK |
| `/wp/v2/comments` | コメント | 応答形式OK |
| `/wp/v2/users` | ユーザー | `me` エンドポイントの確認 |
| `/wp/v2/types` | 投稿タイプ | Gutenberg初期化に必須 |
| `/wp/v2/taxonomies` | タクソノミー | Gutenberg初期化に必須 |
| `/wp/v2/statuses` | 投稿ステータス | Gutenberg初期化に必須 |
| `/wp/v2/themes` | テーマ情報 | `theme_supports` が必要 |
| `/wp/v2/settings` | サイト設定 | Gutenberg初期化に必須 |
| `/wp/v2/block-types` | ブロックタイプ一覧 | 登録済みブロックの定義 |
| `/wp/v2/block-patterns` | ブロックパターン | 空配列でも可 |
| `/wp/v2/search` | 検索 | リンク挿入UIで使用 |
| `/wp/v2/templates` | テンプレート | 空配列でも可 |

## 作業手順

### Step 1: auto-draft ステータスのサポート

Gutenbergは新規投稿時に `status: "auto-draft"` でPOSTしてから編集を開始する。

**確認:**
```bash
curl -X POST http://localhost:8080/wp-json/wp/v2/posts \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"title":"","content":"","status":"auto-draft"}'
```

期待: 201 Created + 投稿オブジェクト返却

**もし 400 が返る場合:**
`crates/rustpress-api/src/posts.rs` で `auto-draft` を有効な status として受け付けるよう修正。

### Step 2: /wp/v2/types の応答確認

Gutenbergは起動時に `/wp/v2/types` を呼んで投稿タイプの情報を取得する。

**必須フィールド:**
```json
{
  "post": {
    "name": "post",
    "slug": "post",
    "rest_base": "posts",
    "rest_namespace": "wp/v2",
    "labels": {
      "name": "Posts",
      "singular_name": "Post",
      "add_new": "Add New Post"
    },
    "supports": {
      "title": true,
      "editor": true,
      "author": true,
      "thumbnail": true,
      "excerpt": true,
      "comments": true,
      "revisions": true,
      "custom-fields": true
    },
    "taxonomies": ["category", "post_tag"],
    "viewable": true
  }
}
```

### Step 3: /wp/v2/themes の応答確認

Gutenbergはテーマのブロックサポート情報を取得する。

**必須フィールド:**
```json
[{
  "stylesheet": "twentytwentyfive",
  "theme_supports": {
    "align-wide": true,
    "responsive-embeds": true,
    "editor-styles": true,
    "wp-block-styles": true,
    "editor-color-palette": [],
    "editor-font-sizes": []
  }
}]
```

### Step 4: /wp/v2/users/me の確認

Gutenbergはログインユーザー情報を `/wp/v2/users/me` で取得する。

**確認:**
```bash
curl http://localhost:8080/wp-json/wp/v2/users/me \
  -H "Authorization: Bearer <token>"
```

期待: 200 + ユーザーオブジェクト（id, name, slug, roles, capabilities）

### Step 5: Gutenberg応答形式テスト

#01がGutenbergの配信を完了した後、ブラウザのDevToolsでコンソールエラーとNetworkタブを確認:

1. ブラウザで `/wp-admin/post-new.php` を開く
2. DevTools → Network タブを開く
3. 赤くなっている（失敗している）APIリクエストを特定
4. WordPress (`:8081`) の同じエンドポイントと応答を比較
5. 差異があれば修正

### Step 6: nonce / Cookie認証の対応

Gutenbergの `api-fetch` は `X-WP-Nonce` ヘッダーでCSRF保護する。
RustPressのREST APIがこのヘッダーを受け付けるか確認:

- 現在JWTで認証しているが、Gutenbergはセッションベース（Cookie + nonce）
- `api-fetch` の `createNonceMiddleware` が `X-WP-Nonce` ヘッダーを付与
- RustPressのミドルウェアがこのヘッダーを検証する必要がある

**方針:**
- REST APIでCookie認証を受け付ける（セッションCookieがあればJWTなしでOK）
- `X-WP-Nonce` ヘッダーの検証を追加

## 完了条件

- [ ] `auto-draft` ステータスで投稿作成が可能
- [ ] `/wp/v2/types` が Gutenberg互換の応答を返す
- [ ] `/wp/v2/themes` が `theme_supports` を含む応答を返す
- [ ] `/wp/v2/users/me` が正常動作
- [ ] Cookie + X-WP-Nonce による REST API認証が動作
- [ ] Gutenbergエディタから投稿の作成・編集・保存が可能

## 注意事項

- #01の作業と並行して進められる（Step 1-4は独立して作業可能）
- Step 5-6 は#01のGutenberg配信完了後に実施
- APIの応答比較は WordPress (`:8081`) を参照基準にすること
- `cargo check --workspace` / `cargo test` が通ることを確認してからPR
