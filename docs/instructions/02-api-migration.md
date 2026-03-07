# 指示書 #02: RustPress API・移行・CI担当

## あなたの役割

あなたはRustPressプロジェクトの**API・移行・CI担当AI開発者**です。
あなたのミッションは以下の3つのBeta基準を達成することです:

- **B-3**: WP REST API v2が100%互換
- **B-5**: `rustpress migrate` コマンド一発で5分以内にWPサイトが稼働
- **B-7**: CI/CDが完全稼働

他の担当者がテーマ互換性、セキュリティなどを並行で開発しています。あなたはAPI・移行ツール・CIに集中してください。

---

## プロジェクト概要

RustPressはWordPress互換のCMSで、Rustで書かれています。WordPressの既存MySQLデータベースにそのまま接続し、同じコンテンツを表示します。

- リポジトリ: https://github.com/LegacyToRustProject/RustPress
- 言語: Rust 1.88+
- フレームワーク: Axum + Tokio + SeaORM + Tera
- DB: MySQL 8.0（WordPressと同じDB）

---

## リポジトリ構成（API・移行・CI関連）

```
RustPress/
├── crates/
│   ├── rustpress-api/           # WP REST API v2 実装（あなたの主な作業場所）
│   │   └── src/
│   │       └── lib.rs           # APIエンドポイント定義
│   ├── rustpress-server/        # Webサーバー
│   │   └── src/
│   │       ├── main.rs          # エントリーポイント、ルーター構築
│   │       ├── middleware.rs    # ミドルウェア（認証、CORS等）
│   │       └── routes/
│   │           ├── mod.rs       # ルート定義一覧
│   │           ├── posts.rs     # /wp-json/wp/v2/posts
│   │           ├── users.rs     # /wp-json/wp/v2/users
│   │           ├── auth.rs      # 認証エンドポイント
│   │           ├── health.rs    # ヘルスチェック
│   │           ├── commerce.rs  # WooCommerce互換
│   │           ├── seo.rs       # SEOエンドポイント
│   │           ├── forms.rs     # フォームエンドポイント
│   │           └── xmlrpc.rs    # XML-RPC互換
│   ├── rustpress-auth/          # 認証（JWT, セッション, RBAC）
│   ├── rustpress-db/            # DBエンティティ（wp_posts, wp_options等）
│   ├── rustpress-query/         # WP_Query互換クエリビルダー
│   ├── rustpress-migrate/       # マイグレーションツール（あなたの作業場所）
│   ├── rustpress-cli/           # CLIツール（あなたの作業場所）
│   └── rustpress-e2e/           # E2Eテスト
├── .github/
│   └── workflows/
│       └── ci.yml               # GitHub Actions（あなたの作業場所）
└── docker-compose.yml           # テスト環境
```

---

## 現在の状態

### WP REST API（B-3関連）

#### 動いているもの
- `GET /wp-json/wp/v2/posts` — 投稿一覧
- `GET/POST /wp-json/wp/v2/posts/{id}` — 投稿取得・作成
- `GET /wp-json/wp/v2/pages` — 固定ページ一覧
- `GET /wp-json/wp/v2/categories` — カテゴリ一覧
- `GET /wp-json/wp/v2/tags` — タグ一覧
- `GET /wp-json/wp/v2/users` — ユーザー一覧
- `GET /wp-json/wp/v2/media` — メディア一覧
- `GET /wp-json/wp/v2/comments` — コメント一覧
- `GET /wp-json/wp/v2/search` — 検索
- `GET /wp-json/wp/v2/settings` — 設定
- `GET /wp-json/wp/v2/types` — 投稿タイプ
- `GET /wp-json/wp/v2/taxonomies` — タクソノミー
- `GET /wp-json/` — APIディスカバリー
- JWT認証（Bearerトークン）

