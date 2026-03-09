# RustPress

**WordPressをRustで書き直したCMS。** 既存のWordPressデータベースに接続するだけで、コンテンツもテーマもプラグインも変換されて即座に動く。100倍高速に。

[![License: GPL v2](https://img.shields.io/badge/License-GPL%20v2-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html)
[![Rust: 1.88+](https://img.shields.io/badge/Rust-1.91%2B-orange.svg)](https://www.rust-lang.org/)
[![Status: Alpha](https://img.shields.io/badge/Status-Alpha-yellow.svg)](#ステータス)

[English](README.md)

> **Alpha Release** — WordPress 6.9 互換を目標に開発中。フロントエンドはTwenty Twenty-Fiveテーマと97%以上のピクセル一致を達成。コントリビューション歓迎！

---

## 課題: WordPressはセキュリティ危機にある

WordPressはウェブの **43%** を動かしている — 8億サイト以上。しかし、この数字の裏にある現実は深刻である:

| 問題 | 規模 |
|------|------|
| **古いバージョンのWordPress** | [全サイトの49.8%が古いバージョンで稼働](https://www.wpbeginner.com/research/ultimate-wordpress-statistics/) |
| **脆弱なプラグイン** | WPセキュリティ問題の[97%がプラグインの脆弱性](https://patchstack.com/whitepaper/state-of-wordpress-security-in-2024/) |
| **放置されたサイト** | 数百万の「セットアップして忘れた」サイトがセキュリティ更新なしで公開中 |
| **年間ハッキング件数** | 1日あたり約13,000サイトがハッキング被害（[年間約470万サイト](https://www.colorlib.com/wp/wordpress-statistics/)） |
| **PHPのメモリ消費** | プロセスあたり50-100 MB、同時接続数が制限される |
| **サーバーコスト** | PHPのリクエスト単位モデルではスケールに高額なホスティングが必要 |

根本的な問題: **PHPは20年前のアーキテクチャを持つインタプリタ言語である。** リクエストのたびにランタイム全体をゼロからブートストラップする。プラグインは任意のPHPコードを実行できる攻撃面そのもの。パッチ未適用のサイトは攻撃者への招待状。

大半のサイトオーナーは開発者ではない。WordPressを一度セットアップし、プラグインをインストールし、二度と更新しない。セキュリティモデルが人間の継続的な注意力に依存している — しかし人間は忘れる。

### AIが脅威を加速させている

この危機は壊滅的に悪化しつつある。AIがサイバー攻撃を根本的に変えている:

- **脆弱性の自動発見** — AIエージェントがインターネット全体をスキャンし、WordPressサイトを検出し、バージョンとインストール済みプラグインを特定し、既知の脆弱性を数秒で発見できる。人間の攻撃者が数時間かけていた偵察が、ミリ秒で完了する。
- **AI生成エクスプロイト** — LLMがCVE公開情報を分析し、動作するエクスプロイトコードを生成できる。攻撃者に必要なスキルの閾値がほぼゼロに低下した。
- **自律型攻撃チェーン** — AIエージェントが脆弱性を発見し、エクスプロイトを生成し、ペイロードをデプロイし、永続化を確立し、横展開する — 全て人間の介入なしで。
- **スケール** — 1つのAIエージェントが数千のWordPressサイトを同時に攻撃できる。年間470万サイトのハッキング被害は、近い将来「少なかった時代」として振り返られるだろう。

WordPressのセキュリティモデル — 「人間が手動でパッチを適用する」— は、AIの攻撃者がマシンスピードで24時間365日、8億のターゲットに対して活動する時代には生き残れない。**スケールする唯一の防御は、脆弱性の攻撃面を根本から排除すること。** それはメモリ安全なネイティブコードへのコンパイル、プラグインのサンドボックス化、PHPの任意コード実行モデルの廃止を意味する。

これは将来の脅威ではない。今起きていることだ。

---

## 解決策: WordPressを単一バイナリにコンパイルする

RustPressは根本的に異なるアプローチを取る:

```
WordPress (PHP)                    RustPress (Rust)
├── 実行時にインタプリタ             ├── ネイティブバイナリにコンパイル
├── プロセスあたり50-100 MB          ├── 合計35 MB
├── リクエストごとにブートストラップ    ├── 常駐型非同期サーバー
├── プラグイン = 任意のPHPコード      ├── プラグイン = サンドボックス化WASM or ネイティブRust
├── 文字列操作によるSQLインジェクション ├── 型システムが強制するパラメタライズドクエリ
├── 常にパッチが必要                  ├── 構造的にメモリ安全
└── ページあたり ~200ms              └── ページあたり ~2ms
```

**既存のWordPressデータベースにRustPressを接続するだけ。サイトは100倍高速になり、構造的に安全になる。**

---

## 移行: そのまま動く

RustPressは、WordPressからの移行を「既存のデータに接続するだけ」で完了できるように設計されている:

### データベース — 移行作業ゼロ

```env
DATABASE_URL=mysql://user:pass@localhost:3306/wordpress
SKIP_MIGRATIONS=true
```

RustPressは **WordPressと全く同じテーブル** (`wp_posts`, `wp_options`, `wp_users` 等) を直接読み取る。データ変換なし、エクスポート/インポートなし、ダウンタイムなし。移行期間中、WordPressとRustPressを同じデータベース上で並行稼働させることもできる。

### テーマ — AI変換

既存のWordPressテーマはAIによってPHPからTeraテンプレートに変換される。変換にはWordPress自体の出力を正解として使用し、自動ビジュアル比較テストによるピクセル単位の精度検証を行う。

デフォルトテーマ（Twenty Twenty-Five相当）はRustPressに同梱され、WordPress版と **97%以上のピクセル一致** を達成している。

### プラグイン — AI変換

WordPressプラグインはAIによってPHPからRustに変換される。これが機能する理由:

1. **WordPressは100%オープンソース** — 全てのPHPコードが読める
2. **AIがPHPソースコードを読む** — コードがそのまま仕様書
3. **AIがRustに変換する** — WP関数のRust実装を呼び出すコードを生成
4. **WordPress出力と比較検証する** — 正解が常に存在する
5. **差分を修正する** — 正解が存在するから必ず修正可能、100%一致するまで繰り返す

主要プラグイン（WooCommerce、Yoast SEO、Contact Form 7、ACF、Wordfence）はこのリポジトリ内でRustネイティブとして最大性能で再構築中。

> **根幹思想:** RustPressを可能にしたのはRustの速度ではない。「正解のソースコード（WordPressのPHP）が完全に存在する泥臭い変換作業を、AIがスケールさせられる時代になった」からである。 [詳細](docs/adr/001-php-bridge-mode.md)

---

## 使い方を選べる — 小さく始めて、必要なだけ広げる

RustPressは「全部一気に移行」を求めない。各ステップは独立しており、必要なところまで進めればいい。

### Step 1: まず試す — 一部のページをRustPressに向ける

WordPressはそのまま動き続ける。重いページだけ、あるいは全公開ページをNginxでRustPressに向ける。記事の書き方は何も変わらない。

```nginx
# 選択肢A: 特定の重いページだけ（例: 商品ページ、検索）
location ~ ^/(shop|products|search)/ {
    proxy_pass http://127.0.0.1:3000;
    error_page 502 503 = @wordpress;  # 何かあれば自動でWPにフォールバック
}

# 選択肢B: 全公開ページ（wp-adminはPHPのまま）
location / {
    proxy_pass http://127.0.0.1:3000;
    error_page 502 503 = @wordpress;
}

location @wordpress { fastcgi_pass php-fpm; }
```

### Step 2: 重いページを静的ファイルに変換する

アクセスが多く、更新頻度が低いページを事前生成する。CDN配信 — データベースなし、PHPなし、そのページに関しては攻撃面ゼロ。

```bash
rustpress generate --path /products/
rustpress generate --path /lp/summer-sale/
```

Nginxが `dist/` フォルダを直接配信。何かあれば自動で動的サーバーにフォールバックする。

### Step 3: 完全置き換え

PHPを完全に除去。単一バイナリ、単一設定ファイル。

```
移行前: Nginx → PHP-FPM → WordPress → MySQL   (200ms, 80MB RAM)
移行後: Nginx → RustPress → MySQL             (2ms, 35MB RAM)
```

---

## 全てのページタイプに対応

> *「ユーザーごとに表示が変わるページはどうなる？」*

RustPressはWordPressが処理できる全てのページタイプを処理できる。SSGは静的にできるページへのオプション最適化であって、必須ではない。

| ページ種別 | RustPressの処理方法 | 典型的な速度 |
|-----------|-------------------|:-----------:|
| 静的コンテンツ（会社概要、LP） | 事前生成HTML、CDN配信 | **1ms未満** |
| 動的公開ページ（新着記事一覧、検索） | Rustがリクエストのたびにレンダリング | **~3ms** |
| ログイン必須ページ（会員ページ、マイページ） | セッション確認 → Rustがユーザー個別にレンダリング | **~5ms** |
| EC（カート、注文履歴、決済） | 完全動的、セッション対応 | **~8ms** |

WordPressが遅い理由は「動的だから」ではない。**リクエストのたびにPHPランタイム全体をゼロからブートストラップするから**だ。RustPressは常駐型の非同期サーバー。セッション確認もDBクエリもマイクロ秒単位で完了する。

**動的ページは遅くなくていい。PHPでなければいい。**

---

## 使命: 全てのWordPressサイトに移行経路を

RustPressの目標は単に高速なCMSを作ることではない。**全世界の全WordPressサイトに移行経路を確立すること。**

```
任意のWordPressサイト
    ↓ rustpress migrate analyze（互換性レポート）
    ↓ rustpress migrate database（既存DBに接続）
    ↓ rustpress migrate theme（AIがPHP → Teraに変換）
    ↓ rustpress migrate plugins（AI変換 or Rustネイティブ代替）
    ↓ rustpress migrate seo-audit（SEO影響ゼロを検証）
RustPressサイト — 100倍高速、構造的に安全、単一バイナリ
```

8億のWordPressサイトは前に進む道を必要としている。専門のエンジニアチームを持つサイトだけでなく — **全てのサイト。** 3年間更新されていないが今もアクセスがある小さなブログ、中小企業のホームページ、NPOのウェブサイトも含めて。

### 誰一人見捨てない

**すべてのバージョンにセキュリティパッチを提供し続ける。** なぜ可能か？ AIによる開発でメンテナンスの限界コストがほぼゼロになるからだ。従来のオープンソースプロジェクトは古いバージョンのサポートを打ち切らざるを得なかった — 人間の労働コストが高すぎるからだ。RustPressはこの制約を打ち破る。AIがWordPressのバージョン間差分を読み、対応するRustパッチを生成できるなら、「サポート終了」は必然ではなく選択になる。我々は、誰一人見捨てないことを選ぶ。

---

## Beta への道

**我々のBetaは「だいたい動く」ではない。「ほぼ完成品」だ。** AI駆動開発だからこそ、基準を高く設定できる。

| # | 条件 | 状態 |
|---|------|------|
| B-1 | WordPress上位100テーマが全て正常表示 | 🟡 開発中 | TT16–TT25（公式テーマ10本）完成。Astra/Divi/OceanWP予定。 |
| B-2 | WordPress上位50プラグインのデータが移行・表示可能 | 🟡 開発中 | WooCommerce, Yoast, CF7, ACF, Wordfence — Rustネイティブクレート構築中 |
| B-3 | WP REST API v2が100%互換 | 🟡 開発中 | 73ブロックタイプ、リビジョン、自動保存、検索、テンプレート、グローバルスタイル完成 |
| B-4 | 全ページでWordPressと97%+ピクセルマッチ | ✅ 完成 | **98.27%** 平均（TT25、9ページタイプ）。全ページ97%超え。 |
| B-5 | `rustpress migrate` コマンド一発で5分以内に稼働 | ✅ 完成 | `.env`生成、スキーマ確認、互換性レポート |
| B-6 | OWASP Top 10 全項目に対応 | 🟡 開発中 | レート制限、セッション固定、Argon2id、TOTP 2FA、JWTブラックリスト、WAF、XML-RPCブロック、OAuth 2.0/OIDC、SAML 2.0完成。ZAPスキャン待ち。 |
| B-7 | CI/CDが完全稼働 | ✅ 完成 | GitHub Actions: check, test, clippy, fmt, audit, build |

従来の開発ならRC（リリース候補）レベル。AI開発ではこれをBetaに設定し、到達速度で価値を証明する。

### Beta Sprint 進捗 (2026-03-09時点)

```
RustPress (main)          ██████████████████████░░   88%
 ├─ #01 テーマ              ████████████████████████░  95%  TT16–TT25完成（10テーマ）
 ├─ #02 REST API           ████████████████████░░░░░  80%  73ブロック、リビジョン、検索
 ├─ #03a 認証/セッション    ████████████████████████░  95%  OAuth2/OIDC, SAML 2.0, TOTP, Argon2id
 ├─ #03b エンドポイント防御  ████████████████████░░░░░  80%  XML-RPC, WAF, ヘッダー, C1-C5修正済み
 └─ QA (#09)               ███████████████░░░░░░░░░░  60%  PR#3マージ待ち

Phase 8 (完了 ✅)
 ├─ OAuth 2.0/OIDC         ████████████████████████░  95%  Google, GitHub, Microsoft, Apple, PKCE
 ├─ SAML 2.0 SP            ████████████████████████░  95%
 └─ Observability          ████████████████████████░  95%  OTLP + Sentry + Prometheus

変換エンジン               ████████████░░░░░░░░░░░░   52%
 ├─ #04 php-to-rust        ████████████░░░░░░░░░░░░░  45%
 ├─ #05 cobol-to-rust      █████████████░░░░░░░░░░░░  50%
 ├─ #06 cpp-to-rust        ██████████████░░░░░░░░░░░  55%
 ├─ #07 java-to-rust       ███████████████░░░░░░░░░░  60%
 └─ #08 perl-to-rust       ██████████████░░░░░░░░░░░  55%

全体                       ████████████████████░░░░░  80%
```

---

## パフォーマンス

同一マシン、同一MySQLデータベース、同一コンテンツでのベンチマーク。

| 指標 | WordPress (PHP 8.x) | RustPress (Rust) | 改善 |
|------|---------------------|------------------|------|
| **トップページ応答** | 200-500 ms | **2.7 ms** | **74-185倍高速** |
| **REST API (投稿)** | 100-300 ms | **5.9 ms** | **17-51倍高速** |
| **メモリ使用量** | 50-100 MB | **35 MB** | **1.4-2.9倍削減** |
| **リクエスト/秒** | 10-50 rps | **509 rps** | **10-50倍** |
| **起動時間** | 2-5秒 | **0.4秒** | **5-12倍高速** |
| **バイナリサイズ** | PHPランタイム + 依存 | **19 MB** | 単一バイナリ |

---

## クイックスタート

### 方法1: Docker（推奨）

```bash
git clone https://github.com/rustpress-project/RustPress.git
cd RustPress
cp .env.example .env

# MySQL + RustPress を起動
docker compose up -d
```

`http://localhost:8080` でアクセスできます。

### 方法2: ソースからビルド

**必要環境:** Rust 1.88+、MySQL 8.0+（またはMariaDB 10.5+）

```bash
git clone https://github.com/rustpress-project/RustPress.git
cd RustPress

cp .env.example .env
# .env を編集 — DATABASE_URL にWordPressデータベースを設定

cargo build --release
./target/release/rustpress-server
```

### 既存のWordPressデータベースを使用

RustPressは標準の `wp_*` テーブルを直接読み取ります。`SKIP_MIGRATIONS=true` を設定し、`DATABASE_URL` をWordPressデータベースに向けてください：

```env
DATABASE_URL=mysql://user:pass@localhost:3306/wordpress
SKIP_MIGRATIONS=true
```

---

## 機能

### コンテンツ配信（実装済み）
- WordPressテンプレート階層の完全対応（`single`, `page`, `archive`, `category`, `tag`, `author`, `search`, `404`）
- Twenty Twenty-Fiveテーマとの視覚的一致（Selenium E2Eテストで97%以上のピクセル一致）
- 投稿、固定ページ、カテゴリー、タグ、コメント（スレッド表示）
- 固定表示投稿、パスワード保護投稿、予約投稿
- RSSフィード (`/feed`)、XMLサイトマップ (`/sitemap.xml`)、robots.txt
- パーマリンク構造 (`/%postname%/`, `/%year%/%monthnum%/%day%/`)

### REST API（WP v2 互換）
```
GET/POST   /wp-json/wp/v2/posts
GET/PUT/DEL /wp-json/wp/v2/posts/{id}
```
他: `/pages`, `/media`, `/users`, `/categories`, `/tags`, `/comments`, `/search`, `/settings`, `/types`, `/taxonomies`, `/menus`, `/themes`, `/plugins`

### 認証とセキュリティ
- JWT トークン（API用）、HTTP-only Cookie（セッション用）
- Argon2（新規）+ bcrypt（WordPress既存互換）パスワードハッシュ
- ロールベースアクセス制御（5ロール、73権限）
- セキュリティヘッダー（CSP、X-Frame-Options、HSTS）

### ネイティブRustプラグインクレート
| クレート | WordPress相当 | 状態 |
|---------|-------------|------|
| `rustpress-commerce` | WooCommerce | 開発中 |
| `rustpress-seo` | Yoast / RankMath | 開発中 |
| `rustpress-forms` | Contact Form 7 / Gravity Forms | 開発中 |
| `rustpress-fields` | ACF (Advanced Custom Fields) | 開発中 |
| `rustpress-security` | Wordfence | 開発中 |

### インフラストラクチャ
- ページキャッシュ（moka、5分TTL、サブミリ秒応答）
- gzip圧縮
- SeaORMによるコネクションプーリング
- コンパイル済みTeraテンプレート（起動時に一度だけパース）

### 開発予定
- [ ] `theme.json` パーサーによるCSS変数の完全一致
- [ ] Gutenbergブロックレンダリング（高度なブロック）
- [ ] プラグインフックシステム（`add_action`/`add_filter` のRust実装）
- [ ] WASMプラグインランタイム（Extism）
- [ ] 管理画面（wp-admin）の完全対応
- [ ] マルチサイト対応
- [ ] WPGraphQL互換エンドポイント
- [ ] AIプラグイン/テーマ変換サービス（rustpress-convert）

---

## アーキテクチャ

```
rustpress/
├── crates/
│   ├── rustpress-server    # Axum Webサーバー、ルーティング、ミドルウェア
│   ├── rustpress-db        # SeaORM エンティティ、マイグレーション
│   ├── rustpress-api       # WP REST API v2 エンドポイント
│   ├── rustpress-auth      # JWT、セッション、パスワード、RBAC
│   ├── rustpress-themes    # テンプレートエンジン、テンプレートタグ
│   ├── rustpress-query     # WP_Query形式のクエリビルダー
│   ├── rustpress-cache     # ページ/オブジェクト/トランジェントキャッシュ
│   ├── rustpress-plugins   # プラグインレジストリ、WASMホスト
│   ├── rustpress-admin     # 管理画面CRUD API
│   ├── rustpress-migrate   # データベースマイグレーション
│   ├── rustpress-cron      # バックグラウンドタスク
│   └── rustpress-e2e       # Selenium ビジュアル比較テスト
├── templates/              # Teraテンプレート（TT25互換）
├── static/                 # CSS、フォント、アセット
└── docker-compose.yml
```

| レイヤー | 技術 |
|---------|------|
| **Webフレームワーク** | [Axum](https://github.com/tokio-rs/axum) + [Tokio](https://tokio.rs/) |
| **ORM** | [SeaORM](https://www.sea-ql.org/SeaORM/) (MySQL) |
| **テンプレート** | [Tera](https://keats.github.io/tera/) |
| **キャッシュ** | [Moka](https://github.com/moka-rs/moka) |

---

## データベース互換性

RustPressは **WordPressと全く同じスキーマ** を使用する:

```
wp_posts, wp_postmeta, wp_users, wp_usermeta, wp_options,
wp_comments, wp_commentmeta, wp_terms, wp_term_taxonomy,
wp_term_relationships, wp_links, wp_termmeta
```

WordPressとRustPressは同じデータベース上で並行稼働できる。

---

## テスト

```bash
# ユニットテスト
cargo test --workspace

# E2Eビジュアル比較（Docker必要）
docker compose --profile e2e up -d
./tests/run_e2e.sh
```

E2Eテストスイートは、SeleniumでWordPressとRustPressの両方のスクリーンショットを取得し、9種類のページタイプでピクセル単位の比較を行います。閾値: 93%（実測: 97%以上）。

---

## コントリビューション

[CONTRIBUTING.md](CONTRIBUTING.md) をご参照ください。

バグ報告、テーマ互換性の改善、新機能、ドキュメント、実際のWordPressデータベースでのテストなど、あらゆるコントリビューションを歓迎します。

---

## アーキテクチャ決定記録

重要な設計判断はADR（Architecture Decision Records）として記録しています:

- [ADR-001: PHP Bridge Mode の採否とプラグイン互換性戦略](docs/adr/001-php-bridge-mode.md)（[English](docs/adr/001-php-bridge-mode.en.md)）

---

## ステータス

**Alpha** — **WordPress 6.9** 互換を目標に開発中。高い視覚的忠実度でWordPressコンテンツを配信できますが、プロダクション利用にはまだ対応していません。

動作している機能:
- 全WordPressコンテンツタイプの読み込みと配信
- REST API互換
- TT25テーマとの視覚的一致（97%以上）
- パフォーマンス（PHP WordPressの100倍以上高速）

開発中の機能:
- 管理画面
- フロントエンドからの書き込み操作
- プラグインシステム
- Gutenbergブロック完全対応

---

## ライセンス

GPL v2（WordPressと同じ）。[LICENSE](LICENSE) をご参照ください。
