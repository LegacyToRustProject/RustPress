# 指示書 #08: perl-to-rust 変換エンジン担当

## あなたの役割

あなたは**perl-to-rust**プロジェクトの**リード開発者**です。
Perl 5のコードベースをRustに変換するAIエージェントを構築します。

- リポジトリ: https://github.com/LegacyToRustProject/perl-to-rust
- 言語: Rust
- LLM: Claude API（デフォルト。差し替え可能に設計）
- ライセンス: MIT

---

## なぜPerl

- 通信・金融・バイオインフォマティクスの基幹にPerl 5が大量残存
- 「Only Perl can parse Perl」— 人間にとって最も読みにくい言語の一つ
- 開発者が年々減少。コードは動いているが誰もメンテナンスできない
- AIにとってPerlの構文的複雑さは障害にならない。振る舞いを理解し、Rustで書き直す

---

## アーキテクチャ

```
perl-to-rust/
├── Cargo.toml
├── crates/
│   ├── perl-parser/             # Perl ソースコード解析
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── analyzer.rs      # モジュール構造分析
│   │       ├── cpan.rs          # CPAN依存関係の検出・マッピング
│   │       ├── regex.rs         # Perl正規表現の解析
│   │       └── types.rs
│   ├── rust-generator/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── llm.rs
│   │       ├── prompt.rs        # Perl専用変換プロンプト
│   │       └── generator.rs
│   ├── verifier/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── compiler.rs
│   │       ├── comparator.rs    # Perl vs Rust 出力比較
│   │       └── fix_loop.rs
│   └── cli/
│       └── src/
│           └── main.rs
├── profiles/
│   ├── telecom/                 # 通信系パターン
│   ├── bioinformatics/          # バイオ系パターン（BioPerl等）
│   └── generic/
├── cpan-mappings/               # CPANモジュール → Rustクレート対応表
│   └── mappings.toml
└── tests/
    └── fixtures/
```

---

## Perl固有の課題と変換戦略

### 動的型付け（$, @, %）

```perl
my $scalar = "hello";      # スカラー
my @array = (1, 2, 3);     # 配列
my %hash = (key => "val"); # ハッシュ
$scalar = 42;               # 型が変わる！
```

**変換戦略**:
```rust
// AIがコンテキストから型を推論
let scalar: String = "hello".to_string();
let array: Vec<i32> = vec![1, 2, 3];
let hash: HashMap<String, String> = HashMap::from([("key".into(), "val".into())]);

// 型が変わるケース → enum
enum PerlValue {
    Str(String),
    Int(i64),
    Float(f64),
    Array(Vec<PerlValue>),
    Hash(HashMap<String, PerlValue>),
}
```

ただし、多くの場合AIがコンテキストから単一型を推論できる。PerlValue enumは最後の手段。

### 正規表現

Perlの正規表現はRustの`regex`クレートでほぼ互換。

```perl
if ($line =~ /^(\d{4})-(\d{2})-(\d{2})$/) {
    my ($year, $month, $day) = ($1, $2, $3);
}

$text =~ s/foo/bar/g;
```

**変換**:
```rust
use regex::Regex;

let re = Regex::new(r"^(\d{4})-(\d{2})-(\d{2})$").unwrap();
if let Some(caps) = re.captures(&line) {
    let year = &caps[1];
    let month = &caps[2];
    let day = &caps[3];
}

let text = re.replace_all(&text, "bar").to_string();
```

**注意**: Perlの一部正規表現機能（後方参照、条件パターン、コードブロック埋め込み）はRustの`regex`では未対応。`fancy-regex`クレートで対応可能。

### コンテキスト感度（スカラー/リスト）

```perl
my @arr = (1, 2, 3);
my $count = @arr;        # スカラーコンテキスト → 要素数 (3)
my @copy = @arr;          # リストコンテキスト → コピー
```

**変換**: AIが文脈を判断し、明示的なRust呼び出しに変換。
```rust
let arr = vec![1, 2, 3];
let count = arr.len();        // スカラーコンテキスト
let copy = arr.clone();       // リストコンテキスト
```

### 暗黙の変数（$_）

```perl
for (@items) {
    print if /pattern/;    # $_ が暗黙的に使われている
}
```

**変換**: 暗黙を明示に展開。
```rust
for item in &items {
    if regex.is_match(item) {
        println!("{}", item);
    }
}
```

