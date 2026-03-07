# 指示書 #04: php-to-rust 変換エンジン担当

## あなたの役割

あなたは**php-to-rust**プロジェクトの**リード開発者**です。
PHPのコードベース全体をRustに変換するAIエージェントを構築します。

これはRustPressとは別のリポジトリです。RustPressのWordPressプラグイン/テーマ変換を動力源として、将来的にはあらゆるPHPプロジェクトのRust変換に対応します。

- リポジトリ: https://github.com/LegacyToRustProject/php-to-rust
- 言語: Rust
- LLM: Claude API（デフォルト。プロバイダーは差し替え可能に設計）
- ライセンス: MIT

---

## プロジェクトのビジョン

「正解のソースコード（PHP）が存在する限り、AIが変換をスケールさせる」

これは従来のトランスパイラ（AST→AST変換）ではない。PHPとRustは根本的に異なる言語であり、機械的な構文変換は不可能。代わりに**AIがPHPコードの意味を理解し、同じ振る舞いのRustコードを生成する**。人間のエンジニアが手で書き直すのと同じプロセスを、AIでスケールさせる。

---

## リポジトリ構成（これから作る）

```
php-to-rust/
├── Cargo.toml                   # ワークスペース定義
├── crates/
│   ├── php-parser/              # PHP ソースコード解析
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── analyzer.rs      # ファイル構造・依存関係の分析
│   │       ├── detector.rs      # PHPバージョン・フレームワーク検出
│   │       └── types.rs         # PHP AST / 構造体定義
│   ├── rust-generator/          # AI によるRustコード生成
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── llm.rs           # LLM プロバイダー trait + Claude実装
│   │       ├── prompt.rs        # 変換プロンプトテンプレート
│   │       ├── context.rs       # 変換コンテキスト（型推論結果等）
│   │       └── generator.rs     # モジュール単位の変換オーケストレーション
│   ├── verifier/                # 変換結果の自動検証
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── compiler.rs      # cargo check 自動実行
│   │       ├── comparator.rs    # PHP vs Rust 出力比較
│   │       ├── diff_report.rs   # 差分レポート生成
│   │       └── fix_loop.rs      # AI修正ループ制御
│   └── cli/                     # コマンドラインインターフェース
│       └── src/
│           └── main.rs          # エントリーポイント
├── profiles/
│   ├── wordpress/
│   │   ├── api_mappings.toml    # wp_* 関数 → Rust関数のマッピング
│   │   ├── hooks.toml           # add_action/add_filter のパターン
│   │   └── db_patterns.toml     # $wpdb → SeaORM のパターン
│   ├── laravel/                 # 将来
│   └── generic/                 # フレームワーク非依存
├── tests/
│   ├── fixtures/                # PHP入力 → 期待されるRust出力のペア
│   │   ├── simple_function/
│   │   │   ├── input.php
│   │   │   └── expected.rs
│   │   ├── wordpress_plugin/
│   │   │   ├── input/           # PHPプラグインディレクトリ
│   │   │   └── expected/        # 期待されるRustクレート
│   │   └── ...
│   └── integration/             # E2E変換テスト
└── README.md
```

---

## 現在の状態

リポジトリにはREADMEのみ。コードはゼロ。全てこれから作る。

---

## Step 1: プロジェクト初期化

```bash
# リポジトリをクローン
git clone https://github.com/LegacyToRustProject/php-to-rust.git
cd php-to-rust

# Cargoワークスペース初期化
# Cargo.toml (workspace)
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

各クレートを `cargo init` で作成する。

---

## Step 2: php-parser クレート

PHPソースコードを解析し、構造情報を抽出する。

**注意**: 完全なPHPパーサーを自作する必要はない。目的は「AIに渡すための構造情報の抽出」。

### 機能

1. **ファイルスキャン**: PHPプロジェクトの全ファイルをリストアップ
2. **依存関係分析**: `use`, `require`, `include` の解決
3. **クラス/関数の抽出**: 名前、引数、戻り値型（型ヒントがあれば）
4. **PHPバージョン検出**: `declare(strict_types=1)`, 型ヒントの有無等から推定
5. **フレームワーク検出**: WordPress（`add_action`, `wp_*`）、Laravel（`Route::`, `Eloquent`）等

### 既存ツールの活用

- [php-rust-tools/parser](https://github.com/php-rust-tools/parser): RustでPHPをパースするライブラリ。これをラップして使う。
- もしこのライブラリが不十分なら、正規表現ベースの簡易解析で十分。AIが理解するのに完全なASTは不要。

### 出力形式

```rust
pub struct PhpProject {
    pub version: PhpVersion,          // 検出されたPHPバージョン
    pub framework: Option<Framework>, // WordPress, Laravel, etc.
    pub files: Vec<PhpFile>,
}

pub struct PhpFile {
    pub path: PathBuf,
    pub classes: Vec<PhpClass>,
    pub functions: Vec<PhpFunction>,
    pub dependencies: Vec<String>,    // require/use 先
}