#### 不完全・未実装
- PUT/DELETE の多くのエンドポイント
- クエリパラメータの完全対応（`_fields`, `_embed`, `per_page`, `page`, `search`, `after`, `before`, `author`, `categories`, `tags`, `status`, `orderby`, `order` 等）
- `_embed` によるリレーション展開（author, featured_media, wp:term 等）
- レスポンスヘッダー（`X-WP-Total`, `X-WP-TotalPages`, `Link`）
- エラーレスポンスの形式（`code`, `message`, `data.status` の構造）
- バッチリクエスト（`/wp-json/batch/v1`）
- カスタムエンドポイント登録機構
- `/wp-json/wp/v2/block-types`
- `/wp-json/wp/v2/blocks`
- `/wp-json/wp/v2/block-patterns`
- `/wp-json/wp/v2/navigation`
- `/wp-json/wp/v2/templates`
- `/wp-json/wp/v2/global-styles`
- XML-RPCの完全互換

### 移行ツール（B-5関連）

#### 動いているもの
- `SKIP_MIGRATIONS=true` + `DATABASE_URL` で既存WP DBに接続可能
- DBスキーマはWordPressと互換（そのまま読み取り）

#### 未実装
- `rustpress migrate` CLIコマンドが存在しない
- テーマの自動検出・変換
- プラグインデータの互換性チェック
- 移行前の互換性レポート
- SEO影響の検証
- メディアファイルのコピー/リンク

### CI/CD（B-7関連）

#### 現状
- `.github/workflows/ci.yml` は作成済みだがリモートにプッシュされていない
- 理由: GitHub PATに `workflow` スコープがない
- ci.yml の内容: check, test, fmt, clippy, build の5ジョブ

---

## ゴール: B-3（WP REST API v2 100%互換）

### WordPress REST API の完全仕様

WordPress REST APIは以下のエンドポイントで構成される。全て実装する。

#### コアエンドポイント一覧

```
# 投稿関連
GET    /wp-json/wp/v2/posts                # 一覧取得
POST   /wp-json/wp/v2/posts                # 新規作成
GET    /wp-json/wp/v2/posts/{id}           # 個別取得
PUT    /wp-json/wp/v2/posts/{id}           # 更新
PATCH  /wp-json/wp/v2/posts/{id}           # 部分更新
DELETE /wp-json/wp/v2/posts/{id}           # 削除
GET    /wp-json/wp/v2/posts/{id}/revisions # リビジョン一覧

# 固定ページ
GET/POST       /wp-json/wp/v2/pages
GET/PUT/DELETE /wp-json/wp/v2/pages/{id}

# メディア
GET/POST       /wp-json/wp/v2/media
GET/PUT/DELETE /wp-json/wp/v2/media/{id}

# カテゴリ・タグ
GET/POST       /wp-json/wp/v2/categories
GET/PUT/DELETE /wp-json/wp/v2/categories/{id}
GET/POST       /wp-json/wp/v2/tags
GET/PUT/DELETE /wp-json/wp/v2/tags/{id}

# コメント
GET/POST       /wp-json/wp/v2/comments
GET/PUT/DELETE /wp-json/wp/v2/comments/{id}

# ユーザー
GET/POST       /wp-json/wp/v2/users
GET/PUT/DELETE /wp-json/wp/v2/users/{id}
GET            /wp-json/wp/v2/users/me

# タクソノミー・投稿タイプ
GET            /wp-json/wp/v2/taxonomies
GET            /wp-json/wp/v2/taxonomies/{slug}
GET            /wp-json/wp/v2/types
GET            /wp-json/wp/v2/types/{slug}

# 検索
GET            /wp-json/wp/v2/search

# 設定
GET/PUT        /wp-json/wp/v2/settings

# メニュー（WP 5.9+）
GET            /wp-json/wp/v2/menus
GET            /wp-json/wp/v2/menus/{id}
GET            /wp-json/wp/v2/menu-items
GET            /wp-json/wp/v2/menu-locations

# ブロック関連（WP 5.8+）
GET            /wp-json/wp/v2/blocks
GET            /wp-json/wp/v2/block-types
GET            /wp-json/wp/v2/block-patterns
GET            /wp-json/wp/v2/block-patterns/categories

# テンプレート（FSE、WP 5.9+）
GET            /wp-json/wp/v2/templates
GET            /wp-json/wp/v2/template-parts

# グローバルスタイル（WP 6.0+）
GET            /wp-json/wp/v2/global-styles

# ナビゲーション（WP 5.9+）
GET/POST       /wp-json/wp/v2/navigation
GET/PUT/DELETE /wp-json/wp/v2/navigation/{id}

# プラグイン・テーマ
GET            /wp-json/wp/v2/plugins
GET            /wp-json/wp/v2/themes

# ステータス
GET            /wp-json/wp/v2/statuses

# バッチ
POST           /wp-json/batch/v1

# ディスカバリー
GET            /wp-json/                    # APIルート一覧
HEAD           /                            # Link ヘッダーでAPI URLを返す
```

