# RustPress マスタープラン
## "Save WordPress with Rust"

---

## 1. WordPress アーキテクチャ分析サマリー

WordPressは17個の主要サブシステムで構成されている。RustPressで再現すべき全体像:

```
┌─────────────────────────────────────────────────────────────┐
│                    WordPress Architecture                    │
├──────────┬──────────┬──────────┬──────────┬─────────────────┤
│ Routing  │ Template │  Admin   │ REST API │   Cron System   │
│ Rewrite  │ Hierarchy│ wp-admin │ /wp-json │   wp-cron.php   │
├──────────┴──────────┴──────────┴──────────┴─────────────────┤
│                    The Loop / WP_Query                       │
│          (メインクエリ, カスタムクエリ, ページネーション)         │
├─────────────────────────────────────────────────────────────┤
│  Post Types & Taxonomies    │  User & Role System           │
│  (post, page, CPT, terms)   │  (認証, 権限, セッション)       │
├─────────────────────────────┴───────────────────────────────┤
│                 Hook System (Plugin API)                     │
│     add_action / do_action / add_filter / apply_filters     │
│         ← WordPressの全機能がこの上に構築されている →          │
├─────────────────────────────────────────────────────────────┤
│                    Database Layer (wpdb)                     │
│  wp_posts │ wp_postmeta │ wp_users │ wp_usermeta │ wp_options│
│  wp_terms │ wp_term_taxonomy │ wp_term_relationships        │
│  wp_comments │ wp_commentmeta │ wp_links                    │
├─────────────────────────────────────────────────────────────┤
│  Object Cache  │  Transients  │  i18n  │  Media  │ Security │
└─────────────────────────────────────────────────────────────┘
```

### 最重要の設計原則

WordPressの核心は **Hook System** である。全てのサブシステムはフックを通じて結合されており、プラグインが任意のタイミングで動作を差し込める。RustPressでもこれを最優先で実装する必要がある。

### WordPressのDBスキーマ (全11テーブル)

| テーブル | 役割 | 行数目安 |
|---------|------|---------|
| `wp_posts` | 全コンテンツ (投稿, 固定ページ, 添付ファイル, リビジョン等) | 数百〜数万 |
| `wp_postmeta` | 投稿のメタデータ (EAVパターン) | wp_postsの10〜50倍 |
| `wp_users` | ユーザーアカウント | 数十〜数千 |
| `wp_usermeta` | ユーザーメタデータ (権限含む) | wp_usersの20倍 |
| `wp_options` | サイト設定 (autoload=yesは毎リクエスト読込) | 数百 |
| `wp_comments` | コメント | 数十〜数万 |
| `wp_commentmeta` | コメントメタデータ | 少量 |
| `wp_terms` | タグ・カテゴリ等のタクソノミー用語 | 数十〜数百 |
| `wp_term_taxonomy` | 用語とタクソノミーの関連 | wp_termsと同数 |
| `wp_term_relationships` | 投稿とタクソノミーの関連 | wp_postsの数倍 |
| `wp_links` | ブログロール (レガシー) | ほぼ0 |

---

## 2. リポジトリ戦略

### メインリポジトリ: Cargo Workspace モノレポ

```
rustpress/                          ← メインリポジトリ
├── Cargo.toml                      # Virtual manifest (workspace定義)
├── crates/
│   ├── rustpress-core/             # 型定義, トレイト, Hook System
│   ├── rustpress-db/               # SeaORM エンティティ, DB抽象化層
│   ├── rustpress-query/            # WP_Query相当のクエリエンジン
│   ├── rustpress-auth/             # 認証, セッション, ユーザー管理
│   ├── rustpress-server/           # Axum HTTPサーバー, ルーティング
│   ├── rustpress-api/              # WP REST API互換エンドポイント
│   ├── rustpress-themes/           # テーマエンジン (Tera), テンプレート階層
│   ├── rustpress-admin/            # 管理画面 (バックエンドAPI)
│   ├── rustpress-plugins/          # プラグインホスト (WASM runtime)
│   ├── rustpress-cli/              # CLIツール (wp-cli相当)
│   ├── rustpress-migrate/          # WP DBマイグレーションツール
│   └── rustpress-cache/            # オブジェクトキャッシュ, トランジェント
├── admin-ui/                       # 管理画面フロントエンド (別ビルド)
├── templates/                      # デフォルトテーマのテンプレート
├── static/                         # 静的アセット
├── migrations/                     # SeaORM マイグレーション
└── xtask/                          # ビルド自動化タスク
```

### サブリポジトリ (必要に応じて段階的に作成)

| リポジトリ | 用途 | いつ作るか |
|-----------|------|----------|
| `rustpress/rustpress` | メインのモノレポ (上記) | **今 (Phase 1)** |
| `rustpress/admin-ui` | 管理画面SPA (React/Solid) | Phase 5 で分離を検討 |
| `rustpress/theme-developer-kit` | テーマ開発者向けテンプレート・ドキュメント | Phase 4 完了後 |
| `rustpress/plugin-sdk` | WASM プラグイン開発SDK + テンプレート | Phase 6 完了後 |
| `rustpress/wp-import` | WordPress XMLインポーター | Phase 3 完了後 |
| `rustpress/docker` | Docker Compose セットアップ | Open時 |
| `rustpress/docs` | ドキュメントサイト (mdBook) | Open時 |
| `rustpress/homebrew-tap` | macOS用 Homebrewフォーミュラ | v0.5以降 |