pub struct PhpClass {
    pub name: String,
    pub extends: Option<String>,
    pub implements: Vec<String>,
    pub methods: Vec<PhpFunction>,
    pub properties: Vec<PhpProperty>,
}

pub struct PhpFunction {
    pub name: String,
    pub params: Vec<PhpParam>,
    pub return_type: Option<String>,
    pub body: String,                 // 関数本体のソースコード（AIに渡す用）
}
```

---

## Step 3: rust-generator クレート

AIを使ってPHPコードをRustに変換する。

### LLMプロバイダー抽象化

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn generate(&self, prompt: &str) -> Result<String>;
}

pub struct ClaudeProvider {
    api_key: String,
    model: String,  // "claude-opus-4-6"
}

// 将来の拡張
// pub struct OpenAiProvider { ... }
// pub struct OllamaProvider { ... }
```

Claude APIの呼び出しには `anthropic` Rust SDKまたはHTTP直接呼び出しを使う。

### プロンプト設計

変換の品質はプロンプトで決まる。以下の構造:

```
システムプロンプト:
- あなたはPHP→Rustの変換エキスパートです
- 入力PHPコードと同じ振る舞いのRustコードを生成してください
- idiomaticなRustを書いてください（所有権、Result、trait等を適切に使用）
- [プロファイル固有の指示: WordPress APIマッピング等]

ユーザープロンプト:
- 変換対象のPHPコード
- 依存関係のコンテキスト（このクラスが使っている他のクラス/関数）
- 型推論結果（PHPの動的型から推定されたRust型）

期待する出力:
- Rustのソースコード（```rust ブロック内）
- 変換できなかった部分のTODOコメント
```

### プロファイルシステム

```rust
pub struct ConversionProfile {
    pub name: String,                           // "wordpress"
    pub api_mappings: HashMap<String, String>,   // "wp_get_posts" → "rustpress_query::get_posts"
    pub type_mappings: HashMap<String, String>,  // "WP_Post" → "rustpress_db::Post"
    pub additional_instructions: String,         // プロファイル固有のプロンプト補足
}
```

WordPressプロファイルの例:
```toml
# profiles/wordpress/api_mappings.toml
[functions]
"add_action" = "hooks.add_action"
"add_filter" = "hooks.add_filter"
"get_option" = "options::get"
"wp_query" = "rustpress_query::WpQuery::new"
"wp_insert_post" = "rustpress_db::posts::insert"
"esc_html" = "rustpress_themes::escape::html"
"wp_nonce_field" = "rustpress_auth::nonce::field"
```

---

## Step 4: verifier クレート

変換結果を自動検証するループ。これが最も重要な差別化要素。

### 検証フロー

```
1. cargo check
   → コンパイルエラー？ → AIにエラーメッセージを渡して修正させる → 1に戻る

2. cargo test（生成されたテストがあれば）
   → テスト失敗？ → AIに失敗内容を渡して修正させる → 1に戻る

3. 出力比較
   → PHPを実行した結果とRustを実行した結果を比較
   → 差分がある？ → AIに差分を渡して修正させる → 1に戻る

4. 全パス → 変換完了
```

### コンパイルチェック

```rust
pub struct CompileChecker {
    project_dir: PathBuf,
}

impl CompileChecker {
    pub fn check(&self) -> Result<CompileResult> {
        // cargo check を実行
        // stderr からエラーメッセージを抽出
        // 構造化されたエラー情報を返す
    }
}

pub enum CompileResult {
    Success,
    Errors(Vec<CompileError>),
}

pub struct CompileError {
    pub file: String,
    pub line: usize,
    pub message: String,
    pub suggestion: Option<String>,  // rustc の suggestion
}
```

### 出力比較

```rust
pub struct OutputComparator {
    php_binary: PathBuf,    // php コマンドのパス
}

impl OutputComparator {
    pub async fn compare(
        &self,
        php_file: &Path,
        rust_binary: &Path,
        test_inputs: &[TestInput],
    ) -> Result<ComparisonResult> {
        // 1. 各テスト入力に対してPHPを実行し、出力をキャプチャ
        // 2. 同じ入力でRustバイナリを実行し、出力をキャプチャ
        // 3. 出力を比較
        // 4. 差分をレポート
    }
}
```

### 修正ループ

```rust
pub struct FixLoop {
    llm: Box<dyn LlmProvider>,
    max_iterations: usize,  // 無限ループ防止（デフォルト10）
}

impl FixLoop {
    pub async fn run(
        &self,
        rust_code: &str,
        error: &str,  // コンパイルエラー or 出力差分
    ) -> Result<String> {
        // AIにエラー情報を渡して修正コードを生成
        // 修正されたRustコードを返す
    }
}
```

---

## Step 5: cli クレート

ユーザーインターフェース。

```bash
# プロジェクト全体を変換
php-to-rust convert ./my-php-project --profile wordpress --verify

# 分析のみ（変換せずにレポート）
php-to-rust analyze ./my-php-project

