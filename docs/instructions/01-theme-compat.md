# 指示書 #01: RustPress テーマ互換担当

## あなたの役割

あなたはRustPressプロジェクトの**テーマ互換性担当AI開発者**です。
あなたのミッションは以下の2つのBeta基準を達成することです:

- **B-1**: WordPress上位100テーマが全て正常表示される
- **B-4**: 全ページでWordPressと97%+ピクセルマッチ

他の担当者がAPI、セキュリティ、CIなどを並行で開発しています。あなたはテーマ表示に集中してください。

---

## プロジェクト概要

RustPressはWordPress互換のCMSで、Rustで書かれています。WordPressの既存MySQLデータベースにそのまま接続し、同じコンテンツを表示します。テンプレートエンジンはTera（Jinja2系）を使用しています。

- リポジトリ: https://github.com/LegacyToRustProject/RustPress
- 言語: Rust 1.88+
- フレームワーク: Axum + Tokio + SeaORM + Tera
- DB: MySQL 8.0（WordPressと同じDB）

---

## リポジトリ構成（テーマ関連）

```
RustPress/
├── crates/
│   ├── rustpress-themes/        # あなたの主な作業場所
│   │   └── src/
│   │       ├── engine.rs        # ThemeEngine: Teraテンプレートのロード・レンダリング
│   │       ├── hierarchy.rs     # TemplateHierarchy: WPテンプレート階層の解決
│   │       ├── tags.rs          # テンプレートタグ（the_title, the_content等のTera関数）
│   │       ├── formatting.rs    # wpautop, wptexturize等の書式変換
│   │       ├── theme_json.rs    # theme.json パーサー（CSS変数生成）
│   │       ├── wp_head.rs       # wp_head() 出力
│   │       └── lib.rs
│   ├── rustpress-server/        # Webサーバー（ルーティング）
│   │   └── src/
│   │       └── routes/
│   │           └── frontend.rs  # フロントエンドのルートハンドラー（テンプレート呼び出し）
│   ├── rustpress-query/         # WP_Query互換のクエリビルダー
│   ├── rustpress-db/            # DBエンティティ（wp_posts, wp_options等）
│   └── rustpress-blocks/        # Gutenbergブロックレンダリング
├── templates/                   # 現在のTwenty Twenty-Five Teraテンプレート
│   ├── base.html               # 全ページ共通レイアウト
│   ├── single.html             # 単一投稿
│   ├── page.html               # 固定ページ
│   ├── archive.html            # アーカイブ
│   ├── category.html           # カテゴリ
│   ├── search.html             # 検索結果
│   ├── 404.html                # 404ページ
│   ├── header.html             # ヘッダー部品
│   ├── footer.html             # フッター部品
│   └── theme.json              # テーマ設定（CSS変数等）
├── static/                      # CSS, JS, 画像等の静的ファイル
└── docker-compose.yml           # テスト環境（WordPress + RustPress + Selenium）
```

---

## 現在の状態

### 動いているもの
- Twenty Twenty-Five (TT25) テーマで97%+ピクセルマッチ（1テーマのみ）
- WordPress Template Hierarchy の基本実装（single, page, archive, category, tag, author, search, 404, front-page, home, attachment）
- Teraテンプレートタグ: the_title, the_content, the_excerpt, the_permalink, the_date等
- wpautop（段落自動整形）、書式変換パイプライン
- theme.json パーサー（CSS変数生成）
- ページキャッシュ（moka, 5分TTL）

### まだないもの / 不完全なもの
- TT25以外のテーマは全くテストされていない
- FSE（Full Site Editing）ブロックテーマの完全対応
- レガシーウィジェットシステム（register_widget / dynamic_sidebar）
- wp_nav_menu() の完全実装（カスタムウォーカー等）
- 多くのテンプレートタグが未実装（get_template_part等）
- カスタム投稿タイプ用テンプレート
- ブロックパターン・ブロックスタイル

---

## ゴール: B-1（上位100テーマ対応）

### 手順

#### Step 1: 上位100テーマのリストを作成

WordPressテーマディレクトリ（https://wordpress.org/themes/browse/popular/）から上位100テーマを取得し、`docs/theme-compat-matrix.md` にリストを作成する。

各テーマについて以下を記録:
- テーマ名
- タイプ（ブロックテーマ / クラシックテーマ）
- 使用するテンプレートタグ / ウィジェット / メニュー
- 対応状態（未テスト / 表示崩れあり / OK）

#### Step 2: テーマをカテゴリ分類

テーマは大きく2種類:

1. **ブロックテーマ（FSE）** — WordPress 5.9+。theme.json + HTMLブロックテンプレート
   - 例: Twenty Twenty-Five, Twenty Twenty-Four, Twenty Twenty-Three
   - 対応に必要: theme.json完全パース、ブロックレンダリング、グローバルスタイル

2. **クラシックテーマ** — PHP テンプレート。functions.php + テンプレートファイル
   - 例: Astra, OceanWP, GeneratePress, Flavor
   - 対応に必要: wp_nav_menu, ウィジェット, サイドバー, カスタムヘッダー/フッター
   - **重要**: クラシックテーマのPHPテンプレートは直接実行できない。Teraテンプレートへの変換が必要。この変換は php-to-rust チーム（別担当）と連携する。あなたの仕事は**Tera側で必要なテンプレートタグ・機能を全て用意すること**。

#### Step 3: テーマ切り替え機構の実装

現在はテンプレートが `templates/` に直接置かれている。これを以下に変更:

```
themes/
├── twentytwentyfive/
│   ├── theme.json
│   ├── templates/
│   │   ├── base.html
│   │   ├── single.html
│   │   └── ...
│   └── static/
│       ├── style.css
│       └── ...
├── astra/
│   └── ...
└── oceanwp/
    └── ...
```

- `.env` または `wp_options` の `template` / `stylesheet` の値でアクティブテーマを決定
- `ThemeEngine::new()` が `themes/{active_theme}/` からテンプレートを読む
- テーマごとのstatic assets を `/wp-content/themes/{name}/` で配信

#### Step 4: 不足テンプレートタグの実装

WordPress上位100テーマで使われる主要テンプレートタグを全て実装する。

以下は未実装または不完全なものの例（実際のテーマ解析で追加すること）:

| テンプレートタグ | 用途 | 実装場所 |
|---|---|---|
| `get_template_part()` | テンプレートの部品読み込み | `tags.rs` |
| `wp_nav_menu()` | ナビゲーションメニュー | `tags.rs` + 新規 `menu.rs` |
| `dynamic_sidebar()` | ウィジェットエリア表示 | 新規 `widgets.rs` |
| `the_post_thumbnail()` | アイキャッチ画像 | `tags.rs` |
| `wp_enqueue_style/script()` | CSS/JSの読み込み | `wp_head.rs` |
| `body_class()` | body要素のCSSクラス | `tags.rs` |
| `post_class()` | 投稿要素のCSSクラス | `tags.rs` |
| `get_header() / get_footer()` | ヘッダー/フッター読み込み | `tags.rs` |
| `comments_template()` | コメントテンプレート読み込み | `tags.rs` |
| `wp_link_pages()` | ページ分割リンク | `tags.rs` |
| `the_tags() / the_category()` | タグ/カテゴリ表示 | `tags.rs` |
| `get_search_form()` | 検索フォーム出力 | `tags.rs` |
| `wp_footer()` | フッタースクリプト出力 | `wp_head.rs` |

Teraでの実装方法: Tera のカスタム関数 (`tera.register_function`) として登録する。テンプレート内では `{{ wp_nav_menu(location="primary") }}` のように呼び出す。

#### Step 5: ウィジェットシステムの実装

クラシックテーマの大半がウィジェットを使用する。

```rust
// 必要な実装:
// 1. ウィジェットエリア（サイドバー）の定義読み取り
//    - wp_options の sidebars_widgets から読み取り
// 2. 各ウィジェットのレンダリング
//    - 最近の投稿、カテゴリ一覧、アーカイブ、テキスト、カスタムHTML
//    - 検索、メタ情報、RSS、タグクラウド、ナビゲーションメニュー
// 3. dynamic_sidebar() Tera関数
//    - テンプレートから {{ dynamic_sidebar(name="sidebar-1") }} で呼び出し
```

ウィジェットデータはWordPress DBの `wp_options` テーブルに `widget_*` キーで保存されている（PHPシリアライズ形式）。既にPHPシリアライズのパーサーが `rustpress-db` にあるのでそれを利用する。

#### Step 6: ナビゲーションメニュー完全実装

```rust
// wp_nav_menu() で必要な機能:
// 1. メニューロケーション → メニューID の解決
//    - wp_options の nav_menu_locations から
// 2. メニューアイテムの取得
//    - wp_posts (post_type = 'nav_menu_item') + wp_postmeta
// 3. 階層構造（親子）のネストレンダリング
// 4. CSSクラス自動付与:
//    - current-menu-item（現在のページ）
//    - current-menu-ancestor（祖先メニュー）
//    - menu-item-has-children（子を持つアイテム）
// 5. HTMLラッパー: <nav>, <ul>, <li>, <a> の構造
```

#### Step 7: theme.json の完全パース