**原則:** Phase 4までは全てモノレポ内で開発。分離はプロジェクトが成長してからでよい。

---

## 3. 開発フェーズ (超精密ロードマップ)

### Phase 1: Foundation ✅ 完了
> Axum + Tokio + SeaORM の最小構成

- [x] `cargo init` + 依存関係設定
- [x] Hello World HTTPサーバー
- [x] ヘルスチェックエンドポイント
- [x] プロジェクト構造の骨格

---

### Phase 2: WordPress DB読み取り
> **目標: 既存のWordPress DBに接続し、投稿を読み出す**

これが最初の「価値証明」。既存WPサイトのDBに繋いで、Rustで投稿を表示できることを実証する。

#### 2-1. DB接続とエンティティ生成
```
実装内容:
- SeaORM で MySQL接続プール構築
- sea-orm-cli で既存WP DBからエンティティ自動生成
- wp_posts, wp_postmeta, wp_users, wp_options のエンティティ
- AppState に DatabaseConnection を保持
- 接続設定を .env / config.rs から読み込み
```

#### 2-2. Options API
```
実装内容:
- wp_options テーブルからの読み取り
- autoload=yes のオプションを起動時に一括ロード → メモリキャッシュ
- get_option(key) / option_exists(key) の実装
- PHPのシリアライズ形式のデシリアライズ (既存WPデータ互換)
```

#### 2-3. 投稿の読み取りAPI
```
実装内容:
- GET /api/posts → 投稿一覧 (JSON)
- GET /api/posts/:id → 投稿詳細
- GET /api/posts/:slug → スラッグ指定
- 基本的なフィルタリング (post_type, post_status)
- ページネーション (limit, offset)
```

#### 2-4. ユーザーの読み取り
```
実装内容:
- wp_users エンティティ
- GET /api/users/:id → ユーザー情報 (パスワード除外)
```

**Phase 2 完了基準:**
`cargo run -- --db-url mysql://user:pass@localhost/wordpress` で起動し、
既存WPサイトの投稿一覧がJSONで返ること。

---

### Phase 3: Hook System + クエリエンジン
> **目標: WordPressの心臓部を構築**

#### 3-1. Hook System (rustpress-core)
```
実装内容:
- HookRegistry 構造体 (WP_Hook相当)
- add_action(tag, callback, priority) / do_action(tag, args)
- add_filter(tag, callback, priority) / apply_filters(tag, value, args)
- remove_action / remove_filter
- has_action / has_filter / did_action / doing_action
- コールバックの優先度ソート (BTreeMap使用)
- ネストされたフック実行のサポート
- スレッドセーフな設計 (Arc<RwLock<HookRegistry>>)
```

```rust
// 目指すAPI (イメージ)
hooks.add_filter("the_content", |content: String| {
    content.replace("\n\n", "<p>")
}, 10);

let html = hooks.apply_filters("the_content", raw_content);
```

#### 3-2. WP_Query相当のクエリエンジン (rustpress-query)
```
実装内容:
- PostQuery ビルダー (WP_Query相当)
  - post_type, post_status フィルタ
  - author, date_query, meta_query
  - tax_query (タクソノミークエリ)
  - orderby, order, posts_per_page
  - 検索 (s パラメータ)
- pre_get_posts フック (クエリ実行前の修正)
- the_posts フック (結果取得後の修正)
- ページネーション計算
```

```rust
// 目指すAPI
let query = PostQuery::new()
    .post_type("post")
    .status("publish")
    .posts_per_page(10)
    .meta_query(MetaQuery::new("price", Compare::Gte, "100"))
    .tax_query(TaxQuery::new("category", "tech"));

let results = query.execute(&db).await?;
```

#### 3-3. タクソノミーシステム
```
実装内容:
- wp_terms, wp_term_taxonomy, wp_term_relationships のエンティティ
- カテゴリ・タグの読み取り
- 投稿とタクソノミーの関連取得
- タクソノミーの登録 (register_taxonomy相当)
```

**Phase 3 完了基準:**
Hook SystemでフィルタチェーンとアクションがWordPressと同等に動作し、
PostQueryで複雑な条件指定のクエリが実行できること。

---

### Phase 4: テーマエンジン + フロントエンド表示
> **目標: HTMLページをレンダリングして人間が見られるサイトにする**

#### 4-1. テンプレート階層
```
実装内容 (WordPress Template Hierarchyの再現):
- テンプレート解決ロジック:
  single-{post_type}-{slug}.html
  → single-{post_type}.html
  → single.html
  → singular.html
  → index.html
- アーカイブ: archive-{post_type}.html → archive.html → index.html
- カテゴリ: category-{slug}.html → category-{id}.html → category.html
- 固定ページ: page-{slug}.html → page-{id}.html → page.html
- 404: 404.html → index.html
- テーマディレクトリの読み込みとスキャン
```

#### 4-2. テンプレートタグ
```
実装内容 (Teraカスタム関数として):
- the_title(), the_content(), the_excerpt()
- the_permalink(), the_author()
- get_header(), get_footer(), get_sidebar()
- wp_head(), wp_footer() (フック発火ポイント)
- bloginfo() (サイト情報)
- テンプレート内でのループ (have_posts / the_post 相当)
```