# 単一ファイルを変換
php-to-rust convert-file ./plugin.php --profile wordpress

# LLMプロバイダー指定
php-to-rust convert ./project --llm claude --model claude-opus-4-6
php-to-rust convert ./project --llm openai --model gpt-4o
```

### CLIの実装

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "php-to-rust")]
#[command(about = "AI-powered PHP to Rust conversion")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Convert {
        path: PathBuf,
        #[arg(long, default_value = "generic")]
        profile: String,
        #[arg(long)]
        verify: bool,
        #[arg(long, default_value = "claude")]
        llm: String,
    },
    Analyze {
        path: PathBuf,
    },
    ConvertFile {
        path: PathBuf,
        #[arg(long, default_value = "generic")]
        profile: String,
    },
}
```

---

## Step 6: テストフィクスチャの作成

変換品質を検証するためのPHP→Rustペアを作成する。

### 基本テスト（最初に作る）

```
tests/fixtures/
├── 01_hello_world/
│   ├── input.php           # <?php echo "Hello, World!";
│   └── expected_output.txt # Hello, World!
├── 02_simple_function/
│   ├── input.php           # function add($a, $b) { return $a + $b; }
│   └── expected.rs         # fn add(a: i64, b: i64) -> i64 { a + b }
├── 03_class/
│   ├── input.php           # class User { ... }
│   └── expected.rs         # struct User { ... } impl User { ... }
├── 04_array_manipulation/
│   ├── input.php           # array_map, array_filter等
│   └── expected.rs         # iter().map(), iter().filter()等
├── 05_wordpress_hook/
│   ├── input.php           # add_action('init', 'my_func');
│   └── expected.rs         # hooks.add_action("init", my_func, 10);
├── 10_wordpress_plugin/
│   ├── input/              # 完全なWPプラグインディレクトリ
│   │   ├── my-plugin.php
│   │   └── includes/
│   └── expected/           # 期待されるRustクレート
│       ├── Cargo.toml
│       └── src/
```

---

## バージョン互換

**初期ターゲット: PHP 8.x。** 型ヒントが充実しており、変換が最も容易。

| PHP Version | Priority | Notes |
|-------------|----------|-------|
| 8.0 - 8.4 | **First** | strict types, union types, named args |
| 7.4 | Second | return types, typed properties. WordPress 6.x minimum |
| 7.0 - 7.3 | Third | scalar type hints. Large installed base |
| 5.6 | Fourth | No type hints. WordPress 4.x era |
| 5.3 - 5.5 | Fifth | Namespaces, traits |

バージョン検出は `php-parser` の `detector.rs` で行い、プロンプトにバージョン情報を含めてAIの変換精度を高める。

---

## 開発ルール

### ビルド・テスト

```bash
cargo check --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace -- -D warnings
```

### コミットルール

- `feat: Add PHP class/function extraction in php-parser`
- `feat: Implement Claude API provider for rust-generator`
- `test: Add WordPress plugin conversion fixture`
- コミットは小さく、頻繁に。

### 環境変数

```bash
ANTHROPIC_API_KEY=sk-ant-...   # Claude API キー
PHP_BINARY=/usr/bin/php         # PHP実行パス（出力比較用）
```

---

## 完了条件

以下が全て満たされた時、あなたの仕事は完了です:

- [ ] `php-to-rust analyze` でPHPプロジェクトの構造レポートが出力される
- [ ] `php-to-rust convert-file` で単一PHPファイルがRustに変換される
- [ ] `php-to-rust convert` でPHPプロジェクト全体がRustクレートに変換される
- [ ] Claude APIを使った変換が動作する
- [ ] LLMプロバイダーがtrait抽象化されており、差し替え可能
- [ ] WordPressプロファイルが存在し、wp_*関数のマッピングが機能する
- [ ] cargo check による自動コンパイルチェックが動作する
- [ ] PHP/Rust出力比較による自動検証が動作する
- [ ] AI修正ループ（エラー→修正→再チェック）が動作する
- [ ] 基本テストフィクスチャ（hello world〜簡単なクラス）で変換が成功する
- [ ] WordPressプラグイン（簡単なもの1つ）の変換がE2Eで成功する
- [ ] 全ユニットテストが通る
- [ ] cargo clippy -- -D warnings が通る

---

## 自律的に動くこと

**あなたは自分で判断して進めてください。** 優先順位は指針として示していますが、状況に応じて順序を変えて構いません。

- ブロッカーがあればスキップして別の項目を進める
- 「次何をすべきか」を毎回聞かない。自分で決めて進める。
- 進捗や判断の記録はコミットメッセージとコード内コメントで残す

## 判断原則

1. **動くものを早く。** 完璧なパーサーより、動く変換パイプラインを先に作る。
2. **AIの出力を信用しない。** 必ず検証ループで確認する。
3. **プロンプトが全て。** 変換品質の80%はプロンプト設計で決まる。プロンプトの改善に時間を使え。
4. **RustPressが最初の顧客。** WordPressプラグイン変換が動くことが最優先。


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
