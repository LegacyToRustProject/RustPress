# RustPress

**WordPressをRustで完全に書き直したCMS。** クローンでも「インスパイア」でもない。本物のWordPress互換を、速度と安全性とともに。

[![License: GPL v2](https://img.shields.io/badge/License-GPL%20v2-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html)
[![Rust: 1.88+](https://img.shields.io/badge/Rust-1.88%2B-orange.svg)](https://www.rust-lang.org/)
[![Status: Alpha](https://img.shields.io/badge/Status-Alpha-yellow.svg)](#%E3%82%B9%E3%83%86%E3%83%BC%E3%82%BF%E3%82%B9)

[English](README.md)

> **Alpha Release** — WordPress 6.9 互換を目標に開発中。フロントエンドはTwenty Twenty-Fiveテーマと97%以上のピクセル一致を達成。コントリビューション歓迎！

---

## ビジョン

RustPressは**100% WordPress互換のCMS**をRustで実現することを目指しています。同じデータベーススキーマ、同じREST API、同じテンプレート出力、同じテーマレンダリング — WordPressとピクセル単位で一致する出力を、Rustの性能と安全性とともに提供します。

**このプロジェクトはAI支援開発で構築されています。** 開発コストをほぼゼロに近づけることで、従来は大規模チームと長い年月が必要だったWordPressの広大な互換性対応を実現しています。

### デュアルモード テーマ＆プラグイン アーキテクチャ

RustPressはテーマとプラグインに対して2つのアプローチを設計しています：

1. **WordPress互換モード** — 既存のWordPress PHPテーマ・プラグインをそのまま読み込み・レンダリング。WordPress エコシステムとの完全な互換性を実現。
2. **Rust最適化モード** — ネイティブRustテーマとプラグイン（WebAssembly対応）。PHPインタープリタのオーバーヘッドなし、ビルド時コンパイル、型安全なプラグインAPI。

両モードは共存可能で、PHPからRustへの段階的な移行が可能です。

---

## なぜRustPress？

WordPressはウェブの40%以上を動かしていますが、PHPのリクエストごとのオーバーヘッドがパフォーマンスの限界となっています。RustPressは**WordPressデータベースとの完全な互換性**を保ちながら、ネイティブコンパイルされた速度を提供します。

**既存のWordPressデータベースにRustPressを接続するだけで、同じコンテンツを桁違いに高速に配信できます。**

### パフォーマンス

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
git clone https://github.com/example/rustpress.git
cd rustpress
cp .env.example .env

# MySQL + RustPress を起動
docker compose up -d
```

`http://localhost:8080` でアクセスできます。

### 方法2: ソースからビルド

**必要環境:** Rust 1.88+、MySQL 8.0+（またはMariaDB 10.5+）

```bash
git clone https://github.com/example/rustpress.git
cd rustpress

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

### 開発予定
- [ ] `theme.json` パーサーによるCSS変数の完全一致
- [ ] Gutenbergブロックレンダリング（高度なブロック）
- [ ] プラグインフックシステム（`add_action`/`add_filter` のRust実装）
- [ ] ネイティブRustプラグインAPI（WASM/dylib）— Rust最適化モード
- [ ] PHPテーマ/プラグイン互換レイヤー — WordPress互換モード
- [ ] 管理画面（wp-admin）の完全対応
- [ ] マルチサイト対応

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