#### 4-3. デフォルトテーマ ("TwentyRust")
```
実装内容:
- index.html, single.html, page.html, archive.html, 404.html
- header.html, footer.html, sidebar.html (パーシャル)
- シンプルだが美しいCSS
- レスポンシブデザイン
```

#### 4-4. 静的ファイル配信
```
実装内容:
- tower-http ServeDir で /static/ を配信
- テーマ内のCSS/JS/画像の配信
- アップロードされたメディア (/wp-content/uploads/) の配信
```

**Phase 4 完了基準:**
既存WP DBの投稿が、テンプレートを通じてHTMLでレンダリングされ、
ブラウザでブログとして閲覧できること。

---

### Phase 5: 認証 + 管理API
> **目標: ログインしてコンテンツを管理できるようにする**

#### 5-1. 認証システム
```
実装内容:
- パスワードハッシュ検証 (PHPass/bcrypt互換 + Argon2新規)
- セッション管理 (JWT or セッションテーブル)
- ログイン / ログアウト API
- Cookie認証ミドルウェア
- CSRF保護 (ノンスシステム)
```

#### 5-2. ロール & 権限
```
実装内容:
- 組み込みロール: administrator, editor, author, contributor, subscriber
- 権限チェック (current_user_can 相当)
- wp_usermeta からロール/権限読み込み
- ルートごとの権限ガード (Axumミドルウェア)
```

#### 5-3. コンテンツ管理API (CRUD)
```
実装内容:
- POST /api/posts → 投稿作成
- PUT /api/posts/:id → 投稿更新
- DELETE /api/posts/:id → 投稿削除 (ゴミ箱)
- リビジョン管理 (post_type='revision')
- 投稿ステータスフロー (draft → pending → publish)
- メタデータのCRUD
```

#### 5-4. メディア管理
```
実装内容:
- ファイルアップロード (multipart/form-data)
- 画像リサイズ (thumbnail, medium, large)
- wp_posts (post_type='attachment') としての管理
- メディアライブラリAPI
```

**Phase 5 完了基準:**
管理者がログインし、投稿の作成・編集・削除ができること。
メディアのアップロードと表示ができること。

---

### Phase 6: プラグインシステム
> **目標: サードパーティによる機能拡張を可能にする**

#### プラグイン戦略: 二段構え

RustPressのプラグイン戦略は以下の2本柱で進める:

1. **主要プラグインはRustで再開発** (本リポジトリ内)
   - WordPress主要プラグイン相当の機能をRustネイティブで再実装
   - PHPの技術的負債なしにゼロから最適設計
   - 対象: EC, SEO, フォーム, カスタムフィールド, セキュリティ等

2. **中小プラグインはAI変換Webサービスで対応** (別リポジトリ)
   - PHPプラグインをアップロードすると、Rustプラグインに変換するWebサービス
   - Claude API等のLLMで関数/クラス単位で変換 + cargo checkで自動検証
   - 100%自動ではなく、80%スキャフォールド生成 + 20%手動調整を想定
   - リポジトリ: `rustpress/rustpress-convert` (別途作成)

#### 6-1. 主要プラグイン再開発 (Rustネイティブ)

| クレート | WordPress相当 | 概要 |
|---------|-------------|------|
| `rustpress-commerce` | WooCommerce | EC機能 (商品, カート, 決済) |
| `rustpress-seo` | Yoast / RankMath | SEOメタタグ, サイトマップ, OGP |
| `rustpress-forms` | Contact Form 7 / Gravity Forms | フォーム構築・送信 |
| `rustpress-fields` | ACF (Advanced Custom Fields) | カスタムフィールド管理 |
| `rustpress-security` | Wordfence | WAF, ログイン保護, 脆弱性スキャン |

#### 6-2. ネイティブRustプラグインAPI
```
実装内容:
- プラグイントレイト定義
  - fn name() -> &str
  - fn version() -> &str
  - fn activate(&self, hooks: &mut HookRegistry)
  - fn deactivate(&self)
- プラグインローダー (ディレクトリスキャン)
- プラグインの有効化/無効化の永続化
```

#### 6-3. WASMプラグイン (Extism利用)
```
実装内容:
- Extism hostランタイム統合
- WIT (WebAssembly Interface Types) でプラグインAPI定義
- サンドボックス内でのプラグイン実行
- プラグインからのフック登録
- ホスト関数の公開 (DB読み取り, オプション取得, etc.)
```

#### 6-4. プラグインSDK
```
実装内容:
- Rustプラグイン用テンプレート (cargo-generate)
- WASM PDK (Plugin Development Kit)
- サンプルプラグイン:
  - SEOメタタグ挿入
  - お問い合わせフォーム
  - サイトマップ生成
```

#### 6-5. AI変換Webサービス (rustpress-convert)
```
実装内容 (別リポジトリ):
- Web UI: PHPプラグインzipアップロード → 変換進捗表示 → 結果ダウンロード
- API: POST /convert でPHPプラグイン受付
- Worker: LLM APIでPHP→Rust変換 + cargo checkによる自動検証ループ
- CLI連携: rustpress-cli convert-plugin でサーバー経由の変換も可能
- 技術スタック: Axum + Tera (RustPress本体と統一)
```

