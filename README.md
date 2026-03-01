# 🦀 RustPress

**The Next Generation CMS: WordPress Ecosystem meets Rust Performance.**

[![License: GPL v2](https://img.shields.io/badge/License-GPL%20v2-blue.svg)](https://www.gnu.org/licenses/old-licenses/gpl-2.0.en.html)
[![Rust: 1.75+](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Status: Concept/PoC](https://img.shields.io/badge/Status-Concept%2FPoC-yellow.svg)](#)

RustPressは、世界で最も愛されているCMSである**WordPressの哲学と資産**を、現代で最も安全かつ高速な言語である**Rust**へ移植する野心的なプロジェクトです。

---

## 🚀 Vision: "Save WordPress with Rust"
現在、WordPressはパフォーマンスとセキュリティの限界に直面しています。RustPressは、PHPをRustへ「進化」させることで、以下の未来を提供します。

* **100x Performance:** インタプリタを排除し、ネイティブバイナリによる圧倒的な応答速度。
* **Memory Safety:** Rustの所有権システムにより、WPサイトを悩ませる脆弱性の多くを構造的に排除。
* **AI-Driven Migration:** AI（LLM）の力を1000%活用し、数万件の既存PHPプラグインをRustコードへ自動変換・最適化。

---

## 🛠 Tech Stack
* **Core:** [Axum](https://github.com/tokio-rs/axum) (High-performance Web Framework)
* **Async Runtime:** [Tokio](https://tokio.rs/)
* **ORM:** [SeaORM](https://www.sea-ql.org/SeaORM/) (Compatible with existing `wp_` database schema)
* **Template:** [Tera](https://keats.github.io/tera/) (Jinja2-like, optimized for WP theme developers)
* **Plugin Engine:** AI-powered Transpiler & WebAssembly (Wasm)

---

## 🏗 Key Features (Roadmap)
- [ ] **WP-Hook-System (Rust-Native):** `add_action` / `apply_filters` の完全再現。
- [ ] **Database Compatibility:** 既存のWordPress DBをそのまま接続・利用可能。
- [ ] **AI Transpiler:** PHPプラグインを解析し、IdiomaticなRustコードへ自動変換。
- [ ] **Wasm Sidecar:** 未変換のPHPプラグインをサンドボックス内で安全に実行。

---

## 💻 Getting Started (PoC)
現在、コアエンジンのプロトタイプを開発中です。

```bash
# Clone the repository
git clone [https://github.com/your-username/rustpress.git](https://github.com/your-username/rustpress.git)

# Build the core
cd rustpress
cargo build --release

# Run RustPress Server (Connects to your existing WP database)
./target/release/rustpress-core --db-url mysql://user:pass@localhost/wp_db