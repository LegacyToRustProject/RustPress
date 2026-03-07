# RustPress マスタープラン
## "Save WordPress with Rust"

---

## 0. このプロジェクトの根幹思想

### なぜ「全WordPressサイトの完全移行」が可能と言えるのか

WordPressは100%オープンソースである。WordPress Coreの約400,000行のPHP、2,000個のWP関数、59,000以上のプラグイン — 全てのソースコードが読める。つまり「正解」が完全に存在する。

従来、このスケールの書き直しは人間の作業量として非現実的だった。しかしAI（LLM）の登場により、状況が根本的に変わった:

```
1. AIがWordPressのPHPソースコードを読む（正解の仕様書）
2. AIがRustに変換する
3. WordPress出力と比較テストする（正解と照合）
4. 差分があればAIが修正する（正解が存在するから必ず直せる）
5. 1-4を繰り返せば、100%の互換性に収束する
```

これは「技術的に困難」な問題ではない。「作業量が膨大」な問題である。そしてAIは、まさにこの種の「正解が明確で、量が膨大な泥臭い作業」を得意とする。

**RustPressが成立する理由は、Rustの速度でもAxumの性能でもない。「正解のソースコードが存在する泥臭い変換作業を、AIがスケールさせられる時代になった」からである。** この認識を忘れてはならない。

→ 詳細な議論の記録: [ADR-001: PHP Bridge Mode の採否とプラグイン互換性戦略](docs/adr/001-php-bridge-mode.md)

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

#### 4-5. FSE (Full Site Editing) / ブロックテーマ対応
```
課題:
- WP 6.0+のデフォルトテーマ (TT25含む) はFSEブロックテーマ
- theme.json がテーマの設定・スタイルを定義する標準形式
- ブロックテンプレート/テンプレートパーツがPHP/Teraではなくブロックマークアップで記述

実装内容:
- theme.json パーサー (設定, スタイル, テンプレート定義)
- ブロックテンプレート (HTMLファイル内のブロックマークアップ)
- テンプレートパーツ (header, footer等のブロック構成パーツ)
- グローバルスタイル (theme.json → CSS変数生成)
- FSEテーマのブロックマークアップ → Teraレンダリングへの変換パイプライン
- カスタムテンプレート定義 (theme.json の customTemplates)
```

#### 4-6. レガシーウィジェットシステム
```
課題:
- ブロックウィジェット化されていない旧テーマがWPサイトの大半を占める
- register_widget() / dynamic_sidebar() はテーマ表示の基本機能

実装内容:
- register_widget() / dynamic_sidebar() 互換テンプレートタグ
- 標準ウィジェット実装:
  → 最近の投稿, カテゴリ一覧, アーカイブ, テキスト, カスタムHTML
  → 検索, メタ情報, RSS, タグクラウド, ナビゲーションメニュー
- ウィジェットエリア定義 (register_sidebar 互換)
- ウィジェットデータ (widget_* オプション) の読み取り・レンダリング
- ブロックウィジェット (WP 5.8+) との共存
```

#### 4-7. ナビゲーションメニュー完全互換
```
課題:
- wp_nav_menu() はほぼ全テーマで使用される最重要テンプレートタグ
- カスタムウォーカーでBootstrap/Tailwind等のCSSフレームワーク用出力を行うテーマが多数

実装内容:
- wp_nav_menu() 互換テンプレートタグ (Teraカスタム関数)
- メニューロケーション (register_nav_menus 互換)
- メニュー階層 (親子関係) のネストレンダリング
- メニューアイテムタイプ: 投稿, 固定ページ, カテゴリ, カスタムURL
- CSSクラス自動付与:
  → current-menu-item, current-menu-ancestor, current-menu-parent
  → menu-item-has-children, menu-item-type-*, menu-item-object-*
- カスタムウォーカー相当の出力カスタマイズ機構 (Rustトレイト)
- メニューのキャッシュ (変更時のみ再構築)
```

**Phase 4 完了基準:**
既存WP DBの投稿が、テンプレートを通じてHTMLでレンダリングされ、
ブラウザでブログとして閲覧できること。
FSEブロックテーマ、レガシーウィジェット、ナビゲーションメニューが正常に動作すること。

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

#### 6-6. mu-plugins互換
```
課題:
- エンタープライズ/マネージドWP (WP Engine, Kinsta等) ではmu-pluginsが標準
- wp-content/mu-plugins/ に配置されたプラグインは自動ロード、無効化不可

実装内容:
- plugins/must-use/ ディレクトリの自動スキャン・ロード
- ネイティブRustプラグインのmu-plugins配置対応
- ロード順序の制御 (ファイル名アルファベット順、WP互換)
- 管理画面の「Must-Use」タブでの一覧表示 (無効化ボタンなし)
- 移行時: wp-content/mu-plugins/ の検出とAI変換対象への自動追加
```

#### 6-7. Drop-in互換
```
課題:
- WPコアの動作をオーバーライドするdrop-inファイルが存在する
- db.php, object-cache.php, advanced-cache.php, sunrise.php 等
- これらを使うサイトは高度なカスタマイズ済みで、移行難易度が高い

実装内容:
- 各drop-inの機能をRustPress設定で代替:
  → object-cache.php → rustpress-cache の Redis/Memcached設定
  → advanced-cache.php → ページキャッシュ設定 (rustpress.toml)
  → db.php → SeaORM設定 (接続プール、クエリログ等)
  → sunrise.php → マルチサイトドメインマッピング設定
  → maintenance.php → メンテナンスモード設定
  → blog-deleted.php, blog-inactive.php, blog-suspended.php → マルチサイト状態管理
- 移行ツール: drop-inの自動検出と代替設定の提案
- 移行レポートに「drop-in検出: object-cache.php → Redis設定を推奨」形式で表示
```

#### 6-8. WP関数互換レイヤー (rustpress-wp-compat)
```
課題:
- AI変換されたRustプラグインはWP関数を呼び出す
- RustPress側にWP関数の互換APIが揃っていないと変換プラグインが動作しない
- WPコアには約2,000の公開関数がある

実装内容:
- Tier 1 (必須 - 上位50関数、プラグインの95%が使用):
  → データ: get_option, update_option, delete_option, add_option
  → 投稿: get_post, wp_insert_post, wp_update_post, wp_delete_post
  → クエリ: get_posts, WP_Query, get_pages, get_post_meta, update_post_meta
  → ユーザー: get_current_user_id, wp_get_current_user, current_user_can, is_user_logged_in
  → セキュリティ: wp_nonce_field, wp_verify_nonce, check_admin_referer
  → 出力: esc_html, esc_attr, esc_url, wp_kses, wp_kses_post
  → 入力: sanitize_text_field, absint, sanitize_email
  → フック: add_action, add_filter, do_action, apply_filters, remove_action
  → URL: home_url, site_url, admin_url, plugins_url, content_url
  → 状態判定: is_admin, is_single, is_page, is_archive, is_home, is_front_page

- Tier 2 (重要 - 次の100関数):
  → アセット: wp_enqueue_script, wp_enqueue_style, wp_register_script, wp_localize_script
  → テンプレート: get_template_part, locate_template, get_header, get_footer
  → メール: wp_mail
  → HTTP: wp_remote_get, wp_remote_post, wp_remote_request
  → キャッシュ: set_transient, get_transient, delete_transient, wp_cache_get, wp_cache_set
  → Cron: wp_schedule_event, wp_unschedule_event, wp_next_scheduled
  → 日時: current_time, date_i18n, human_time_diff
  → ファイル: wp_upload_dir, wp_get_attachment_url, wp_get_attachment_image
  → タクソノミー: get_terms, get_the_terms, wp_get_post_terms, get_term_link

- Tier 3 (プラグイン依存):
  → DB直接: $wpdb->prepare, $wpdb->get_results, $wpdb->insert, $wpdb->update, $wpdb->delete, $wpdb->query
  → 管理画面: WP_List_Table, add_menu_page, add_submenu_page, add_meta_box
  → エラー: WP_Error, is_wp_error
  → ウォーカー: Walker クラス互換トレイト
  → ショートコード: add_shortcode, do_shortcode
  → ウィジェット: register_widget, WP_Widget
  → リライト: add_rewrite_rule, add_rewrite_tag, flush_rewrite_rules

- 互換関数カバレッジダッシュボード:
  → 実装済み/未実装の一覧をrustpress.dev/compatで公開
  → プラグイン変換時に使用されるWP関数を検出し、未実装なら警告
```