ブロックテーマのスタイルは全て theme.json で定義される。

```
theme.json の主要セクション:
- settings.color.palette → CSS変数 (--wp--preset--color--{slug})
- settings.typography.fontSizes → CSS変数
- settings.spacing.spacingSizes → CSS変数
- settings.layout.contentSize / wideSize
- styles.color.background / text
- styles.typography.fontSize / fontFamily
- styles.spacing.padding / margin
- styles.blocks.{block-name} → ブロック個別スタイル
- customTemplates → カスタムテンプレート定義
- templateParts → テンプレートパーツ定義
```

現在の `theme_json.rs` を拡張し、上記全セクションをCSS変数に変換する。

---

## ゴール: B-4（97%+ピクセルマッチ）

### 検証方法

既にE2Eテスト環境がある:

```bash
# Docker環境を起動
docker compose --profile e2e up --build --abort-on-container-exit --exit-code-from e2e
```

これにより:
1. MySQL + WordPress（:8081） + RustPress（:8080） + Selenium が起動
2. E2Eテストが WordPress と RustPress の同じページのスクリーンショットを撮影
3. ピクセル単位で比較し、差分を報告

#### E2Eテストの場所

```
crates/rustpress-e2e/          # E2Eテストクレート
```

#### テストの追加方法

各テーマ × 各ページタイプのスクリーンショット比較テストを追加する:

```rust
// テストの基本パターン:
// 1. Selenium で WordPress のページを開いてスクリーンショット
// 2. Selenium で RustPress の同じパスを開いてスクリーンショット
// 3. 画像をピクセル比較（97%以上の一致で合格）
```

#### 差分が出やすいポイント

これまでの開発で判明している差分の原因:
- CSS変数の値の違い（theme.json パース不完全）
- フォントの読み込みタイミング（Google Fonts等）
- wp_head() / wp_footer() の出力内容の違い
- ブロックコメント（`<!-- wp:paragraph -->` 等）の処理
- 日付フォーマットの違い（WordPressの "F j, Y" 形式）
- サイドバー/ウィジェットの有無

---

## 開発ルール

### ビルド・テスト

```bash
# コンパイル確認
cargo check --workspace

# ユニットテスト実行
cargo test --workspace --lib --bins -- --skip e2e

# 特定クレートのテスト
cargo test -p rustpress-themes --lib

# E2Eテスト（Docker環境が必要）
docker compose --profile e2e up --build --abort-on-container-exit --exit-code-from e2e
```

### コード品質

```bash
# フォーマット
cargo fmt --all

# Linter
cargo clippy --workspace -- -D warnings
```

### コミットルール

- 機能追加: `feat: Add wp_nav_menu() template tag`
- バグ修正: `fix: Correct category template resolution for nested categories`
- テスト: `test: Add E2E pixel comparison for Astra theme`
- コミットは小さく、頻繁に。1機能1コミット。

### ブランチ

- `main` ブランチで作業
- 大きな変更はfeatureブランチ → PR

---

## 作業の優先順位

1. **テーマ切り替え機構**（Step 3）— これがないと複数テーマをテストできない
2. **wp_nav_menu() 完全実装**（Step 6）— ほぼ全テーマで使用
3. **ウィジェットシステム**（Step 5）— クラシックテーマの過半数が使用
4. **不足テンプレートタグ**（Step 4）— テーマごとに必要なものを追加
5. **theme.json 完全パース**（Step 7）— ブロックテーマの表示品質向上
6. **上位100テーマのテスト**（Step 1, 2）— 全テーマで検証
7. **E2Eテスト追加**（B-4）— ピクセルマッチの自動検証

---

## 完了条件

以下が全て満たされた時、あなたの仕事は完了です:

- [ ] WordPress上位100テーマの互換性マトリクス（docs/theme-compat-matrix.md）が作成されている
- [ ] テーマ切り替え機構が動作する（.envまたはDB設定でテーマ変更可能）
- [ ] 上位100テーマが全てエラーなく表示される（表示崩れ軽微はOK、クラッシュはNG）
- [ ] 上位20テーマで97%+ピクセルマッチのE2Eテストが通る
- [ ] wp_nav_menu(), dynamic_sidebar(), 主要テンプレートタグが全て実装済み
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

1. **WordPressの動作が正解。** 迷ったらWordPressの挙動を再現する。
2. **完璧より動作優先。** 80%の互換性で動くものを先に出し、残り20%を後から埋める。
3. **テストを書く。** 実装したらテストを書く。テストがないコードは存在しないのと同じ。
4. **MASTERPLANを読む。** 不明点があればプロジェクトルートの `MASTERPLAN.md` に詳細な仕様がある。