**Phase 6 完了基準:**
Rust (ネイティブ) と WASM の両方でプラグインを書いて、
フックを通じて動作を拡張できること。
主要プラグイン (SEO, フォーム) の最低1つがRustネイティブで動作すること。

---

### Phase 7: REST API互換 + 管理画面UI
> **目標: WP REST API互換で既存ツールと連携可能に**

#### 7-1. WP REST API互換エンドポイント
```
実装内容:
- /wp-json/wp/v2/posts
- /wp-json/wp/v2/pages
- /wp-json/wp/v2/media
- /wp-json/wp/v2/users
- /wp-json/wp/v2/categories
- /wp-json/wp/v2/tags
- /wp-json/wp/v2/comments
- /wp-json/wp/v2/settings
- Discovery: /wp-json/ (APIルートインデックス)
- 認証: Application Passwords 対応
- _embed パラメータ (関連データ埋め込み)
```

#### 7-2. 管理画面UI
```
実装内容:
- SPA (React or Solid.js)
- ダッシュボード
- 投稿一覧・編集画面
- メディアライブラリ
- カテゴリ・タグ管理
- ユーザー管理
- 設定画面
- テーマ選択
- プラグイン管理
```

**Phase 7 完了基準:**
Gutenbergエディタ等の既存WPクライアントがRustPressのAPIに接続して動作すること。
独自管理画面で基本的なサイト管理ができること。

---

### Phase 8: 本番運用レベル
> **目標: 実際のサイトで使える品質に仕上げる**

#### 8-1. パフォーマンス最適化
```
- オブジェクトキャッシュ (インメモリ + Redis対応)
- ページキャッシュ (静的HTML生成)
- クエリ最適化 (N+1問題の解消)
- gzip / brotli 圧縮
```

#### 8-2. 運用機能
```
- コメント管理 + スパム対策
- 検索機能 (全文検索 or タイトル/コンテンツ検索)
- パーマリンク設定 (pretty URLs)
- リダイレクト管理
- wp-cron相当のタスクスケジューラ (Tokio-based)
- XML Sitemap生成
- RSS/Atom フィード
```

#### 8-3. セキュリティ強化
```
- レート制限
- CSP ヘッダー
- XSS対策 (コンテンツエスケープ)
- SQLインジェクション対策 (SeaORM + パラメタライズドクエリ)
- 入力バリデーション
- セキュリティ監査
```

#### 8-4. CLI (wp-cli相当)
```
- rustpress post list / create / update / delete
- rustpress user create / list
- rustpress option get / set
- rustpress db export / import
- rustpress server start / stop
- rustpress plugin install / activate / deactivate
- rustpress theme install / activate
```

---

### Phase 9: 完全WordPress互換
> **目標: 全てのWordPressユーザーが移行可能な状態を実現する**

#### 9-1. 国際化 (i18n)
```
実装内容:
- rustpress-i18n クレート新設
- .mo ファイルのバイナリパーサー (GNU gettext形式)
- 翻訳関数: __(), _e(), _n(), _x(), _nx() をTeraカスタム関数として登録
- ロケール解決: Accept-Languageヘッダー + WPLANGオプション
- 既存WordPressの wp-content/languages/ をそのまま読み込み
- 管理画面・テーマ・プラグイン全てで翻訳関数を使用可能に
```

#### 9-2. Gutenbergブロックレンダリング
```
実装内容:
- rustpress-blocks クレート新設
- <!-- wp:xxx --> 形式のブロックコメントパーサー
- ブロックレジストリ: 各ブロック型のレンダラーを登録
- 標準ブロック網羅:
  - テキスト系: paragraph, heading, list, quote, code, preformatted, verse
  - メディア系: image, gallery, audio, video, cover, file
  - レイアウト系: columns, group, row, stack, spacer, separator
  - ウィジェット系: shortcode, archives, categories, latest-posts, latest-comments
  - 埋め込み系: embed (YouTube, Twitter, etc.)
  - テーマ系: site-title, site-logo, navigation, query-loop, post-title, post-content
- 編集側: WordPress公式 @wordpress/block-editor をnpmからバンドルし、
  RustPressのREST API経由で接続 (Phase 7のAPI互換が前提)
```

#### 9-3. マルチサイト対応
```
実装内容:
- rustpress-multisite クレート新設
- wp_blogs, wp_site, wp_sitemeta エンティティ追加
- テーブルプレフィックス動的切り替え (wp_2_posts, wp_3_posts 等)
- Axumミドルウェアでリクエストのドメイン/パスからサイトID解決
- サブディレクトリ方式 (example.com/site2/) とサブドメイン方式 (site2.example.com) の両対応
- ドメインマッピング (カスタムドメインの割り当て)
- ネットワーク管理画面 (スーパー管理者)
- サイト間でのユーザー共有
```

#### 9-4. XML-RPC互換
```
実装内容:
- rustpress-api 内に xmlrpc モジュール追加
- XMLパース: quick-xml クレート使用
- 主要メソッド:
  - wp.getPosts, wp.newPost, wp.editPost, wp.deletePost
  - wp.getMediaLibrary, wp.uploadFile
  - wp.getTerms, wp.newTerm
  - metaWeblog.newPost, metaWeblog.editPost, metaWeblog.getPost
  - blogger.getUsersBlogs, blogger.deletePost
  - wp.getOptions, wp.setOptions
  - pingback.ping, pingback.extensions.getPingbacks
- セキュリティ: デフォルト無効、設定で有効化
```