#### 6-9. プラグイン間依存関係の解決
```
課題:
- WooCommerce拡張プラグイン (WooCommerce Subscriptions等) はWooCommerceに依存
- プラグインAがプラグインBのクラス/関数を直接呼び出すケースが多数
- 依存関係を無視して個別変換すると動作しない

実装内容:
- プラグインヘッダーの依存宣言パース (Requires Plugins ヘッダー, WP 6.5+)
- PHPコード解析による暗黙的依存の検出 (クラス参照, 関数呼び出し)
- 依存グラフの構築と可視化
- 変換順序の自動決定 (トポロジカルソート)
- 基盤プラグインの優先変換:
  → WooCommerce → WooCommerce拡張群
  → ACF → ACF依存テーマ/プラグイン
  → Elementor → Elementor Addons
- 依存プラグインのモック/スタブ自動生成 (変換中の部分テスト用)
```

#### 6-10. AI変換サービス完全版 (rustpress-convert 拡充)
```
課題:
- 「80%スキャフォールド+20%手動」では非エンジニアユーザーは移行不可
- PHP特有の動的パターンのRust変換が構造的に困難
- 変換後の品質保証が cargo check だけでは不十分

(a) 非エンジニア向け自動化率向上:
- 目標: 95%自動 + 5%Web UIでの対話的補完
- 変換後の設定項目 (API キー, エンドポイントURL等) をWeb UIフォームで入力
- 手動コード修正が必要な箇所はAIが修正候補を3つ提示
- 完全自動変換不可の場合:
  → コミュニティ変換リクエスト掲示板への投稿
  → 代替Rustプラグインの提案
  → 「このプラグインの機能はrustpress-seoに含まれています」形式のマッピング

(b) 外部API呼び出しの自動変換:
- cURL / wp_remote_get/post → reqwest への自動変換
- API認証情報の安全な移行 (暗号化ストレージ)
- REST API クライアントコードのパターン認識と変換
- Webhook受信エンドポイントの自動登録

(c) PHP動的パターンの変換戦略:
- マジックメソッド (__get, __set, __call) → Rustトレイト実装
- mixed型 → serde_json::Value または enum ディスパッチ
- 動的プロパティ → HashMap<String, Value>
- eval() / call_user_func → コンパイル時解決 or 変換不可警告
- PHP配列 (連想配列+数値配列混在) → Vec / HashMap の適切な選択
- グローバル変数 → Arc<RwLock<T>> or AppState注入

(d) 変換後の品質保証:
- cargo check → cargo test → 統合テスト の3段階検証
- AIによるテストコード自動生成 (ユニットテスト + 統合テスト)
- PHPプラグインのスクリーンショット比較 (変換前後のUI差分検出)
- E2Eテストシナリオの自動生成
- パフォーマンスベンチマーク (PHP版との速度比較)

(e) ライセンス検証:
- GPL / MIT / Apache / BSD等のOSSライセンスのみ変換許可
- 商用プラグイン (Elementor Pro, ACF Pro, Gravity Forms等) は変換拒否
- ライセンスヘッダーの自動検出
- 変換後のコードにオリジナルのライセンス情報を自動継承
- 商用プラグインに対しては rustpress-* 純正代替を提案
```

**Phase 6 完了基準:**
Rust (ネイティブ) と WASM の両方でプラグインを書いて、
フックを通じて動作を拡張できること。
主要プラグイン (SEO, フォーム) の最低1つがRustネイティブで動作すること。
WP関数互換レイヤーのTier 1が100%実装済みであること。
mu-pluginsとdrop-inの移行パスが確立されていること。

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

#### 7-3. admin-ajax.php 互換エンドポイント
```
課題:
- WPプラグインの大多数が admin-ajax.php 経由でAJAXリクエストを処理
- フロントエンド (wp_ajax_nopriv_*) とバックエンド (wp_ajax_*) の両方で使用
- このエンドポイントがないと、ほぼ全てのプラグインのフロント動的機能が停止

実装内容:
- POST /wp-admin/admin-ajax.php ディスパッチャ
- action パラメータでハンドラをルーティング
- フック経由でのハンドラ登録:
  → wp_ajax_{action} (ログインユーザー用)
  → wp_ajax_nopriv_{action} (非ログインユーザー用)
- nonceバリデーション互換 (check_ajax_referer)
- multipart/form-data (ファイルアップロード) 対応
- JSON / URLencoded / FormData 全フォーマット対応
- wp_send_json_success() / wp_send_json_error() 互換レスポンス
```

#### 7-4. admin-post.php 互換エンドポイント
```
課題:
- カスタムフォーム処理の標準パターン
- admin-ajax.phpのノンAJAX版

実装内容:
- POST /wp-admin/admin-post.php ディスパッチャ
- admin_post_{action} (ログインユーザー用) フック
- admin_post_nopriv_{action} (非ログインユーザー用) フック
- リダイレクトレスポンス対応
```

#### 7-5. Heartbeat API
```
課題:
- WP管理画面のリアルタイム機能の基盤
- 投稿ロック (他ユーザーが編集中の警告)、自動保存、通知の受信に使用
- 15-60秒間隔のポーリングで実装されている

実装内容:
- POST /wp-admin/admin-ajax.php?action=heartbeat 互換
- heartbeat_received / heartbeat_send フィルタ
- 投稿ロック機構:
  → 投稿編集開始時にロック取得
  → 他ユーザーが編集中の場合に警告表示
  → ロック解放 (ページ離脱 or タイムアウト)
- 自動保存 (autosave):
  → 60秒間隔で下書き自動保存
  → post_type='revision', post_name='{id}-autosave-v1' で保存
- ログインセッション有効性チェック (セッション切れ警告)
- 将来的にWebSocket/SSEへの置き換え検討 (パフォーマンス改善)
```

#### 7-6. カスタムREST APIエンドポイント登録
```
課題:
- プラグインが register_rest_route() でカスタム名前空間のAPIを追加
- /wp-json/myplugin/v1/data 等の独自エンドポイントが多数存在
- AI変換されたプラグインもカスタムエンドポイントを登録する必要がある

実装内容:
- register_rest_route() 互換API:
  → 名前空間 + ルート + メソッド + コールバック + 権限チェック
- パーミッションコールバック対応
- スキーマ定義 (引数のバリデーション)
- /wp-json/ ディスカバリにカスタムエンドポイントを自動登録
- WooCommerce REST API (/wc/v3/) の基盤としても使用
```

#### 7-7. WPGraphQL互換
```
課題:
- Headless WPの標準GraphQLレイヤーとして30万+サイトが使用
- Next.js (faust.js), Gatsby, Astro等のフレームワークがGraphQLで接続
- REST APIだけではHeadless WPサイトの完全移行ができない

実装内容:
- /graphql エンドポイント (async-graphql クレート)
- 標準スキーマ:
  → Posts, Pages, MediaItems, Users, Comments
  → Categories, Tags, カスタムタクソノミー
  → Menus, MenuItems
  → Settings
- カスタム投稿タイプの自動スキーマ登録
- カスタムフィールド (ACF, Meta Box等) のGraphQLフィールド化
- ミューテーション: 投稿作成/更新/削除、メディアアップロード
- 認証: Application Passwords, JWT対応
- ページネーション: Cursor-based (Relay仕様)
- Headless WPフレームワークとの接続テスト:
  → faust.js (WP Engine製)
  → gatsby-source-wordpress
  → astro-wordpress
```

#### 7-8. Headless運用モード
```
課題:
- WPをAPIバックエンドのみとして使うHeadless構成が急増
- フロントエンドはNext.js/Nuxt/Gatsby等で構築
- RustPressでもAPI専用モードが必要

実装内容:
- RUSTPRESS_HEADLESS=true でフロントエンドテンプレートレンダリング無効化
- API専用モード (REST + GraphQL のみ稼働)
- CORS設定:
  → 許可オリジン, メソッド, ヘッダーの管理画面設定
  → プリフライト (OPTIONS) の自動処理
- Webhooks:
  → コンテンツ変更時 (create/update/delete) の外部通知
  → 設定可能なペイロードフォーマット (WP Hook形式)
  → リトライ + 配信ログ
- プレビュー: 外部フロントエンドでの下書きプレビュー用トークン生成
```

**Phase 7 完了基準:**
Gutenbergエディタ等の既存WPクライアントがRustPressのAPIに接続して動作すること。
独自管理画面で基本的なサイト管理ができること。
admin-ajax.php互換エンドポイントでプラグインのAJAXリクエストが処理されること。
GraphQLエンドポイントでHeadless WPクライアントが接続可能であること。

---

### Phase 8: 本番運用レベル
> **目標: 実際のサイトで使える品質に仕上げる**

#### 8-1. パフォーマンス最適化
```
- オブジェクトキャッシュ (インメモリ + Redis対応)
- ページキャッシュ (静的HTML生成)
- クエリ最適化 (N+1問題の解消)
- gzip / brotli 圧縮
- Cache-Control / ETag / 304 Not Modified レスポンス
- CDNパージ連携 (CloudFlare, Bunny CDN, Varnish)
- Surrogate-Key ヘッダーによる選択的キャッシュ無効化
- HTTP/2 対応 + Link preload ヘッダー
```