#### 共通クエリパラメータ（全コレクションエンドポイント）

```
?page=1              # ページ番号（デフォルト1）
?per_page=10         # 1ページあたりの件数（デフォルト10、最大100）
?search=keyword      # 全文検索
?after=ISO8601       # この日時以降
?before=ISO8601      # この日時以前
?order=asc|desc      # 並び順
?orderby=date|title|id|slug|relevance  # ソートキー
?_fields=id,title    # 返すフィールドを限定
?_embed              # リレーションを展開して含める
?_envelope           # レスポンスをエンベロープでラップ
```

#### 共通レスポンスヘッダー

```
X-WP-Total: 42           # 全件数
X-WP-TotalPages: 5       # 全ページ数
Link: <...?page=2>; rel="next", <...?page=5>; rel="last"
Allow: GET, POST          # OPTIONSレスポンスで許可メソッド
```

#### エラーレスポンス形式

```json
{
  "code": "rest_post_invalid_id",
  "message": "Invalid post ID.",
  "data": {
    "status": 404
  }
}
```

WordPress公式のエラーコード一覧:
- `rest_no_route` — ルートが存在しない
- `rest_forbidden` — 権限がない
- `rest_post_invalid_id` — 無効な投稿ID
- `rest_cannot_create` — 作成権限がない
- `rest_invalid_param` — 無効なパラメータ
- `rest_missing_callback_param` — 必須パラメータが欠落

#### `_embed` の実装

`?_embed` が指定された場合、レスポンスの `_embedded` フィールドにリレーションデータを含める:

```json
{
  "id": 1,
  "title": {"rendered": "Hello world!"},
  "_embedded": {
    "author": [{"id": 1, "name": "admin", ...}],
    "wp:featuredmedia": [{"id": 5, "source_url": "...", ...}],
    "wp:term": [
      [{"id": 1, "name": "Uncategorized", ...}],
      [{"id": 3, "name": "news", ...}]
    ]
  }
}
```

### 検証方法

1. **WordPress公式テストスイートとの比較**
   - Docker環境でWordPressを起動
   - 同じリクエストをWordPressとRustPressの両方に送信
   - レスポンスのJSON構造を比較
   - 差分をレポート

2. **自動テストの作成**
   ```bash
   # 各エンドポイントのテスト例
   # WordPress
   curl -s http://localhost:8081/wp-json/wp/v2/posts?per_page=5 | jq .
   # RustPress
   curl -s http://localhost:8080/wp-json/wp/v2/posts?per_page=5 | jq .
   # 両者のレスポンス構造が一致することを確認
   ```

3. **互換性テストクレートの作成**
   `crates/rustpress-e2e/` にAPIレスポンス比較テストを追加する。全エンドポイント × 主要パラメータの組み合わせ。

### 作業手順

1. 既存のエンドポイントに不足しているクエリパラメータ（`_embed`, `_fields`, `per_page`等）を追加
2. 不足しているCRUD操作（PUT, DELETE）を追加
3. レスポンスヘッダー（`X-WP-Total`, `X-WP-TotalPages`, `Link`）を追加
4. エラーレスポンス形式をWordPress準拠に統一
5. 未実装エンドポイント（menus, blocks, templates等）を追加
6. `_embed` を実装
7. バッチリクエストを実装
8. 全エンドポイントの自動テストを作成

---

## ゴール: B-5（`rustpress migrate` コマンド）

### ユーザー体験の目標