#### 9-5. WP-CLI完全互換
```
実装内容:
- rustpress-cli にWP-CLI互換のコマンド体系を完全実装
- エイリアス: rustpress wp <command> でも rustpress <command> でも動作
- 必須コマンド:
  - core: version, update, verify-checksums
  - post: list, create, update, delete, generate
  - user: create, list, update, delete, reset-password
  - option: get, set, delete, list
  - db: export, import, query, search, optimize, repair
  - plugin: list, install, activate, deactivate, uninstall
  - theme: list, install, activate, delete
  - cache: flush, type
  - search-replace: (DB内の文字列一括置換、ドメイン移行時に必須)
  - export: (WordPress eXtended RSS形式)
  - import: (WXR形式のインポート)
  - media: regenerate, import
  - rewrite: flush, list
  - cron: event list, event run
- 出力形式: table, csv, json, yaml 対応 (--format フラグ)
```

#### 9-6. 古いWordPressバージョン対応
```
対応方針:
- Tier 1 (完全対応): WP 6.0〜6.9
  → DBスキーマ差分がほぼない。メインターゲット。
- Tier 2 (基本対応): WP 5.0〜5.9
  → Gutenbergブロック形式対応、REST API v2あり。
  → Classic Editor使用サイトも対応。
- Tier 3 (レガシー対応): WP 4.4〜4.9
  → wp_termmeta 追加以降。Classic Editor前提。
  → XML-RPC中心のクライアント対応。
- Tier 4 (非対応): WP 4.3以前
  → 市場シェアがほぼゼロ、対応コスト不釣り合い。

実装:
- rustpress-compat クレート新設
- DB接続時に wp_options の db_version からWPバージョンを自動検出
- バージョン別の互換レイヤー (スキーマ差分の吸収)
- マイグレーション不要: 読み取り時にバージョン差を吸収
```

**Phase 9 完了基準:**
多言語サイト、Gutenbergコンテンツ、マルチサイト構成のWordPressが
RustPressで正常に動作すること。WP 5.0以降のサイトが移行可能であること。

---

### Phase 10: エコシステムと移行戦略
> **目標: 全WordPressユーザーの移行経路を確立する**

#### 10-1. 移行ツール (rustpress-cli migrate)
```
実装内容:
- rustpress migrate analyze
  → 既存WPサイトを分析し互換性レポート生成
  → 「互換性: 95%、非対応プラグイン: 2個（代替あり）」形式
  → 移行の心理的障壁を下げる最重要機能

- rustpress migrate database
  → DB接続設定の移行 (SKIP_MIGRATIONSモード)
  → wp_options の siteurl/home を自動更新

- rustpress migrate media
  → wp-content/uploads/ のコピーまたはシンボリックリンク
  → S3/CloudFront等のCDN設定移行

- rustpress migrate theme
  → PHPテーマ → Teraテンプレート変換 (AI支援)
  → 主要テーマ (Astra, GeneratePress, Flavor等) のマッピング提供

- rustpress migrate plugins
  → プラグイン互換性チェック
  → 代替Rustプラグイン提案 (例: Yoast → rustpress-seo)
  → AI変換サービスへの誘導

- rustpress migrate rewrites
  → .htaccess / nginx.conf → RustPress設定変換
  → リダイレクトルールの移行

- rustpress migrate full
  → 上記を一括実行するウィザード
```

#### 10-2. ホスティング戦略
```
A. セルフホスト向け (現在):
- Docker Compose ワンコマンドセットアップ
- VPS向け1行インストール: curl -sSf https://install.rustpress.dev | sh
- systemdサービスファイル同梱
- Ansible / Terraform テンプレート提供

B. マネージドホスティング (中期):
- RustPress Cloud (公式ホスティングSaaS)
  → WordPress.com に対する RustPress.dev の位置づけ
  → 最大のマネタイズポイント
- PaaS公式テンプレート: Fly.io, Railway, Render
- ワンクリックデプロイ: DigitalOcean Marketplace, AWS Marketplace

C. 共有ホスティング対応 (長期):
- musl静的リンクバイナリで共有ホスティングでも動作
- cPanel / Plesk 用インストーラー開発
- ホスティング会社パートナーシップ (RustPress対応プラン)
```

#### 10-3. テーマエコシステム
```
実装内容:
- 公式テーマストア (rustpress.dev/themes)
- テーマ開発者向けドキュメント + スターターキット
- WordPress主要テーマのRustPress版:
  - TwentyTwentyFive相当 (デフォルトテーマ、実装済み)
  - Astra相当 (軽量多目的テーマ)
  - GeneratePress相当 (高速テーマ)
- PHPテーマ → Teraテンプレート変換ガイド
- テーマのAI変換 (rustpress-convert で対応)
```

#### 10-4. プラグインエコシステム
```
実装内容:
- 公式プラグインストア (rustpress.dev/plugins)
- プラグインの自動更新機構
- プラグイン開発者向けドキュメント + SDK
- コミュニティ製プラグインの審査・公開フロー
- AI変換サービス (rustpress-convert) との統合:
  → 変換結果を直接プラグインストアに公開可能
```