#### 8-2. 運用機能
```
- コメント管理 + スパム対策 (Akismet相当のスパムフィルタ)
  → ネストされたコメントスレッド (reply-to)
  → コメントモデレーションワークフロー
  → Gravatar/アバター対応
- 検索機能:
  → MySQL FULLTEXT / Elasticsearch / Meilisearch 対応
  → ファセット検索 (カテゴリ, 日付, 著者でフィルタ)
  → 検索サジェスト / 自動補完
  → タイポ許容 (fuzzy matching)
  → 検索アナリティクス (何が検索されているか)
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

#### 8-5. バックアップ & リストア
```
課題:
- 本番サイトの移行にはバックアップが不可欠
- UpdraftPlus/BackWPup は300万+インストール

解決策:
- rustpress backup create → DB + メディア + 設定の完全バックアップ
- rustpress backup restore → ポイントインタイムリカバリ
- ストレージバックエンド: ローカル, S3, GCS, Azure Blob
- スケジュールバックアップ: 日次/週次/月次 (cron統合)
- 増分バックアップ (差分のみ転送)
- バックアップ検証 (リストアドライラン)
- 管理画面からワンクリックバックアップ/リストア
- ランサムウェア検知 (差分の異常検出)
```

#### 8-6. メール配信 (SMTP/トランザクショナルメール)
```
課題:
- パスワードリセット、注文確認、フォーム通知にメール配信は必須
- WooCommerceの注文フローが完全にメール依存

解決策:
- rustpress-mail モジュール拡充 (lettre クレート既存)
- SMTPプロバイダ設定UI: SendGrid, Mailgun, AWS SES, Postmark
- メールテンプレートエンジン (Tera統合)
- トランザクションメールキュー + リトライ
- バウンス/苦情ハンドリング (SESフィードバックループ)
- メール配信ログ (管理画面で送信履歴確認)
- ニュースレター連携: ConvertKit, Brevo, MailerLite API統合
```

#### 8-7. 二要素認証 (2FA/MFA)
```
課題:
- エンタープライズ/VIPサイトでは2FA必須
- Wordfence, iThemes Securityで1000万+サイトが使用

解決策:
- TOTP (Google Authenticator, Authy) 対応
- WebAuthn / FIDO2 (パスキー / ハードウェアキー)
- バックアップコード (一時利用コード)
- SMSベース2FA (Twilio連携)
- デバイストラスト (信頼済みデバイス記憶)
- ロール別2FA強制 (管理者は必須、投稿者は任意)
- リカバリーフロー (2FA無効化の管理者操作)
```

#### 8-8. OAuth/SAML/SSO 認証
```
課題:
- 企業サイトはActive Directory/Okta/Azure ADでログインする
- 500万+のエンタープライズWPサイトがSSO依存

解決策:
- OAuth 2.0 プロバイダ: Google, GitHub, Microsoft, Apple
- SAML 2.0: Active Directory, Okta, Ping Identity, OneLogin
- OpenID Connect: 汎用OIDC対応
- ソーシャルログイン: Twitter, Facebook, LinkedIn
- JWK (JSON Web Key) プロバイダ (クライアントアプリ用)
- ロールマッピング: IdPグループ → RustPressロール自動割当
- rustpress-auth クレートに統合
```

#### 8-9. 監視 & オブザーバビリティ
```
課題:
- 本番運用にはAPM (Application Performance Monitoring) が不可欠
- 障害発生時のトラブルシューティングにログ集約が必要

解決策:
- OpenTelemetry 統合 (トレース, メトリクス, ログ)
  → Axum middleware で自動計装
  → リクエスト/レスポンス時間, DB クエリ時間, エラー率
- エラートラッキング: Sentry SDK統合
- ログ集約: 構造化ログ (JSON) → ELK, CloudWatch, Datadog
- ヘルスチェック: /health, /ready エンドポイント (LB対応)
- スロークエリログ (閾値設定可能)
- 管理画面ダッシュボード: リクエスト数, エラー率, レスポンスタイム
- アップタイム監視: 外部監視サービス連携 (UptimeRobot, Pingdom)
```

#### 8-10. 高可用性 & スケーリング
```
課題:
- エンタープライズはダウンタイムゼロを要求
- Fortune 500 WordPress サイトはクラスタ構成

解決策:
- セッション共有: Redis / DB バックエンドでステートレス化
- DBレプリケーション: 読み取りレプリカ対応 (SeaORM接続プール分離)
- キャッシュ無効化: Redis Pub/Sub でクラスタ間同期
- ロードバランサー対応: X-Forwarded-For, X-Real-IP ヘッダー処理
- ゼロダウンタイムデプロイ: ローリングアップデート対応
- 水平スケーリングガイド: 複数RustPressインスタンス構成
- データベース接続プーリング戦略 (PgBouncer相当)
```

#### 8-11. Action Scheduler互換
```
課題:
- WooCommerceのバックグラウンド処理基盤 (500万+サイトが依存)
- 定期支払い処理、メール送信、在庫更新、Webhook配信等に使用
- wp-cronとは別の高機能ジョブキューシステム
- Action Schedulerなしでは WooCommerce Subscriptions が動作しない

実装内容:
- バックグラウンドジョブキュー (Tokioタスク + DBバックド永続化)
- ジョブ状態管理: pending → in-progress → complete / failed
- スケジュール種別:
  → 即時実行 (async)
  → 単発予約 (schedule_single_action)
  → 定期実行 (schedule_recurring_action)
  → Cron式 (schedule_cron_action)
- リトライ機構: 失敗時の自動リトライ (最大試行回数設定可)
- 並行実行制御: 同時実行数の上限設定
- 管理画面: ジョブ一覧, 状態フィルタ, 手動実行, ログ表示
- Action Scheduler互換テーブル (actionscheduler_actions, _claims, _groups, _logs)
- 移行時: 既存のスケジュール済みアクションの自動移行
```

#### 8-12. wp-config.php 定数マッピング
```
課題:
- WPサイトの設定は wp-config.php の定数で定義される
- RustPressへの移行時に全定数を適切に変換する必要がある
- 未変換の定数があるとサイトの動作が変わる

実装内容:
- rustpress migrate config でwp-config.php → rustpress.toml 自動変換
- 定数マッピング表:
  [database]
  → DB_NAME, DB_USER, DB_PASSWORD, DB_HOST, DB_CHARSET, DB_COLLATE
  → $table_prefix

  [server]
  → WP_SITEURL, WP_HOME → base_url
  → FORCE_SSL_ADMIN, FORCE_SSL_LOGIN → tls.force = true
  → WP_HTTP_BLOCK_EXTERNAL → http.block_external = true

  [debug]
  → WP_DEBUG → debug.enabled
  → WP_DEBUG_LOG → debug.log_file
  → WP_DEBUG_DISPLAY → debug.display_errors
  → SCRIPT_DEBUG → debug.script_debug
  → SAVEQUERIES → debug.log_queries

  [security]
  → DISALLOW_FILE_EDIT → security.disallow_file_edit
  → DISALLOW_UNFILTERED_HTML → security.disallow_unfiltered_html
  → AUTH_KEY, SECURE_AUTH_KEY等 → security.secret_keys (自動生成推奨)

  [content]
  → WP_POST_REVISIONS → content.max_revisions
  → AUTOSAVE_INTERVAL → content.autosave_interval
  → EMPTY_TRASH_DAYS → content.trash_retention_days
  → WP_DEFAULT_THEME → themes.default

  [performance]
  → WP_MEMORY_LIMIT → 不要 (Rust管理、ログに記録のみ)
  → WP_MAX_MEMORY_LIMIT → 不要
  → WP_CACHE → cache.enabled
  → WP_CRON_LOCK_TIMEOUT → cron.lock_timeout

  [uploads]
  → UPLOADS → uploads.directory
  → WP_CONTENT_DIR → content.directory
  → COOKIE_DOMAIN → auth.cookie_domain

- 未対応定数の警告表示 (移行レポートに含める)
- カスタム定数 (プラグイン固有) の検出と移行アドバイス
```

#### 8-13. カスタムCron完全互換
```
課題:
- wp_schedule_event() で独自間隔・コールバックを登録するプラグインが大多数
- cron_schedules フィルタでカスタム間隔を追加するパターンも一般的
- Phase 8-2のcronスケジューラがWPのcron APIと互換でないとプラグインが動作しない

