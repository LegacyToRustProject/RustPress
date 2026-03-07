# 指示書 #05: cobol-to-rust 変換エンジン担当

## あなたの役割

あなたは**cobol-to-rust**プロジェクトの**リード開発者**です。
COBOLのコードベース全体をRustに変換するAIエージェントを構築します。

- リポジトリ: https://github.com/LegacyToRustProject/cobol-to-rust
- 言語: Rust
- LLM: Claude API（デフォルト。プロバイダーは差し替え可能に設計）
- ライセンス: MIT

---

## なぜCOBOL

- 世界のATM取引の95%がCOBOLを経由
- 推定2400億行のCOBOLが本番稼働中
- COBOL開発者の平均年齢は60代。引退が加速中。後継者がいない
- 仕様書は失われている。ソースコードが唯一の仕様
- 銀行・保険・政府は移行したいが、人間がやるとリスクが高すぎて手を出せない

---

## アーキテクチャ

php-to-rustと同じパターン: **解析 → AI変換 → 検証ループ**

```
cobol-to-rust/
├── Cargo.toml
├── crates/
│   ├── cobol-parser/            # COBOL ソースコード解析
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── analyzer.rs      # DIVISION/SECTION/PARAGRAPH 構造解析
│   │       ├── copybook.rs      # COPY文（共有データ定義）の解決
│   │       ├── data_division.rs # DATA DIVISIONのPIC句パース
│   │       └── types.rs         # COBOL構造体定義
│   ├── rust-generator/          # AI によるRustコード生成
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── llm.rs           # LLMプロバイダー trait + Claude実装
│   │       ├── prompt.rs        # COBOL専用変換プロンプト
│   │       ├── decimal.rs       # 固定小数点数の変換戦略
│   │       └── generator.rs     # 変換オーケストレーション
│   ├── verifier/                # 自動検証
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── compiler.rs      # cargo check
│   │       ├── comparator.rs    # COBOL vs Rust 出力比較
│   │       └── fix_loop.rs      # AI修正ループ
│   └── cli/
│       └── src/
│           └── main.rs
├── profiles/
│   ├── ibm-mainframe/           # IBM Enterprise COBOL固有パターン
│   ├── microfocus/              # Micro Focus COBOL固有パターン
│   └── generic/                 # 標準COBOL-85
└── tests/
    └── fixtures/
        ├── 01_hello_world/
        ├── 02_pic_clause/
        ├── 03_perform_loop/
        ├── 04_file_io/
        └── 05_batch_program/
```

---

## COBOL固有の課題と変換戦略

### PIC句（データ型定義）

COBOLの最大の特徴。全変数に桁数・小数点位置が固定で定義される。

```cobol
01  WS-AMOUNT     PIC 9(7)V99.       *> 7桁整数 + 2桁小数
01  WS-NAME       PIC X(30).          *> 30文字の文字列
01  WS-FLAG       PIC 9.              *> 1桁の数値フラグ
01  WS-NEGATIVE   PIC S9(5)V99.       *> 符号付き5桁 + 2桁小数
```

**変換戦略**:
```rust
// PIC 9(7)V99 → rust_decimal::Decimal or 独自型
let ws_amount: Decimal = Decimal::new(0, 2); // 小数2桁

// PIC X(30) → String（または固定長配列）
let ws_name: String = String::with_capacity(30);

// PIC 9 → u8
let ws_flag: u8 = 0;

// PIC S9(5)V99 → Decimal（符号付き）
let ws_negative: Decimal = Decimal::new(0, 2);
```

**重要**: 金融計算では浮動小数点（f64）は絶対に使わない。`rust_decimal` クレートを使うこと。

### PERFORM（ループ・サブルーチン呼び出し）

```cobol
PERFORM PROCESS-RECORD THRU PROCESS-EXIT
    VARYING WS-IDX FROM 1 BY 1
    UNTIL WS-IDX > WS-COUNT.
```

**変換**:
```rust
for ws_idx in 1..=ws_count {
    process_record();
}
```

### ファイル I/O

COBOLのバッチ処理の核心。ISAM/VSAM/シーケンシャルファイル。

```cobol
SELECT INPUT-FILE ASSIGN TO "DATA.DAT"
    ORGANIZATION IS SEQUENTIAL.

READ INPUT-FILE INTO WS-RECORD
    AT END SET WS-EOF TO TRUE.
```