#### 10-5. SEO安全移行
```
課題:
- URL構造・metaタグ・構造化データが1ビットでも変わるとGoogle検索順位が下落
- WPユーザーにとってSEO順位はビジネス直結。移行でSEOが落ちるなら誰も移行しない

解決策:
- rustpress migrate seo-audit
  → 移行前のWP出力と移行後のRustPress出力をURL単位でHTML diff比較
  → title, meta description, canonical, OGP, 構造化データ (JSON-LD) の差分検出
  → 差分がある場合は修正提案を自動生成
- SEOチェックリスト自動検証:
  → robots.txt の同等性
  → XML Sitemap の全URL一致確認
  → 301リダイレクトの網羅性 (旧URL → 新URL)
  → hreflang タグの維持 (多言語サイト)
  → Core Web Vitals (LCP, CLS, INP) の移行前後比較
- 段階移行モード:
  → リバースプロキシで一部URLだけRustPressに切り替え
  → 問題なければ順次拡大、問題あれば即時ロールバック
  → A/Bテスト的にSEO影響をゼロリスクで検証可能
```

#### 10-6. 自動更新機構
```
課題:
- WordPressは管理画面からワンクリックで本体・テーマ・プラグインを更新可能
- 一般ユーザーはCLIやSSHに慣れていない。自動更新がないと移行障壁が高い

解決策:
- RustPressバイナリの自動更新:
  → 管理画面の「更新」ページでワンクリック更新
  → 裏で新バイナリをダウンロード → 検証 → 旧バイナリと入れ替え → プロセス再起動
  → ロールバック機能: 更新失敗時は旧バイナリに自動復帰
  → 更新チャネル: stable / beta / nightly (ユーザーが選択)
- テーマ・プラグインの自動更新:
  → 公式ストア (rustpress.dev) から最新版を取得
  → WASMプラグインはバイナリ差し替えだけで更新完了 (再コンパイル不要)
  → ネイティブRustプラグインはプリビルドバイナリを配布
- 更新通知:
  → 管理画面ダッシュボードにバッジ表示
  → メール通知 (オプション)
  → セキュリティ更新は自動適用 (WordPressと同じ挙動)
```

#### 10-7. コミュニティ戦略
```
課題:
- OSSプロジェクトの成否はコミュニティの規模と活発さで決まる
- 開発者を引き込むにはドキュメント・チュートリアル・コミュニケーション基盤が不可欠

解決策:
Phase 5 (Public Beta) までに整備:
- GitHub Discussions: Q&A, アイデア, Show & Tell
- Discord サーバー: #general, #dev, #plugins, #themes, #support, #japanese
- CONTRIBUTING.md: コントリビューションガイド (Rust初心者向け含む)
- Good First Issue ラベル: 新規コントリビューター向けのタスクを常時確保

Phase 7 (v0.1.0) までに整備:
- ドキュメントサイト (rustpress.dev/docs): mdBook or Docusaurus
  → Getting Started, テーマ開発ガイド, プラグインSDKリファレンス, API仕様
- チュートリアルシリーズ: 「WordPressからRustPressへ移行する」ステップバイステップ
- ブログ: 技術解説、ベンチマーク、ロードマップ更新

Phase 9 (v2.0) までに整備:
- RustPress Meetup (オンライン): 月1回の開発者ミートアップ
- プラグイン開発者プログラム: 早期アクセス + 技術サポート
- テーマコンテスト: コミュニティ製テーマの募集・表彰
- WordPress開発者向けリクルーティング: 「PHPの経験を活かしてRustへ」のストーリー
```

#### 10-8. 決済プロバイダ連携 (rustpress-commerce)
```
課題:
- ECサイト移行には決済連携が不可欠
- WooCommerceは100以上の決済ゲートウェイに対応。全再実装は非現実的

解決策:
- Tier 1 (本体組み込み): Stripe, PayPal — 世界シェア80%超をカバー
  → Stripe: stripe-rs クレートで直接連携
  → PayPal: REST API直接連携
- Tier 2 (公式プラグイン): Square, Authorize.net, Mollie, Razorpay
  → 各地域の主要決済をカバー
- Tier 3 (汎用アダプタ): 決済ゲートウェイ抽象化トレイト
  → PaymentGateway トレイトを定義し、コミュニティがアダプタを実装可能
  → WooCommerce決済プラグインのAI変換でアダプタ生成を支援
- WooCommerce互換API:
  → /wp-json/wc/v3/ 互換エンドポイント
  → 既存のWooCommerceクライアントアプリがそのまま動作
```

#### 10-9. 法的リスク対策
```
課題:
- "WordPress" は Automattic/WordPress Foundation の登録商標
- 「WordPress互換」を謳う際の商標リスク
- GPL v2 ライセンスの遵守

対策:
- 名称: "RustPress" はWordPress商標を含まない → 安全
- マーケティング: 「for WordPress」ではなく「WordPress-compatible」を使用
  → "compatible with" は記述的使用 (nominative fair use) として認められる
  → 例: "RustPress — A WordPress-compatible CMS" ○
  → 例: "RustPress for WordPress" ○
  → 例: "RustPress WordPress" × (混同を招く)
- ロゴ: WordPress の "W" ロゴは絶対に使用しない
- GPL v2 遵守:
  → RustPress本体は GPL v2 (WordPress と同じ)
  → プラグイン/テーマは GPL v2+ (WPエコシステムの慣行に合わせる)
  → 商用プラグインもGPLで配布 (サポート/ホスティングで収益化)
- ドメイン: rustpress.dev を使用。wordpress を含むドメインは取得しない
- 定期的な法務レビュー: v1.0前に知財弁護士に確認
```