実装内容:
- wp_schedule_event() / wp_schedule_single_event() 互換API
- wp_unschedule_event() / wp_clear_scheduled_hook() 互換
- wp_next_scheduled() / wp_get_schedule() 互換
- cron_schedules フィルタ互換 (カスタム間隔の登録)
- 標準間隔: hourly, twicedaily, daily, weekly
- wp_options の 'cron' エントリからの自動読み込み:
  → 既存WPサイトのスケジュール済みイベントを自動移行
  → PHPシリアライズ形式のcronデータをデシリアライズ
- 仮想cronモード: リクエスト駆動 (WP互換) と真のcron (Tokio interval) の選択
```

#### 8-14. サイトヘルス
```
課題:
- WP 5.2+の標準機能 (/wp-admin/site-health.php)
- サーバー環境の診断、問題の検出、推奨事項の表示
- 管理者が本番サイトの状態を把握するための基本ツール

実装内容:
- /wp-admin/site-health 相当の診断画面
- テスト項目:
  → DB接続状態 / レスポンスタイム
  → ディスク容量 (メディアストレージ)
  → HTTPS有効性
  → PHP→Rust移行完了率 (プラグイン変換状況)
  → プラグイン互換性チェック (未変換プラグインの警告)
  → テーマ互換性チェック
  → キャッシュ動作状態
  → Cron実行状態 (最終実行時刻, 失敗ジョブ数)
  → REST API利用可能性
  → メール送信テスト
  → セキュリティヘッダー (CSP, HSTS等)
  → TLS証明書の有効期限
- ステータス: 良好 (緑) / 改善推奨 (橙) / 要対応 (赤)
- REST API: /wp-json/wp-site-health/v1/tests 互換
```

**Phase 8 完了基準:**
本番環境でRustPressを安全に運用できること。
バックアップ/リストア、メール配信、2FA/SSO、監視が動作すること。
Action Schedulerでバックグラウンドジョブが処理されること。
wp-config.phpの全定数がrustpress.tomlに変換可能であること。

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

#### 9-7. マルチDB対応 (PostgreSQL / SQLite)
```
課題:
- Heroku, Railway, Render 等はPostgreSQL専用
- エッジコンピューティング (Cloudflare Workers) ではSQLiteが主流
- MySQL限定では30-40%のホスティング環境を逃す