**変換戦略**:
```rust
use std::io::BufRead;
let file = File::open("DATA.DAT")?;
for line in BufReader::new(file).lines() {
    let record = parse_record(&line?)?;
    // process...
}
```

### REDEFINES（メモリ再解釈）

```cobol
01  WS-DATE.
    05  WS-YEAR    PIC 9(4).
    05  WS-MONTH   PIC 9(2).
    05  WS-DAY     PIC 9(2).
01  WS-DATE-NUM REDEFINES WS-DATE PIC 9(8).
```

**変換**:
```rust
struct WsDate {
    year: u16,
    month: u8,
    day: u8,
}

impl WsDate {
    fn as_number(&self) -> u32 {
        self.year as u32 * 10000 + self.month as u32 * 100 + self.day as u32
    }
}
```

### COPY文（共有データ定義）

```cobol
COPY CUSTOMER-RECORD.
```

COPYBOOKファイルを解決し、インライン展開してからAIに渡す。

---

## バージョン互換

| COBOL Standard | Priority | Notes |
|----------------|----------|-------|
| COBOL-85 | **First** | 90%+のプロダクション環境。最もシンプル。 |
| VS COBOL II (IBM) | **First** | IBM メインフレーム標準 |
| Enterprise COBOL | Second | 現代IBM方言、一部OOP |
| Micro Focus | Second | 分散環境で一般的 |
| COBOL 2002/2014 | Third | OOP拡張。レガシーでは稀 |

---

## テストフィクスチャ

```
tests/fixtures/
├── 01_hello_world/
│   ├── input.cob          # DISPLAY "HELLO WORLD" STOP RUN.
│   └── expected_output.txt
├── 02_pic_arithmetic/
│   ├── input.cob          # PIC句を使った計算
│   └── expected_output.txt # 小数精度が一致すること
├── 03_perform_loop/
│   ├── input.cob          # PERFORM VARYING
│   └── expected_output.txt
├── 04_file_read/
│   ├── input.cob          # シーケンシャルファイル読み取り
│   ├── test_data.dat      # テスト入力データ
│   └── expected_output.txt
├── 05_copybook/
│   ├── input.cob
│   ├── CUSTOMER.cpy       # COPYBOOKファイル
│   └── expected_output.txt
└── 10_batch_program/
    ├── input/              # 複数ファイルのバッチプログラム
    └── expected/           # 期待されるRustプロジェクト
```

---

## 開発ルール

### ビルド・テスト

```bash
cargo check --workspace
cargo test --workspace
cargo fmt --all
cargo clippy --workspace -- -D warnings
```

### 環境変数

```bash
ANTHROPIC_API_KEY=sk-ant-...
COBOL_COMPILER=cobc              # GnuCOBOL（出力比較用）
```

**GnuCOBOL**: テスト環境でCOBOLを実行するために必要。`apt install gnucobol` でインストール。

---

## 完了条件

- [ ] `cobol-to-rust analyze` でCOBOLプロジェクトの構造レポートが出力される
- [ ] `cobol-to-rust convert` でCOBOLプログラムがRustに変換される
- [ ] PIC句が `rust_decimal` に正しく変換される（金融精度を保証）
- [ ] PERFORM文がRustのループ/関数呼び出しに変換される
- [ ] ファイルI/OがRustのstd::io/std::fsに変換される
- [ ] COPY文（COPYBOOK）が解決・展開される
- [ ] cargo check + 出力比較の検証ループが動作する
- [ ] 基本テストフィクスチャで変換成功
- [ ] 全テスト通過、clippy通過

---

## 自律的に動くこと

**あなたは自分で判断して進めてください。**

- ブロッカーがあればスキップして別の項目を進める
- 「次何をすべきか」を毎回聞かない。自分で決めて進める
- 進捗や判断の記録はコミットメッセージとコード内コメントで残す

## 判断原則

1. **金融精度は絶対。** COBOLは銀行で使われる。1円の誤差も許されない。浮動小数点は禁止。
2. **動くものを早く。** COBOL-85の基本構文で動く変換を先に作る。
3. **GnuCOBOLで検証。** 出力比較はGnuCOBOLコンパイラで元のCOBOLを実行し、Rustの出力と比較する。
4. **php-to-rustと設計を揃える。** LLMプロバイダー、検証ループの設計パターンは共通化する。