**Phase 10 完了基準:**
任意のWordPressサイトに対して `rustpress migrate analyze` を実行し、
移行パスが明確に提示されること。
RustPress Cloudで新規サイトを30秒以内に立ち上げられること。
SEO安全移行が検証済みで、移行前後で検索順位に影響がないこと。
自動更新機構により、一般ユーザーが管理画面からワンクリックで更新可能であること。

---

## 4. オープンソース公開戦略

### いつOpenにするか？

```
Phase 1-3: Private (非公開)
  → 基盤が不安定な状態で公開すると、APIが頻繁に変わり信頼を失う
  → この段階では2人称で開発に集中する

Phase 4 完了時: Limited Preview (限定公開)
  → 「既存WP DBに接続して投稿をHTMLで表示できる」= 最初の価値証明
  → 招待制でフィードバックを収集
  → README, CONTRIBUTING.md, ライセンス(GPL v2)を整備

Phase 5 完了時: Public Beta (一般公開) ★ ここがOpen Source公開ポイント
  → ログイン + CRUD ができる = 最小限のCMSとして成立
  → ベンチマーク結果を添えて「WordPress 100x faster」のストーリーで公開
  → Hacker News, Reddit r/rust, r/wordpress に投稿
  → GitHub Discussions を有効化しコミュニティを構築

Phase 7 完了時: v0.1.0 Release
  → REST API互換 = 既存エコシステムとの接続点
  → Docker Compose ワンコマンドセットアップ

Phase 8 完了時: v1.0 Release
  → 本番運用レベルの品質

Phase 9 完了時: v2.0 Release
  → 完全WordPress互換 (i18n, Gutenberg, マルチサイト, 旧バージョン)
  → 「あらゆるWPサイトがRustPressに移行可能」の実現

Phase 10 完了時: WordPress Killer
  → 移行ツール + ホスティング + エコシステム完成
  → RustPress Cloud 本格運用
  → 全WPユーザーの移行経路が確立
```

### 公開前に必要なもの

| 項目 | 必須タイミング |
|------|--------------|
| LICENSE (GPL v2) | Limited Preview前 |
| README.md (英語, 日本語) | Limited Preview前 |
| CONTRIBUTING.md | Public Beta前 |
| CODE_OF_CONDUCT.md | Public Beta前 |
| SECURITY.md | Public Beta前 |
| CI/CD (GitHub Actions) | Public Beta前 |
| テスト (unit + integration) | 各Phase完了時 |
| ベンチマーク結果 | Public Beta前 |
| Docker Compose | Public Beta前 |
| ドキュメントサイト | v0.1.0前 |

### 公開時の「物語」

**ポジショニング:** "WordPress, but 100x faster — powered by Rust"

**訴求ポイント:**
1. 既存WP DBにそのまま接続 → ゼロマイグレーション
2. レスポンス: PHP WordPress ~200ms → RustPress ~2ms
3. メモリ使用量: PHP ~50MB → Rust ~5MB
4. セキュリティ: Rustの所有権システムによる脆弱性の構造的排除
5. 単一バイナリ配布: `curl | sh` でインストール完了

---

## 5. 技術スタック確定

| レイヤー | 技術 | 理由 |
|---------|------|------|
| Web Framework | Axum 0.8 | Tokioエコシステム, Tower middleware, 業界コンセンサス |
| Async Runtime | Tokio | デファクトスタンダード |
| ORM | SeaORM 1.1 | 既存DB→エンティティ自動生成, async-native |
| Template | Tera → MiniJinja検討 | ランタイムローディング必須 (テーマシステム) |
| Plugin Runtime | Extism (Wasmtime) | サンドボックス, 多言語プラグイン対応 |
| Serialization | serde / serde_json | Rustのデファクト |
| Auth | argon2 + JWT | 新規ユーザーはArgon2, WP既存ユーザーはPHPass互換 |
| Cache | moka (インメモリ) + Redis | 高速インメモリ + 分散キャッシュ |
| Admin UI | React or Solid.js | WP Gutenberg互換を視野に |
| CLI | clap | Rustの標準CLIフレームワーク |
| Testing | cargo test + sqlx-test | ユニット + 統合テスト |
| CI | GitHub Actions | Rust公式の標準 |

---

## 6. 各クレートの責務と依存関係

```
rustpress-core        ← 依存なし (型, トレイト, Hook System)
    ↑
rustpress-db          ← core (SeaORMエンティティ, DB接続)
    ↑
rustpress-i18n        ← core (.mo/.poパース, 翻訳関数)
    ↑
rustpress-query       ← core, db (PostQuery, TaxQuery)
    ↑
rustpress-auth        ← core, db (認証, セッション)
    ↑
rustpress-blocks      ← core (Gutenbergブロックパース, レンダリング)
    ↑
rustpress-themes      ← core, query, i18n, blocks (テンプレート階層, レンダリング)
    ↑
rustpress-api         ← core, db, query, auth (REST API + XML-RPC)
    ↑
rustpress-plugins     ← core (WASMプラグインホスト)
    ↑
rustpress-admin       ← api, auth (管理画面API)
    ↑
rustpress-multisite   ← core, db (マルチサイト, テーブルプレフィックス切替)
    ↑
rustpress-compat      ← core, db (古いWPバージョン互換レイヤー)
    ↑
rustpress-server      ← 全クレート (Axumサーバー, ルーティング, 統合)
    ↑
rustpress-cli         ← server, db, migrate (CLIツール + 移行ウィザード)

主要プラグイン (独立クレート):
rustpress-commerce    ← core, db, query (EC機能)
rustpress-seo         ← core, db, themes (SEO)
rustpress-forms       ← core, db, themes (フォーム)
rustpress-fields      ← core, db (カスタムフィールド)
rustpress-security    ← core, auth (WAF, セキュリティ)
```