### CPANモジュール → Rustクレートマッピング

```toml
# cpan-mappings/mappings.toml
[modules]
"LWP::UserAgent" = "reqwest"
"JSON" = "serde_json"
"DBI" = "sqlx"
"DateTime" = "chrono"
"File::Path" = "std::fs"
"Getopt::Long" = "clap"
"Test::More" = "# built-in test framework"
"Moose" = "# struct + impl + traits"
"Try::Tiny" = "# Result<T,E> / anyhow"
"Log::Log4perl" = "tracing"
"XML::LibXML" = "quick-xml"
"Text::CSV" = "csv"
"MIME::Base64" = "base64"
"Digest::SHA" = "sha2"
"IO::Socket::SSL" = "rustls"
```

### オブジェクト指向（bless）

```perl
package Dog;
sub new {
    my ($class, %args) = @_;
    return bless { name => $args{name} }, $class;
}
sub speak { return "Woof!"; }
```

**変換**:
```rust
struct Dog {
    name: String,
}

impl Dog {
    fn new(name: String) -> Self {
        Self { name }
    }
    fn speak(&self) -> &str {
        "Woof!"
    }
}
```

---

## バージョン互換

| Perl Version | Priority | Notes |
|---|---|---|
| 5.26 - 5.40 | **First** | 現代Perl 5 |
| 5.16 - 5.24 | Second | 企業の安定版 |
| 5.10 - 5.14 | Third | say, given/when。レガシー境界 |
| 5.8 | Fourth | Unicode時代。通信系に残存 |
| 5.6 | Fifth | 非常に古い。テレコム |

**Perl 6 (Raku) は対象外。** 全く別の言語。

---

## テストフィクスチャ

```
tests/fixtures/
├── 01_hello_world/
├── 02_regex/
├── 03_hash_manipulation/
├── 04_file_io/
├── 05_oop_bless/
├── 06_cpan_module_usage/
├── 07_one_liner_expansion/
├── 08_context_sensitivity/
├── 09_implicit_variables/
└── 10_small_project/
    ├── input/
    │   ├── lib/
    │   │   └── MyModule.pm
    │   ├── script.pl
    │   └── cpanfile
    └── expected/
        ├── Cargo.toml
        └── src/
```

---

## 完了条件

- [ ] `perl-to-rust analyze` でPerlプロジェクトの構造レポート出力（CPANモジュール検出含む）
- [ ] `perl-to-rust convert` でPerlプロジェクトがRustに変換される
- [ ] 動的型付け→静的型推論が動作する
- [ ] Perl正規表現→regex/fancy-regex変換が動作する
- [ ] コンテキスト感度（スカラー/リスト）が正しく変換される
- [ ] 暗黙変数（$_等）が明示変数に展開される
- [ ] CPANモジュール→Rustクレートマッピングが機能する
- [ ] cargo check + 出力比較の検証ループが動作する
- [ ] 基本テストフィクスチャで変換成功
- [ ] 全テスト通過、clippy通過

---

## 自律的に動くこと

**あなたは自分で判断して進めてください。**

## 判断原則

1. **暗黙を明示にする。** Perlの最大の問題は暗黙の挙動。Rustでは全てを明示的に書く。
2. **正規表現の精度が命。** Perlコードの多くは正規表現で構成される。ここの変換精度が全体の品質を決める。
3. **CPANマッピングを充実させる。** 実プロジェクトではCPANモジュールへの依存が大きい。マッピング表の充実が変換成功率に直結する。
4. **php-to-rustと設計を揃える。** LLMプロバイダー、検証ループの設計パターンは共通化する。


---

## ビジネスモデル（次フェーズ）

この変換エンジンは2つの形態で提供される。現時点ではCLI開発に集中し、ホスト型は次フェーズで実装する。

| | セルフホスト (CLI) | ホスト型 (Web UI) |
|---|---|---|
| 料金 | 無料（OSS） | Stripe課金 |
| APIキー | ユーザーが自分で用意 | サービス側が提供 |
| 対象 | 開発者 | 非開発者 |

**現時点での影響:**
- APIキーはハードコードしない（環境変数 or 設定ファイル）
- APIキーをログに出力しない
- LlmProviderトレイトの差し替え可能な設計を維持する（将来のマルチプロバイダー対応）

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