```bash
# ユーザーがやること（これだけ）:
rustpress migrate --source mysql://user:pass@old-server:3306/wordpress

# 出力:
[1/6] Analyzing WordPress database...
      WordPress 6.7, 342 posts, 12 pages, 1,847 comments
      Theme: flavor (classic)
      Plugins: contact-form-7, yoast-seo, woocommerce

[2/6] Connecting to database...
      OK — using existing WordPress tables directly

[3/6] Checking theme compatibility...
      flavor: Classic theme → Tera conversion needed
      Converting templates... done (14 templates)

[4/6] Checking plugin compatibility...
      contact-form-7: ✅ Rust-native equivalent (rustpress-forms)
      yoast-seo:      ✅ Rust-native equivalent (rustpress-seo)
      woocommerce:    ✅ Rust-native equivalent (rustpress-commerce)

[5/6] Verifying SEO compatibility...
      Checking permalinks... ✅ all 342 URLs preserved
      Checking meta tags... ✅ title, description, og:tags preserved

[6/6] Starting RustPress...
      Server running at http://localhost:8080
      Migration complete in 47 seconds.
```

### 実装場所

- CLI: `crates/rustpress-cli/`
- 移行ロジック: `crates/rustpress-migrate/`

### 必要なサブコマンド

```bash
# 移行（メイン機能）
rustpress migrate --source <DATABASE_URL>

# 分析のみ（移行前チェック）
rustpress migrate analyze --source <DATABASE_URL>

# テーマ変換のみ
rustpress migrate theme --source <DATABASE_URL>

# プラグイン互換性チェック
rustpress migrate plugins --source <DATABASE_URL>

# SEO影響チェック
rustpress migrate seo-audit --source <DATABASE_URL>
```

### 実装手順

1. **CLIフレームワーク構築**: `clap` クレートでサブコマンド定義
2. **DB分析**: 接続 → WordPress版情報取得 → 投稿数・テーマ・プラグイン検出
3. **テーマ互換チェック**: アクティブテーマがRustPress対応済みか判定
4. **プラグイン互換チェック**: 有効プラグインリスト → Rust-native代替の有無を表示
5. **SEO監査**: パーマリンク構造の保持、メタタグの保持を検証
6. **サーバー起動**: 全チェック通過後にRustPressを起動

### 重要な設計原則

- **既存DBを変更しない**: RustPressは読み取り専用でWordPressのDBを使う。テーブル追加・変更は絶対にしない（`SKIP_MIGRATIONS=true`モード）
- **WordPressと共存可能**: 移行中もWordPressは動き続ける。RustPressは別ポートで起動し、切り替えはDNS/リバースプロキシで行う
- **ロールバック可能**: いつでもWordPressに戻せる

---

## ゴール: B-7（CI/CD完全稼働）

### 現状

`.github/workflows/ci.yml` が存在するがリモートにプッシュされていない。

```yaml
# 現在の ci.yml の内容:
name: CI
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  check:    cargo check --workspace
  test:     cargo test --workspace --lib --bins -- --skip e2e
  fmt:      cargo fmt --all -- --check
  clippy:   cargo clippy --workspace -- -D warnings
  build:    cargo build --release --workspace
```

### ブロッカー

GitHub PATに `workflow` スコープがない。ユーザー（プロジェクトオーナー）がGitHub Settings > Personal Access Tokens で `workflow` スコープを追加する必要がある。

**あなたがやること:**
1. ユーザーに `workflow` スコープの追加を依頼する
2. スコープが追加されたら `.github/workflows/ci.yml` をプッシュする
3. CIが動作することを確認する

### 追加すべきCIジョブ

現在の5ジョブに加えて:

```yaml
# セキュリティ監査
audit:
  name: Security Audit
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - run: cargo install cargo-audit
    - run: cargo audit

# 依存関係ライセンスチェック
license:
  name: License Check
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - run: cargo install cargo-deny
    - run: cargo deny check licenses

# リリースバイナリのビルド（タグプッシュ時）
release:
  name: Release Build
  if: startsWith(github.ref, 'refs/tags/v')
  strategy:
    matrix:
      os: [ubuntu-latest, macos-latest, windows-latest]
  runs-on: ${{ matrix.os }}
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
    - run: cargo build --release
    - uses: softprops/action-gh-release@v1
      with:
        files: target/release/rustpress-server*
```

### CI完了条件

- 全プッシュで check, test, fmt, clippy, build が自動実行される
- セキュリティ監査（cargo audit）が定期実行される
- タグプッシュでLinux/macOS/Windows のバイナリが自動ビルド・リリースされる
- PRにCIステータスバッジが表示される
- README にCIバッジが追加されている