解決策:
- SeaORM は PostgreSQL/SQLite を既にサポート → 接続設定の追加で対応可能
- DB抽象化レイヤー:
  → 接続文字列でDB種別を自動検出 (mysql://, postgres://, sqlite://)
  → WP DBスキーマの各DB方言への変換
  → PHPシリアライズ形式はDB非依存 → そのまま動作
- WP → PostgreSQL マイグレーションツール:
  → rustpress migrate db-convert でMySQL→PG変換
  → テーブル定義, データ型, AUTO_INCREMENT→SERIAL 変換
- SQLite モード:
  → 開発環境/シングルユーザー向け軽量モード
  → ファイル1つで完結 (rustpress.db)
```

#### 9-8. カスタマイザー (ライブテーマ編集)
```
課題:
- WPユーザーの40%がカスタマイザーでテーマ調整
- 非エンジニアがコードなしで色/フォント/レイアウトを変更

解決策:
- /wp-admin/customize.php 相当のライブプレビューUI
- REST API経由でテーマ設定のCRUD
- 対応項目:
  → サイトアイデンティティ (サイト名, ロゴ, ファビコン)
  → 色設定 (プライマリ/セカンダリ/背景色)
  → フォント選択 (Google Fonts連携)
  → ヘッダー/フッター画像
  → カスタムCSS エディタ
  → ウィジェットエリア設定
  → メニュー管理 (ドラッグ&ドロップ)
- 変更のライブプレビュー (iframe + postMessage)
- 変更のスケジュール公開 (変更セットを予約)
```

#### 9-9. ショートコード完全対応
```
課題:
- WPコンテンツの80%+がショートコードを含む
- [gallery], [embed], [caption] 等の標準ショートコードが動かないとコンテンツ崩壊

解決策:
- 標準ショートコード完全実装:
  → [gallery] : メディアライブラリからギャラリー生成
  → [embed] : oEmbed対応 (YouTube, Twitter, Vimeo, etc.)
  → [caption] : 画像キャプション
  → [audio] : HTML5オーディオプレイヤー
  → [video] : HTML5ビデオプレイヤー
  → [playlist] : オーディオ/ビデオプレイリスト
- oEmbedプロバイダ登録:
  → YouTube, Vimeo, Twitter, Instagram, TikTok, Spotify, etc.
  → 自動ディスカバリー (oEmbed endpoint検出)
- カスタムショートコード登録API:
  → プラグインからショートコード登録可能
  → ネストされたショートコード対応
  → ショートコード内でのショートコード実行
- ショートコードキャッシュ (外部API呼び出しの結果をキャッシュ)
```

#### 9-10. ACF (Advanced Custom Fields) 完全互換
```
課題:
- プロフェッショナルWPサイトの40%がACFを使用
- ACFフィールド定義のインポートができないと移行不可

解決策:
- rustpress-fields をACF互換に拡張:
  → ACF JSON エクスポートファイルのインポート (.json)
  → ACF PHP export のパース
- 全フィールドタイプ対応:
  → 基本: text, textarea, number, email, url, password
  → 選択: select, checkbox, radio, button group, true/false
  → コンテンツ: wysiwyg, image, file, gallery, oEmbed
  → 関連: relationship, post_object, page_link, taxonomy, user
  → レイアウト: group, repeater, flexible_content
  → 特殊: date_picker, time_picker, color_picker, google_map
- ACF REST API互換:
  → /wp-json/acf/v3/ エンドポイント
  → フィールドグループのCRUD
- 条件分岐ロジック (フィールドAの値でフィールドBの表示/非表示)
- フィールドグループのエクスポート (JSON形式)
```

#### 9-11. 高度なメディア管理
```
課題:
- コンテンツチームはメディア編集をWP管理画面内で完結させる
- WebP/AVIF変換はSEO/パフォーマンスに直結

解決策:
- 画像編集API:
  → クロップ / 回転 / 反転 (image クレート)
  → 明度 / コントラスト調整
  → リサイズ (サムネイル自動生成)
- フォーマット変換:
  → WebP / AVIF 自動変換 (アップロード時)
  → レスポンシブ画像 (srcset 自動生成)
  → 遅延読み込み (loading="lazy" 自動付与)
- PDFプレビュー: 1ページ目をサムネイルとして生成
- 動画サムネイル: FFmpeg連携でフレーム抽出
- 画像最適化: 品質維持しつつファイルサイズ削減
- メディアライブラリ: フォルダ管理, タグ付け, 一括操作
```

#### 9-12. GDPR準拠 & プライバシーツール
```
課題:
- EU企業はGDPR準拠なしにCMSを使えない
- 個人データエクスポート/削除は法的義務

解決策:
- 個人データエクスポート (GDPR Article 20):
  → ユーザーの全データをJSON/CSVでダウンロード
  → 投稿, コメント, メタデータ, メディア一括出力
- データ削除 (Right to be Forgotten):
  → ユーザーデータの完全削除ワークフロー
  → 匿名化オプション (コメント著者名を「匿名」に)
  → 削除確認と監査ログ
- 同意管理:
  → Cookie同意バナー (カスタマイズ可能)
  → トラッキング同意 (Google Analytics, Facebook Pixel等)
  → 同意記録の保存と監査
- プライバシーポリシー:
  → テンプレート生成ツール
  → 各プラグインのデータ収集項目の自動集約
- DPA (Data Processing Agreement) テンプレート
```

#### 9-13. コンテンツインポート/エクスポート (WXR)
```
課題:
- WordPress eXtended RSS (WXR) はWP間のデータ移行標準
- 他のWPサイトからの移行にはWXRインポートが必須

解決策:
- WXRエクスポート:
  → 全コンテンツ (投稿, 固定ページ, CPT) のXMLエクスポート
  → メディア添付ファイルのURL参照
  → タクソノミー, メタデータ, コメント含む
  → 著者情報, カテゴリ階層の保持
- WXRインポート:
  → XMLパース (quick-xml)
  → メディアファイルの自動ダウンロード
  → 著者マッピング (既存ユーザーへの紐付け)
  → コメントスレッドの再構築
  → カスタムフィールド/メタデータの移行
  → 重複検出 (既存コンテンツとの衝突回避)
- CLI: rustpress export / rustpress import
```

#### 9-14. カスタムGutenbergブロック移行
```
課題:
- プラグイン/テーマが register_block_type() でカスタムブロックを登録
- カスタムブロックはPHPのrender_callbackでサーバーサイドHTML生成を行う
- 標準ブロック (9-2) だけでは、カスタムブロックを含むコンテンツが崩壊する

実装内容:
- block.json 定義ファイルの解析 (ブロック名, 属性, スタイル)
- カスタムブロックの検出:
  → wp_posts.post_content 内の <!-- wp:namespace/block-name --> を走査
  → 未知のブロック名を「カスタムブロック」としてリストアップ
- サーバーサイドレンダリング:
  → PHP render_callback → Rustレンダラーへの変換 (AI変換サービス連携)
  → 変換不可能なブロック: フォールバックHTML (保存済みHTML) をそのまま出力
- クライアントサイド (エディタ側):
  → React製エディタコンポーネントはそのまま使用 (npm パッケージ)
  → RustPress REST API 経由でデータ通信
- カスタムブロックカテゴリの登録
- ダイナミックブロック vs 静的ブロックの判別と適切な処理
```

#### 9-15. ブロックパターン / ブロックスタイル
```
課題:
- register_block_pattern() でテーマ/プラグインが定義するレイアウトパターン
- register_block_style() でブロックの見た目バリエーションを追加
- WP 6.0+ではパターンがテーマの中心的な機能

実装内容:
- ブロックパターン:
  → register_block_pattern() 互換API
  → パターンカテゴリの登録 (register_block_pattern_category)
  → コアパターン (WP同梱) の網羅
  → テーマ内 /patterns/ ディレクトリからの自動読み込み
  → パターンディレクトリ (wordpress.org/patterns/) との連携
- ブロックスタイル:
  → register_block_style() 互換API
  → スタイルバリエーション (例: ボタンのfill/outline)
  → CSSクラスの自動付与 (is-style-{name})
```

#### 9-16. カスタムポストステータス
```
課題:
- register_post_status() で独自ステータスを登録するプラグインが多数
- WooCommerce: wc-pending, wc-processing, wc-on-hold, wc-completed, wc-cancelled, wc-refunded, wc-failed
- 非標準ステータスの投稿がフィルタ/表示されないと業務が停止

実装内容:
- register_post_status() 互換API
- カスタムステータスの属性:
  → public, private, protected, internal
  → label, label_count (翻訳対応)
  → show_in_admin_all_list, show_in_admin_status_list
- WooCommerce標準ステータスのプリセット登録
- 管理画面での投稿リストフィルタ対応
- REST APIでのカスタムステータス対応
- 移行時: wp_posts.post_status の非標準値を自動検出し、対応するステータスを自動登録
```

#### 9-17. カスタムリライトルール
```
課題:
- add_rewrite_rule() / add_rewrite_tag() でプラグインが独自URLパターンを登録
- 例: /products/{slug}/, /events/{year}/{month}/ 等のカスタムURL構造
- カスタムリライトが動かないと、カスタム投稿タイプのURL体系が壊れる

実装内容:
- add_rewrite_rule() 互換API → Axumルーターへの動的ルート追加
- add_rewrite_tag() 互換API → URLパラメータ抽出
- rewrite_rules オプション (wp_options) からの自動読み込み
- WPリライトルール (正規表現ベース) → Axumルートパターンへの変換
- flush_rewrite_rules() 互換 (キャッシュクリア + 再構築)
- .htaccess のリライトルールパース → RustPress設定への変換
- パーマリンク構造の完全互換:
  → /%postname%/, /%category%/%postname%/, /%year%/%monthnum%/%day%/%postname%/
  → カスタム投稿タイプのパーマリンク構造 (rewrite 引数)
```

#### 9-18. 非ACFカスタムフィールドプラグイン互換
```
課題:
- ACF以外にもカスタムフィールドプラグインが多数存在
- Meta Box (100万+), Pods (10万+), JetEngine, Carbon Fields
- 各プラグインが独自のメタデータ保存形式を持つ

実装内容:
- Meta Box互換:
  → rwmb_meta() 関数互換
  → Meta Box グループ/クローンフィールドのデータ構造
  → mb_* メタキープレフィックスの解析
- Pods互換:
  → pods() 関数互換
  → Pods独自テーブル (_pods, _podsrel) の読み取り
  → 拡張投稿タイプ/タクソノミーの移行
- JetEngine互換:
  → jet_engine()->listings のデータ構造
  → JetEngine meta_query 形式
- Carbon Fields互換:
  → carbon_get_* 関数互換
  → _carbon_* メタキープレフィックスの解析
- 統一インターフェース:
  → 全プラグインのメタデータ → rustpress-fields の統一フォーマットへ変換
  → 移行ツール: rustpress migrate custom-fields --source=metabox|pods|jetengine|carbon
```

#### 9-19. 多言語コンテンツプラグイン互換
```
課題:
- Phase 9-1のi18nは UI翻訳 (.moファイル) のみ対応
- コンテンツの多言語管理 (WPML, Polylang) は完全に別の仕組み
- 多言語ECサイト/企業サイトの移行にはコンテンツ多言語対応が必須

実装内容:
- WPML互換:
  → icl_translations テーブルの読み取り
  → 言語ごとの投稿紐付け (trid, language_code, source_language_code)
  → 言語切替UI (言語セレクタ)
  → hreflang タグの自動生成
  → 文字列翻訳 (icl_strings テーブル)
  → WooCommerce多通貨対応 (WCML)
- Polylang互換:
  → pll_languages タクソノミーの読み取り
  → 投稿⇔翻訳の紐付け (pll_translations_* メタ)
  → 言語別カテゴリ/タグの管理
  → Polylang Pro の翻訳管理ワークフロー
- TranslatePress互換:
  → 翻訳データ (tp_translation テーブル) の読み取り
  → フロントエンド翻訳エディタの代替UI
- 移行時: rustpress migrate i18n --source=wpml|polylang|translatepress
- RustPressネイティブの多言語コンテンツ管理機能 (rustpress-i18n 拡張)
```

#### 9-20. リビジョン管理完全実装
```
課題:
- 自動保存 (autosave) とリビジョン比較はWP管理画面の基本機能
- WP_POST_REVISIONS 定数によるリビジョン数制限
- リビジョン復元はコンテンツ事故からの復旧に不可欠

実装内容:
- リビジョン保存:
  → 投稿更新時に自動リビジョン作成 (post_type='revision')
  → WP_POST_REVISIONS 設定に基づくリビジョン数制限 (デフォルト: 無制限)
  → 古いリビジョンの自動パージ (設定値超過分)
- 自動保存 (autosave):
  → AUTOSAVE_INTERVAL (デフォルト60秒) 間隔
  → ユーザーごとに1つの自動保存を保持
  → Heartbeat API連携
- リビジョン比較UI:
  → 2つのリビジョン間のdiff表示 (行単位, 単語単位)
  → スライダーUIで任意のリビジョンを選択
  → タイトル/コンテンツ/抜粋それぞれの差分表示
- リビジョン復元:
  → 任意のリビジョンに戻す (wp_restore_post_revision 互換)
  → 復元前の確認画面
```

#### 9-21. コメントタイプ完全対応
```
課題:
- comment_type フィールドで区別される複数のコメント種別
- WooCommerce商品レビュー (comment_type='review') は星評価を含む
- Pingback/Trackback は外部サイトからの通知プロトコル

実装内容:
- コメントタイプ別処理:
  → '' (通常コメント): 標準表示
  → 'pingback': Pingbackプロトコル処理 + 表示
  → 'trackback': Trackbackプロトコル処理 + 表示
  → 'review': 星評価メタ (rating) の表示、平均評価計算
  → カスタムコメントタイプ: プラグインから登録可能
- WooCommerce商品レビュー:
  → 星評価 (1-5) のレンダリング
  → 評価の集計 (平均, 件数分布)
  → verified owner バッジ
  → Schema.org Review構造化データ
- Pingback/Trackback送受信:
  → 受信: xmlrpc.php 経由 (Phase 9-4で対応)
  → 送信: 投稿公開時にリンク先へPingback送信
  → スパムフィルタ連携
```

#### 9-22. テーマカスタマイザー設定移行
```
課題:
- 既存WPサイトのカスタマイザー設定 (theme_mods_{theme_name}) がRustPressテーマに移行されない
- ユーザーが管理画面から変更した色/フォント/ロゴ等の設定が失われる

実装内容:
- theme_mods_{theme_name} オプションの読み取り
- 標準設定の自動マッピング:
  → custom_logo → サイトロゴ設定
  → header_image → ヘッダー画像
  → background_color → 背景色
  → nav_menu_locations → メニュー配置
  → sidebars_widgets → ウィジェット配置
- テーマ固有設定:
  → WPテーマ → RustPressテーマ間のマッピング定義ファイル
  → 未マッピング設定の警告とCSS上書きの提案
- 移行ツール: rustpress migrate theme-settings --from=theme_name
```

**Phase 9 完了基準:**
多言語サイト、Gutenbergコンテンツ、マルチサイト構成のWordPressが
RustPressで正常に動作すること。WP 5.0以降のサイトが移行可能であること。
PostgreSQL/SQLiteでも動作すること。ACFフィールド定義がインポート可能であること。
GDPR準拠ツールが動作し、EU企業の要件を満たすこと。
カスタムブロック、カスタムポストステータス、カスタムリライトルールが動作すること。
WPML/Polylang の多言語コンテンツが移行可能であること。
非ACFカスタムフィールドプラグイン (Meta Box, Pods等) からの移行パスがあること。

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

#### 10-10. Webインストーラー
```
課題:
- WordPressの5分インストールが普及の最大要因
- CLIに不慣れなユーザーはブラウザからセットアップしたい

解決策:
- /install エンドポイント (初回アクセス時のみ有効)
- ブラウザベースのセットアップウィザード:
  → 言語選択
  → DB接続設定 + 接続テスト
  → サイト情報 (タイトル, 管理者メール)
  → 管理者アカウント作成
  → テーマ選択
  → サンプルデータ投入 (オプション)
- 要件チェッカー: Rustバージョン, MySQL/PG接続, ディスク容量
- インストール完了後に /install を自動無効化
```

#### 10-11. 開発者デバッグツール
```
課題:
- Query Monitor (WP) は開発者の標準ツール
- デバッグなしでは開発者コミュニティが育たない

解決策:
- デバッグバー (管理画面下部):
  → DBクエリ数 / 実行時間 / スロークエリ一覧
  → フック発火順序 (どのフックがいつ呼ばれたか)
  → メモリ使用量 / ピーク
  → テンプレート解決パス (どのテンプレートが選択されたか)
  → リクエスト/レスポンス情報
  → キャッシュヒット率
- 開発モード: RUSTPRESS_DEBUG=true
  → 詳細エラー表示 (バックトレース)
  → テンプレート自動リロード (ファイル変更検知)
  → SQLクエリログ
  → フック実行トレース
- プロファイリング:
  → flamegraph生成
  → リクエスト単位のパフォーマンス分析
```

#### 10-12. コンテナオーケストレーション
```
課題:
- エンタープライズはKubernetes上でCMSを運用する
- Docker Compose は開発環境のみ、本番はK8s

解決策:
- Kubernetes マニフェスト:
  → Deployment (RustPress + サイドカー)
  → Service + Ingress
  → ConfigMap (設定) + Secret (DB認証情報)
  → PersistentVolumeClaim (メディアストレージ)
  → HorizontalPodAutoscaler (自動スケーリング)
- Helm Chart: helm install rustpress rustpress/rustpress
- Docker イメージ最適化:
  → マルチステージビルド (ビルド → scratch/distroless)
  → バイナリ + テンプレート + 静的ファイルのみ (50MB以下)
- Compose for production:
  → RustPress + MySQL/PG + Redis + Nginx リバースプロキシ
  → TLS自動設定 (Let's Encrypt)
```

#### 10-13. ホワイトラベル & エージェンシー機能
```
課題:
- WordPress代理店 (WP Engine, Kinsta モデル) がRustPressを採用するには
  クライアント管理機能が必要
- ホワイトラベルなしでは代理店ビジネスが成立しない

解決策:
- ホワイトラベル:
  → 管理画面ロゴ/ブランド名のカスタマイズ
  → ログインページのカスタムデザイン
  → フッター/ヘッダーのブランディング
  → 管理画面カラースキーム (クライアント別)
- クライアント管理:
  → マルチテナント対応 (1インスタンスで複数クライアント)
  → サブアカウント作成 (クライアント別管理者)
  → 使用量/ストレージクォータ
  → 請求統合 (Stripe Billing)
- ステージング環境:
  → ワンクリックでステージング作成 (DB + ファイルコピー)
  → ステージング → 本番のプロモーション
  → コンテンツステージング (下書き→公開ワークフロー)
  → 変更のdiff/マージツール
```

#### 10-14. アナリティクス統合
```
課題:
- MonsterInsights (300万+サイト) が標準のアナリティクス
- サイト分析なしではマーケティングチームが使えない

解決策:
- Google Analytics 4 ネイティブ統合 (測定ID設定のみ)
- Matomo (自己ホスト型) 統合
- 内蔵アナリティクス (Jetpack Stats相当):
  → ページビュー, ユニークビジター, リファラー
  → 人気ページランキング
  → 検索キーワード (Google Search Console連携)
  → 地域/デバイス/ブラウザ分布
- コンバージョントラッキング:
  → フォーム送信, ECカート追加, 決済完了
  → カスタムイベント (フック経由)
- ダッシュボードウィジェット: 管理画面でサマリー表示
```

#### 10-15. SEOプラグインデータ移行
```
課題:
- Yoast/RankMath/AIOSEOの設定はpostmetaに保存されている
- SEOデータが移行されないと、title/descriptionが空になりSEO順位が下落
- 移行後にSEO設定を手動で再入力するのは非現実的 (数千ページ)

実装内容:
- rustpress migrate seo-data コマンド
- Yoast SEO:
  → _yoast_wpseo_title → rustpress-seo タイトル
  → _yoast_wpseo_metadesc → rustpress-seo メタディスクリプション
  → _yoast_wpseo_focuskw → フォーカスキーワード
  → _yoast_wpseo_canonical → canonical URL
  → _yoast_wpseo_opengraph-* → OGP設定
  → wpseo_taxonomy_meta → タクソノミーSEO設定
- RankMath:
  → rank_math_title, rank_math_description
  → rank_math_focus_keyword
  → rank_math_canonical_url
  → rank_math_og_*, rank_math_twitter_*
  → rank_math_schema_* → 構造化データ
- All in One SEO:
  → _aioseo_title, _aioseo_description
  → _aioseo_og_*, _aioseo_twitter_*
  → aioseo_posts テーブルのデータ
- SEO設定 (サイト全体):
  → タイトル区切り文字, 会社/個人情報, ソーシャルプロフィール
  → robots.txt カスタムルール
  → リダイレクト一覧 (Yoast Premium, RankMath)
- 自動マッピング: 検出されたSEOプラグインに応じて適切な変換を実行
```

#### 10-16. メール設定移行
```
課題:
- WP Mail SMTP / Post SMTP / FluentSMTP で設定されたSMTP設定の移行
- メール設定が移行されないと、パスワードリセット/注文通知等が全て停止

実装内容:
- rustpress migrate mail コマンド
- 設定ソースの自動検出:
  → WP Mail SMTP: wp_options の wp_mail_smtp オプション
  → Post SMTP: wp_options の postman_options
  → FluentSMTP: wp_options の fluentmail-settings
  → wp-config.php のSMTP定数
- 移行先: rustpress.toml [mail] セクション
  → provider: sendgrid | ses | mailgun | smtp
  → host, port, username, password (暗号化保存)
  → from_name, from_email
  → encryption: tls | starttls | none
- APIキーの安全な移行 (暗号化ストレージ)
- 移行後のテストメール送信機能
- WooCommerceメールテンプレートの移行:
  → 注文確認, 発送通知, パスワードリセット等のテンプレート
  → カスタマイズ済みHTML/CSSの保持
```

#### 10-17. メディアCDNオフロード対応
```
課題:
- WP Offload Media / EWWW等で画像URLがCDN URLに書き換えられている
- メディアのURLパスが /wp-content/uploads/ ではなく CDNドメインになっている
- URL解決を誤るとサイト内の全画像が404になる

実装内容:
- CDNオフロードの自動検出:
  → wp_options から as3cf_*, ewwwio_* 等の設定を検出
  → postmeta の amazonS3_info 等の解析
- URL解決戦略:
  → ローカルパス (/wp-content/uploads/) → そのまま配信
  → S3 URL (s3.amazonaws.com) → S3プロキシまたはリダイレクト
  → CDN URL (cdn.example.com) → CDN設定の引き継ぎ
- rustpress.toml [media] セクション:
  → storage: local | s3 | gcs | azure
  → cdn_url: CDNプレフィックス
  → bucket, region, access_key, secret_key
- メディアURL変換: DB内のCDN URLを新しい設定に一括更新
- 既存CDN設定の継続使用オプション (設定変更なしで移行)
```

#### 10-18. DNS / ドメイン移行手順
```
課題:
- ドメインのDNS切り替えは移行の最終ステップで最もリスクが高い
- TTLの設定ミスでダウンタイムが発生
- SSL証明書の発行タイミングも調整が必要

実装内容:
- rustpress migrate dns-check コマンド:
  → 現在のDNSレコード (A, CNAME, MX, TXT) の取得と表示
  → TTL確認 (高TTLの場合は事前に下げるよう提案)
  → MXレコードの保持確認 (メール配信への影響防止)
- ゼロダウンタイム切り替え手順:
  1. ステージング環境でRustPressを構築・検証
  2. 移行前24時間: TTLを300秒に変更
  3. DNS切り替え: Aレコードを新サーバーIPに変更
  4. SSL証明書: Let's Encrypt自動発行 (HTTP-01チャレンジ)
  5. 旧サーバーを1週間維持 (フォールバック)
- 段階移行モード (Phase 10-5) との連携:
  → リバースプロキシで一部URLのみRustPressに転送
  → DNS変更なしで段階的に移行可能
- 有料SSL証明書 (EV証明書等) の手動移行手順ドキュメント
```

#### 10-19. 管理画面カスタマイズの移行
```
課題:
- プラグインが add_menu_page / add_meta_box で管理画面を拡張
- カスタムダッシュボードウィジェットが業務フローの一部になっているサイト
- 管理画面のカスタマイズが移行されないと運用チームが混乱

実装内容:
- add_menu_page / add_submenu_page 互換API:
  → プラグインから管理画面メニューを追加
  → アイコン (Dashicons互換), 表示順序, 権限チェック
- add_meta_box 互換API:
  → 投稿編集画面にカスタムメタボックスを追加
  → コンテキスト (normal, side, advanced), 優先度
  → メタボックスのコールバック → Rustクロージャ or WASMプラグイン
- add_dashboard_widget 互換API:
  → ダッシュボードにカスタムウィジェットを追加
- カスタム管理ページ:
  → 設定API (register_setting, add_settings_section, add_settings_field) 互換
  → 管理画面通知 (admin_notices フック) 互換
- 移行時: 検出された管理画面カスタマイズのリストと対応状況レポート
```

#### 10-20. WooCommerce外部連携API
```
課題:
- WooCommerce Webhooksで外部サービス (Zapier, ShipStation等) と連携しているストア
- WooCommerce Admin API / Analytics APIに依存する管理ツール
- 外部連携が壊れるとECサイトの物流・会計が停止

実装内容:
- WooCommerce Webhooks互換:
  → Webhook登録 (topic, delivery_url, secret)
  → 標準トピック: order.created, order.updated, product.created, customer.created 等
  → ペイロードフォーマット: WooCommerce API v3形式
  → 配信ログ, リトライ (最大5回, 指数バックオフ)
- WooCommerce Admin API:
  → /wp-json/wc-admin/ 互換エンドポイント
  → レポート: 売上, 注文, 商品, カテゴリ, 顧客
  → ダッシュボードウィジェットデータ
- WooCommerce Analytics API:
  → /wp-json/wc-analytics/ 互換エンドポイント
  → 期間比較, フィルタ, CSVエクスポート
- 外部サービス連携テスト:
  → Zapier: Webhook受信テスト
  → ShipStation: 注文同期テスト
  → 会計ソフト (Xero, QuickBooks): 請求書同期テスト
```

#### 10-21. 共有ホスティング対策
```
課題:
- WPサイトの60%以上が共有ホスティング (PHP専用環境) で運用
- Rustバイナリを動かせない環境のユーザーが最大の移行障壁
- 「RustPress Cloud」だけでは移行先の選択肢が狭い

実装内容:
A. RustPress Lite (共有ホスティング互換):
  → musl静的リンクバイナリのCGI/FastCGIモード
  → cPanel Addon として動作するインストーラー
  → 共有ホスティングの制約 (ポート制限, プロセス数制限) への対応

B. ホスティングパートナーシップ:
  → 日本: エックスサーバー, さくら, ロリポップ, ConoHa
  → 海外: Bluehost, SiteGround, HostGator, DreamHost
  → RustPress対応プランの共同開発 (Rust実行環境を提供)

C. マイグレーションパスフローチャート:
  → 共有ホスティング → RustPress Cloud (推奨, ワンクリック移行)
  → 共有ホスティング → VPS移行ガイド (DigitalOcean, Linode, Vultr)
  → 共有ホスティング → RustPress Lite (同一環境で移行)
  → 共有ホスティング → マネージドRustPressホスティング (パートナー)

D. 移行コスト試算ツール:
  → 現在のホスティング費用 vs RustPress移行後の費用比較
  → パフォーマンス改善の定量予測
```

**Phase 10 完了基準:**
任意のWordPressサイトに対して `rustpress migrate analyze` を実行し、
移行パスが明確に提示されること。
RustPress Cloudで新規サイトを30秒以内に立ち上げられること。
SEO安全移行が検証済みで、移行前後で検索順位に影響がないこと。
自動更新機構により、一般ユーザーが管理画面からワンクリックで更新可能であること。
Webインストーラーから非エンジニアでも5分以内にセットアップ完了できること。
SEOプラグイン/メール設定/CDN設定が自動移行されること。
共有ホスティングユーザーへの移行パスが確立されていること。

---

### Phase 11: 垂直市場プラグイン & エンタープライズ
> **目標: 特定業種のWordPressサイトも含め、全ユーザーの移行経路を確立する**

#### 11-1. コミュニティ/フォーラムプラグイン
```
課題:
- bbPress (200万+サイト) / BuddyPress でコミュニティ構築しているサイト
- フォーラム/SNS機能なしでは移行不可

解決策:
- rustpress-community クレート (または別リポ):
  → フォーラム構造: カテゴリ → トピック → リプライ
  → モデレーション: 投稿承認, 報告, BAN
  → ユーザープロフィール & メンバーディレクトリ
  → プライベートメッセージ
  → ソーシャル機能: フォロー, いいね
  → ゲーミフィケーション: バッジ, ポイント, ランキング
  → 通知システム (メール + 管理画面内)
```

#### 11-2. LMS (学習管理システム) プラグイン
```
課題:
- LearnDash, LifterLMS で50万+の教育サイトが運用中
- コース/レッスン/クイズの構造が独自

解決策:
- rustpress-lms クレート (または別リポ):
  → コース構造: コース → セクション → レッスン → トピック
  → クイズエンジン: 選択式, 記述式, ドラッグ&ドロップ
  → 進捗トラッキング (ユーザー別完了率)
  → 修了証生成 (PDF)
  → ドリップコンテンツ (日数ベースのコンテンツ公開)
  → インストラクターダッシュボード
  → 成績管理 & レポート
  → SCORM / xAPI 対応 (eラーニング標準規格)
```

#### 11-3. メンバーシップ & サブスクリプション
```
課題:
- MemberPress, Restrict Content Pro で100万+サイト
- 有料コンテンツ/SaaSモデルのWordPressサイト

解決策:
- rustpress-membership クレート (または別リポ):
  → メンバーシップレベル: Free, Silver, Gold, Platinum (カスタム)
  → コンテンツ制限: 投稿/ページ/CPT単位でアクセス制御
  → ドリップコンテンツ: 時間ベースのコンテンツ開放
  → サブスクリプション課金: Stripe Billing / PayPal連携
  → ペイウォール: 記事ごとの単品課金
  → クーポン/プロモーションコード
  → メンバーダッシュボード
  → コンテンツ閲覧履歴 & お気に入り
```

#### 11-4. ページビルダー互換
```
課題:
- Elementor (1000万+), Divi (100万+), WPBakery が主要ページビルダー
- ページビルダーのデータは独自形式でpostmetaに保存

解決策:
- ページビルダーデータのレンダリング:
  → Elementor: JSON形式のメタデータ → HTML変換
  → Divi: ショートコードベース → 9-9のショートコード対応で処理
  → WPBakery: ショートコードベース → 同上
- 移行ツール:
  → ページビルダーデータ → Gutenbergブロック変換 (AI支援)
  → rustpress migrate page-builder でデータ構造を変換
- 長期的にはGutenbergベースの独自ビジュアルエディタ提供
```

#### 11-5. WooCommerce完全互換 (rustpress-commerce拡充)
```
課題:
- WooCommerce は600万+ストア、EC市場シェア28%
- 基本的なEC機能だけでは移行に不十分

追加実装:
- サブスクリプション商品 (定期購入)
- ダウンロード商品 (ファイルデリバリー)
- 商品バリエーション (サイズ×色マトリックス)
- 商品レビュー & 評価システム
- 商品バンドル/コンポジット商品
- 在庫管理: 低在庫アラート, 入荷待ち, 在庫予約
- 税計算エンジン: 地域別税率, TaxJar/Avalara連携
- 配送: 配送料計算, 追跡番号, 配送業者API連携
- アフィリエイトシステム
- ウィッシュリスト
- ポイント/ロイヤルティプログラム
- WooCommerce REST API完全互換: /wp-json/wc/v3/
```

#### 11-6. エンタープライズ / VIP 対応
```
課題:
- WordPress VIP は Fortune 500企業が利用
- コンプライアンス/SLA/セキュリティ要件が厳格

解決策:
- コンプライアンス対応:
  → SOC 2 Type II 準拠のセキュリティ設計
  → HIPAA 対応 (医療系データの取り扱い)
  → PCI DSS 対応 (決済データセキュリティ)
  → 監査ログ: 全管理操作の記録 (誰が何をいつ)
- SLA対応:
  → 99.99% 稼働率の設計ガイド
  → フェイルオーバー構成ドキュメント
  → インシデント対応プレイブック
- セキュリティ:
  → 定期的なペネトレーションテスト
  → CVE対応プロセス
  → セキュリティアドバイザリ公開体制
- パフォーマンス:
  → 10万+同時接続の負荷テスト
  → コンテンツ配信最適化 (エッジキャッシュ)
  → データベースシャーディング (超大規模サイト)
```

#### 11-7. アクセシビリティ (WCAG 2.1 AA)
```
課題:
- ADA/WCAG準拠は法的義務 (米国, EU, 日本)
- Fortune 500の30%がアクセシビリティ監査を実施
- 非準拠サイトは訴訟リスクあり

解決策:
- デフォルトテーマのWCAG 2.1 AA準拠:
  → ARIA ラベル/ロール全対応
  → キーボードナビゲーション完全対応
  → スキップリンク (本文へジャンプ)
  → フォーカス管理 (タブオーダー)
  → 色コントラスト比 4.5:1 以上
- 管理画面のアクセシビリティ:
  → スクリーンリーダー最適化
  → 全フォーム入力にラベル
  → エラーメッセージの音声読み上げ対応
- コンテンツアクセシビリティ:
  → 画像 alt テキスト未入力警告
  → 動画の字幕/キャプション要求
  → 見出しレベルの構造チェック
- アクセシビリティ監査ツール (管理画面):
  → 各ページのWCAGスコア表示
  → 改善提案の自動生成
- アクセシビリティステートメント生成ツール
```

#### 11-8. パスワード保護 & コンテンツ制限
```
課題:
- パスワード保護投稿/ページはWPの基本機能
- 非公開投稿, 下書きプレビュー共有も必須

解決策:
- パスワード保護投稿:
  → フロントエンドでパスワード入力フォーム表示
  → Cookie ベースのアクセス記憶 (セッション内有効)
  → 投稿/ページ/CPT全てに対応
- 非公開投稿: ログインユーザーのみ閲覧可能
- 下書きプレビュー共有:
  → 一時的なプレビューURL生成 (有効期限付き)
  → 未ログインでもプレビュー可能
- スケジュール投稿:
  → カレンダービュー (管理画面)
  → タイムゾーン対応 (著者のTZで設定)
  → 一括スケジュール変更
```

**Phase 11 完了基準:**
EC, フォーラム, LMS, メンバーシップ等の垂直市場サイトがRustPressに移行可能であること。
エンタープライズ要件 (コンプライアンス, SLA, セキュリティ) を満たすこと。
WCAG 2.1 AAに準拠し、アクセシビリティ監査をパスすること。

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
  → バックアップ, メール, 2FA/SSO, 監視, HA 全て動作

Phase 9 完了時: v2.0 Release
  → 完全WordPress互換 (i18n, Gutenberg, マルチサイト, 旧バージョン)
  → PostgreSQL/SQLite対応, GDPR準拠, ACF互換
  → 「あらゆるWPサイトがRustPressに移行可能」の実現

Phase 10 完了時: v3.0 Release
  → 移行ツール + ホスティング + エコシステム完成
  → RustPress Cloud 本格運用
  → Webインストーラー, デバッグツール, K8s対応

Phase 11 完了時: WordPress Killer
  → 垂直市場 (EC完全版, フォーラム, LMS, メンバーシップ) 対応
  → エンタープライズVIP品質
  → WCAG 2.1 AA 完全準拠
  → 全世界の全WordPressサイトの移行経路が確立
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
| **Full WP Compat** | i18n, Gutenberg, マルチサイト, PG/SQLite, GDPR, ACF | 9 |
| **v2.0** | 完全WordPress互換 | 9完了時 |
| **Ecosystem** | 移行ツール, ホスティング, ストア, Webインストーラー | 10 |
| **v3.0** | エコシステム完成 | 10完了時 |
| **Verticals** | EC完全版, フォーラム, LMS, メンバーシップ | 11 |
| **Enterprise** | VIP品質, コンプライアンス, WCAG | 11 |
| **WordPress Killer** | 全世界の全WPサイト移行可能 | 11完了時 |

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
| バックアップなしで本番移行不可 | 最高 | Phase 8でバックアップ/リストア実装。S3/GCS対応。増分バックアップ+ドライラン検証 |
| メール配信なしでEC/フォーム機能停止 | 最高 | lettreクレート拡充 + SendGrid/SES/Mailgun統合。メールキュー+リトライ |
| 2FA/SSOなしでエンタープライズ移行不可 | 高 | TOTP, WebAuthn, SAML 2.0, OAuth 2.0 全対応。Phase 8で実装 |
| MySQL専用だと30-40%のホスターを逃す | 高 | SeaORMのPG/SQLite対応を活用。接続文字列でDB自動判別。Phase 9で実装 |
| GDPR非準拠だとEU全域で使用不可 | 高 | 個人データエクスポート/削除、同意管理、プライバシーポリシー生成。Phase 9で実装 |
| ACFインポート不可でプロサイト移行不可 | 高 | ACF JSONインポート、全フィールドタイプ対応、ACF REST API互換。Phase 9で実装 |
| ショートコード未対応でコンテンツ崩壊 | 高 | 標準ショートコード完全実装 + oEmbed + カスタムショートコードAPI。Phase 9で実装 |
| Webインストーラーなしで非エンジニア排除 | 高 | ブラウザベースのセットアップウィザード。5分インストール。Phase 10で実装 |
| ページビルダーデータの互換性 | 高 | Elementor JSON/Divi ショートコード対応 + AI変換でGutenbergブロックに移行 |
| WooCommerce機能不足でEC移行不完全 | 高 | サブスクリプション、税計算、在庫、配送、アフィリエイト等フル実装。Phase 11で完成 |
| フォーラム/LMS/メンバーシップサイト移行不可 | 中 | 垂直市場向けプラグインをPhase 11で開発。コミュニティ開発も推進 |
| WCAG非準拠で訴訟リスク | 中 | デフォルトテーマWCAG 2.1 AA準拠。アクセシビリティ監査ツール内蔵。Phase 11で完成 |
| エンタープライズ品質に達しない | 高 | SOC2/HIPAA/PCI DSS対応設計。ペネトレーションテスト。SLAガイド。Phase 11で実装 |
| admin-ajax.php非互換でプラグインのフロント機能全滅 | 最高 | admin-ajax.php互換ディスパッチャをPhase 7で実装。wp_ajax_* フック対応 |
| FSE/ブロックテーマ未対応でWP 6.0+のデフォルトテーマが動かない | 最高 | theme.json パーサー + ブロックテンプレート対応をPhase 4で実装 |
| カスタムDBテーブル未対応で大規模プラグイン移行不可 | 高 | プラグイン独自テーブルの検出・移行をrustpress-migrateで対応。Phase 10で実装 |
| WP関数互換レイヤー不足でAI変換プラグインが動作しない | 最高 | Tier 1-3のWP関数互換APIをPhase 6で体系的に実装。カバレッジダッシュボード公開 |
| WPML/Polylang多言語コンテンツ移行不可 | 高 | 多言語コンテンツプラグイン互換をPhase 9で実装。icl_translations/pll_*の移行 |
| 共有ホスティング(60%+のWPサイト)でRust実行不可 | 最高 | RustPress Lite (CGI/FastCGI) + ホスティングパートナーシップ + RustPress Cloud。Phase 10 |
| カスタムGutenbergブロックのサーバーサイドレンダリング不可 | 高 | PHP render_callback → Rust変換をAI変換サービスで対応。Phase 9で実装 |
| Headless WP (GraphQL) サイト移行不可 | 中 | async-graphqlでWPGraphQL互換エンドポイント提供。Phase 7で実装 |
| Action Scheduler非互換でWooCommerce定期処理停止 | 高 | Tokioベースのジョブキュー + Action Scheduler互換テーブル。Phase 8で実装 |
| 非ACFカスタムフィールド (Meta Box, Pods等) 移行不可 | 高 | 各プラグインのメタデータ形式パーサーとrustpress-fieldsへの統一変換。Phase 9 |
| SEOプラグインデータ未移行で検索順位下落 | 最高 | Yoast/RankMath/AIOSEOのpostmetaを自動変換。Phase 10のseo-data移行ツール |
| PHPコード実行プラグイン (Code Snippets等) のDB内コード移行不可 | 中 | AI変換サービスでDB内PHPスニペットもRustに変換。動的eval()は変換不可警告 |
| プラグイン間依存関係でAI変換の順序問題 | 高 | 依存グラフ構築 + トポロジカルソートで変換順序を自動決定。Phase 6で実装 |