---

## 7. マイルストーンとタイムライン目安

| マイルストーン | 内容 | Phase |
|-------------|------|-------|
| **DB Reader** | WP DBから投稿をJSON取得 | 2 |
| **Hook Engine** | add_action/apply_filters動作 | 3 |
| **First Render** | ブラウザでブログ記事表示 | 4 |
| **Limited Preview** | 招待制で限定公開 | 4完了時 |
| **Auth & CRUD** | ログイン + 投稿編集 | 5 |
| **Public Beta** | GitHub公開, HN投稿 | 5完了時 |
| **Plugin MVP** | WASMプラグイン + 主要プラグイン1つ | 6 |
| **API Compat** | WP REST API互換 | 7 |
| **v0.1.0** | 最初の正式リリース | 7完了時 |
| **Production Ready** | 本番運用レベル | 8 |
| **v1.0** | 安定版リリース | 8完了時 |
| **Full WP Compat** | i18n, Gutenberg, マルチサイト, 旧バージョン対応 | 9 |
| **v2.0** | 完全WordPress互換 | 9完了時 |
| **Ecosystem** | 移行ツール, ホスティング, ストア | 10 |
| **WordPress Killer** | 全WPユーザー移行可能 | 10完了時 |

---

## 8. 競合との差別化

| | WordPress (PHP) | Strapi | Ghost | **RustPress** |
|---|---|---|---|---|
| 言語 | PHP | Node.js | Node.js | **Rust** |
| 速度 | 遅い (~200ms) | 普通 | 速い | **極速 (~2ms)** |
| メモリ | 50-100MB | 100-200MB | 50-100MB | **5-15MB** |
| 既存WP DB | ✅ | ❌ | ❌ | **✅** |
| プラグイン数 | 59,000+ | 少ない | 少ない | **WP互換目標** |
| テーマ | 豊富 | ヘッドレス | 限定的 | **WP互換目標** |
| セキュリティ | 脆弱 | 普通 | 良い | **構造的に安全** |
| デプロイ | LAMP必要 | Node.js | Node.js | **単一バイナリ** |

**最大の差別化: 既存WordPress DBにそのまま接続できる唯一のRust CMS**

---

## 9. リスクと対策

| リスク | 影響度 | 対策 |
|-------|-------|------|
| WordPressの全機能再現は膨大 | 高 | Phase 9まで段階的に網羅。Phase別に完了基準を明確化 |
| PHPプラグイン互換は困難 | 高 | 主要プラグインはRustで再開発、中小プラグインはAI変換Webサービスで移行支援。PHP直接実行は行わない |
| SeaORMの制約 | 中 | 必要に応じてraw SQLにフォールバック |
| WP DBのPHPシリアライズデータ | 中 | php-serialize クレートで対応 |
| 一人での開発は遅い | 高 | Phase 5でOpen → コミュニティ構築。AI駆動開発でスピード担保 |
| APIが安定しない | 中 | Phase 3まではbreaking change許容, Phase 5以降はsemver厳守 |
| Gutenbergブロック互換の維持 | 高 | WordPress本体のブロック追加に追従する必要あり。ブロックレジストリを拡張可能に設計 |
| 多言語対応の網羅性 | 中 | 既存WPの.moファイルをそのまま使用し、翻訳コミュニティの資産を活用 |
| マルチサイトの複雑性 | 高 | 専用クレート (rustpress-multisite) で隔離。段階的に対応 |
| 古いWPバージョンのDBスキーマ差異 | 中 | rustpress-compat でバージョン別互換レイヤー。Tier 1-3の段階対応 |
| 共有ホスティング非対応 | 高 | 短期: VPS/Docker、中期: RustPress Cloud (SaaS)、長期: cPanel対応 |
| 移行時のデータロス/破損 | 高 | rustpress migrate analyze で事前チェック。ドライラン機能で安全確認 |
| SEO順位下落による移行拒否 | 最高 | seo-audit で移行前後のHTML diff検証。段階移行モードでゼロリスク切り替え |
| 決済連携の不足でEC移行不可 | 高 | Stripe/PayPal本体組込 + 決済ゲートウェイ抽象化トレイトでコミュニティ拡張可能に |
| "WordPress" 商標リスク | 中 | "WordPress-compatible" の記述的使用に留める。v1.0前に知財弁護士レビュー |
| コミュニティが育たない | 高 | Phase 5でDiscord/Discussions開設。Good First Issue常時確保。開発者プログラム |
| 自動更新がないと一般ユーザーが使えない | 高 | 管理画面ワンクリック更新。セキュリティ更新は自動適用。ロールバック機能付き |