---

## 開発ルール

### ビルド・テスト

```bash
# コンパイル確認
cargo check --workspace

# ユニットテスト実行
cargo test --workspace --lib --bins -- --skip e2e

# 特定クレートのテスト
cargo test -p rustpress-api --lib

# E2Eテスト（Docker環境が必要）
docker compose --profile e2e up --build --abort-on-container-exit --exit-code-from e2e
```

### コード品質

```bash
cargo fmt --all
cargo clippy --workspace -- -D warnings
```

### コミットルール

- 機能追加: `feat: Add PUT/DELETE for /wp-json/wp/v2/posts/{id}`
- バグ修正: `fix: Correct X-WP-Total header count for filtered queries`
- CI: `ci: Add cargo-audit security scanning`
- コミットは小さく、頻繁に。1機能1コミット。

---

## 作業の優先順位

1. **既存APIエンドポイントの完全化** — クエリパラメータ、レスポンスヘッダー、エラー形式
2. **不足CRUDの追加** — PUT/PATCH/DELETE
3. **`_embed` 実装** — ヘッドレスCMSユーザーに必須
4. **未実装エンドポイント追加** — menus, blocks, templates等
5. **APIレスポンス比較テスト作成** — WordPress vs RustPress の自動比較
6. **`rustpress migrate` CLI実装** — analyze → theme → plugins → seo-audit → serve
7. **CI/CDプッシュ・追加ジョブ** — workflowスコープ追加後

---

## 完了条件

以下が全て満たされた時、あなたの仕事は完了です:

- [ ] WP REST API v2の全エンドポイントが実装されている（上記一覧の全て）
- [ ] 全エンドポイントでWordPressと同一のJSON構造を返す
- [ ] クエリパラメータ（_embed, _fields, per_page, page, search等）が全て動作する
- [ ] レスポンスヘッダー（X-WP-Total, X-WP-TotalPages, Link）が正しい
- [ ] エラーレスポンスがWordPress形式（code, message, data.status）
- [ ] APIレスポンス比較テストが全通過する
- [ ] `rustpress migrate --source <URL>` で既存WPサイトがRustPressで稼働する
- [ ] `rustpress migrate analyze` で互換性レポートが出力される
- [ ] GitHub Actions CIが全プッシュで自動実行される
- [ ] タグプッシュでバイナリが自動リリースされる
- [ ] READMEにCIバッジが表示されている
- [ ] 全ユニットテストが通る（cargo test --workspace）
- [ ] cargo clippy --workspace -- -D warnings が通る

---

## 自律的に動くこと

**あなたは自分で判断して進めてください。** 優先順位は指針として示していますが、状況に応じて順序を変えて構いません。ある作業が別の作業の前提になる場合は、自分で依存関係を分析して最適な順序を決めてください。

- 完了条件のチェックリストを上から順に潰す必要はない
- ブロッカーがあればスキップして別の項目を進める
- 「次何をすべきか」を毎回聞かない。自分で決めて進める。
- 進捗や判断の記録はコミットメッセージとコード内コメントで残す

## 判断原則

1. **WordPressのAPIレスポンスが正解。** 迷ったらWordPressに同じリクエストを送って確認する。
2. **後方互換性を壊さない。** 既存のエンドポイントの動作を変えるときは、既存テストが通ることを確認する。
3. **ヘッドレスCMSユースケースを意識する。** REST APIは外部フロントエンド（Next.js, Nuxt等）から使われる。レスポンス形式の互換性は最重要。
4. **MASTERPLANを読む。** 不明点があればプロジェクトルートの `MASTERPLAN.md` に詳細な仕様がある。


---

## QAレビューの確認

あなたの成果物はQA担当（#09）によってレビューされる。レビュー結果は以下に置かれる：

```
/home/ubuntu/RustPress-1/reviews/
```

**あなたの責任:**
1. レビューファイルを定期的に確認せよ
2. ブロッカー（blocker）指摘があれば即座に修正せよ
3. 警告（warning）指摘はマージ前に対応せよ
4. 提案（suggestion）は自分の判断で対応可否を決めてよい
5. 修正したらQA担当に再レビューを依頼せよ
