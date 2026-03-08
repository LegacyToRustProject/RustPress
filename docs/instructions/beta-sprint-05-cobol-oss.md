# ベータスプリント指示書 — #05 cobol-to-rust OSS変換テスト

## このスプリントのミッション

変換エンジン（cobol-to-rust）を**実際のCOBOLプロジェクト**で試し、変換成功率・精度をレポートにまとめる。

COBOLは「仕様書が失われ、ソースコードが唯一の仕様」という状況が多い。変換エンジンがどこまで自動化でき、どこで人間が必要かを明確にする。

---

## 対象プロジェクト（難易度順）

### Phase 1: GnuCOBOL サンプル（最小確認）

**なぜ**: GnuCOBOLリポジトリには学習用・テスト用のCOBOLサンプルが豊富。構造がシンプルで変換パイプラインのE2E確認に最適。

```bash
# GnuCOBOLのサンプルを取得
git clone https://github.com/openmainframeproject/cobol-programming-course.git \
    ./test-projects/cobol-programming-course

# シンプルなサンプルから始める
ls ./test-projects/cobol-programming-course/COBOL/
```

**ターゲットファイル（難易度低順）**:
1. `HELLO.cbl` — DISPLAY "Hello, World!"
2. `CBL0001.cbl` — 変数・演算の基本
3. `CBL0002.cbl` — 条件分岐（IF/EVALUATE）
4. `CBL0003.cbl` — ループ（PERFORM VARYING）
5. `CBL0004.cbl` — ファイルI/O（シーケンシャル）

---

### Phase 2: COBOL-IT サンプル（実務型）

**なぜ**: 実際の業務システムに近いパターン（マスターファイル更新、帳票出力等）。

```bash
# OpenMainframeProjectのCOBOLコース
# または以下のサンプルリポジトリ
git clone https://github.com/IBM/cobol-programming-course.git \
    ./test-projects/ibm-cobol-course

# バッチプログラム例
ls ./test-projects/ibm-cobol-course/
```

---

### Phase 3: 金融・バッチ処理パターン（難関）

実際の銀行系COBOLに近いパターンを自分で作成してテスト:

```cobol
*> test-projects/batch-update/BATCHUPD.cbl
IDENTIFICATION DIVISION.
PROGRAM-ID. BATCHUPD.

DATA DIVISION.
WORKING-STORAGE SECTION.
01  WS-AMOUNT     PIC 9(9)V99 VALUE ZEROS.
01  WS-BALANCE    PIC S9(11)V99 VALUE ZEROS.
01  WS-ACCT-NO    PIC 9(10) VALUE ZEROS.
01  WS-EOF        PIC 9 VALUE 0.

01  WS-TRANSACTION.
    05 TR-ACCT    PIC 9(10).
    05 TR-TYPE    PIC X.
    05 TR-AMOUNT  PIC 9(9)V99.

FILE SECTION.
FD  INPUT-FILE.
01  INPUT-RECORD  PIC X(21).

FD  OUTPUT-FILE.
01  OUTPUT-RECORD PIC X(80).

PROCEDURE DIVISION.
MAIN-PARA.
    OPEN INPUT INPUT-FILE
         OUTPUT OUTPUT-FILE
    PERFORM READ-PROCESS UNTIL WS-EOF = 1
    CLOSE INPUT-FILE OUTPUT-FILE
    STOP RUN.

READ-PROCESS.
    READ INPUT-FILE INTO WS-TRANSACTION
        AT END MOVE 1 TO WS-EOF
    END-READ
    IF WS-EOF = 0
        PERFORM PROCESS-RECORD
    END-IF.

PROCESS-RECORD.
    IF TR-TYPE = 'D'
        ADD TR-AMOUNT TO WS-BALANCE
    ELSE IF TR-TYPE = 'W'
        SUBTRACT TR-AMOUNT FROM WS-BALANCE
    END-IF
    MOVE TR-ACCT TO WS-ACCT-NO
    PERFORM WRITE-OUTPUT.

WRITE-OUTPUT.
    MOVE SPACES TO OUTPUT-RECORD
    STRING "ACCT:" DELIMITED SPACE
           WS-ACCT-NO DELIMITED SPACE
           " BAL:" DELIMITED SPACE
           WS-BALANCE DELIMITED SPACE
           INTO OUTPUT-RECORD
    WRITE OUTPUT-RECORD.
```

---

## 作業手順

### Step 1: 変換エンジンのセットアップ確認

```bash
cd ~/cobol-to-rust
cargo build --release
./target/release/cobol-to-rust --help

# GnuCOBOLのインストール確認（出力比較に必要）
which cobc || apt install gnucobol -y
cobc --version
```

---

### Step 2: HELLO.cbl で E2Eパイプライン確認

```bash
# 変換実行
./target/release/cobol-to-rust convert-file \
    ./test-projects/cobol-programming-course/COBOL/HELLO.cbl \
    --output ./output/hello-cobol/

# コンパイル確認
cd ./output/hello-cobol && cargo check 2>&1 | tee ../../results/hello-cobol-check.txt

# GnuCOBOLでオリジナルを実行し出力を記録
cobc -x ./test-projects/cobol-programming-course/COBOL/HELLO.cbl -o /tmp/hello-cobol-orig
/tmp/hello-cobol-orig > /tmp/hello-cobol-orig-output.txt

# Rustで実行して比較
cd ./output/hello-cobol && cargo run > /tmp/hello-cobol-rust-output.txt
diff /tmp/hello-cobol-orig-output.txt /tmp/hello-cobol-rust-output.txt
```

---

### Step 3: PIC句変換の精度検証

最も重要なのが**数値精度**。特に小数点を含むPIC句。

```bash
# テスト用COBOLプログラムを作成
cat > /tmp/pic-test.cbl << 'EOF'
IDENTIFICATION DIVISION.
PROGRAM-ID. PICTEST.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-A PIC 9(5)V99 VALUE 12345.67.
01 WS-B PIC 9(5)V99 VALUE 987.89.
01 WS-C PIC S9(7)V99.
PROCEDURE DIVISION.
    COMPUTE WS-C = WS-A + WS-B
    DISPLAY WS-C
    STOP RUN.
EOF

# 変換
./target/release/cobol-to-rust convert-file /tmp/pic-test.cbl --output ./output/pic-test/

# 精度比較
cobc -x /tmp/pic-test.cbl -o /tmp/pic-test-orig && /tmp/pic-test-orig
cd ./output/pic-test && cargo run
# 出力が一致することを確認
```

---

### Step 4: バッチプログラムの変換

```bash
# Phase 3のバッチプログラムを変換
./target/release/cobol-to-rust convert-file \
    ./test-projects/batch-update/BATCHUPD.cbl \
    --output ./output/batch-update/

# テストデータ作成
echo "12345678901D000000099" > /tmp/test-transactions.dat
echo "12345678901W000000050" >> /tmp/test-transactions.dat

# オリジナルCOBOL実行
cobc -x ./test-projects/batch-update/BATCHUPD.cbl -o /tmp/batchupd-orig
/tmp/batchupd-orig < /tmp/test-transactions.dat > /tmp/orig-output.txt

# Rust実行して比較
cd ./output/batch-update && cargo run < /tmp/test-transactions.dat > /tmp/rust-output.txt
diff /tmp/orig-output.txt /tmp/rust-output.txt
```

---

### Step 5: 変換失敗パターンの分析

変換できなかったCOBOLパターンを分類:

```markdown
## 未対応パターン（例）

| COBOLパターン | 出現頻度 | 対応難度 | 対応方針 |
|---|---|---|---|
| REDEFINES句 | 高 | 高 | struct + From実装で対処 |
| COPY文（COPYBOOK解決） | 高 | 中 | インライン展開で対処 |
| COMPUTE（複雑な式） | 中 | 低 | rust_decimalのAPIにマッピング |
| STRING/UNSTRING | 中 | 中 | Rustのformat!/split で対処 |
| INSPECT | 低 | 高 | AI変換に頼る |
| GO TO（スパゲティ） | 低 | 高 | ループ再構成が必要 |
```

---

### Step 6: 内部テスト — バイナリ挙動比較（hexdump精度）

COBOLは出力ファイルのバイト列が1ビットも違わないことが本番相当の基準。

```bash
# シャドウ実行スクリプト
cat > ./scripts/shadow-run.sh << 'EOF'
#!/bin/bash
COBOL_BIN="$1"
RUST_BIN="$2"
INPUT_FILE="$3"

# 出力ファイルをクリア
rm -f MASTER.DAT OUTPUT.DAT

# COBOL版を実行
"$COBOL_BIN" < "$INPUT_FILE"
hexdump -C MASTER.DAT  > /tmp/cobol-master.hex 2>/dev/null
hexdump -C OUTPUT.DAT  > /tmp/cobol-output.hex 2>/dev/null
cp MASTER.DAT /tmp/cobol-master.dat
rm -f MASTER.DAT OUTPUT.DAT

# Rust版を実行
"$RUST_BIN" < "$INPUT_FILE"
hexdump -C MASTER.DAT  > /tmp/rust-master.hex 2>/dev/null
hexdump -C OUTPUT.DAT  > /tmp/rust-output.hex 2>/dev/null

# 1バイト精度で比較
diff /tmp/cobol-master.hex /tmp/rust-master.hex && echo "PASS (MASTER)" || echo "FAIL (MASTER)"
diff /tmp/cobol-output.hex /tmp/rust-output.hex && echo "PASS (OUTPUT)" || echo "FAIL (OUTPUT)"
EOF
chmod +x ./scripts/shadow-run.sh
```

**大量データでのシャドウ実行:**

```bash
# 100万件のランダムトランザクション生成
cargo run --bin generate-test-data -- --count 1000000 --seed 42 > /tmp/big-transactions.dat

./scripts/shadow-run.sh \
    /tmp/cobol-batchupd \
    ./output/batch-update/target/release/batch-update \
    /tmp/big-transactions.dat
```

---

### Step 7: 外部テスト — ネットワーク経由ブラックボックス比較

COBOLが帳票サービス・バッチAPIとして動いている場合の外部比較。

```bash
# COBOLをWebラッパー経由で呼び出す（gnucobol + CGI or socket）
# Rust版をAxumサービスとして起動

# 同じリクエストを両方に送り、レスポンスをdiff
TRANSACTIONS='[{"acct":"1234567890","type":"D","amount":99.99}]'

curl -s -X POST http://localhost:8081/process \
    -H "Content-Type: application/json" \
    -d "$TRANSACTIONS" > /tmp/cobol-api-response.json

curl -s -X POST http://localhost:8080/process \
    -H "Content-Type: application/json" \
    -d "$TRANSACTIONS" > /tmp/rust-api-response.json

diff /tmp/cobol-api-response.json /tmp/rust-api-response.json \
    && echo "PASS" || echo "FAIL"
```

---

### Step 8: 収束分析 — プラトー検出

**`crates/verifier/src/convergence.rs` を実装:**

```rust
pub struct ConvergenceTracker {
    pub batch_size: usize,
    pub window: usize,
    pub threshold: f64,
    history: Vec<BatchResult>,
}

#[derive(Clone)]
pub struct BatchResult {
    pub batch_no: usize,
    pub tests_run: usize,
    pub new_divergences: usize,
    pub cumulative_divergences: usize,
    pub rate: f64,
}

impl ConvergenceTracker {
    pub fn record_batch(&mut self, new_divergences: usize) {
        let cumulative = self.history.last()
            .map(|b| b.cumulative_divergences).unwrap_or(0) + new_divergences;
        self.history.push(BatchResult {
            batch_no: self.history.len() + 1,
            tests_run: (self.history.len() + 1) * self.batch_size,
            new_divergences,
            cumulative_divergences: cumulative,
            rate: new_divergences as f64 / self.batch_size as f64,
        });
    }

    pub fn is_plateau(&self) -> bool {
        if self.history.len() < self.window { return false; }
        self.history.iter().rev().take(self.window)
            .all(|b| b.rate < self.threshold)
    }

    pub fn report(&self) -> String {
        let mut out = String::from(
            "| バッチ | 累積テスト数 | 新規差分 | 発見率 | 判定 |\n\
             |--------|------------|---------|--------|------|\n"
        );
        for (i, b) in self.history.iter().enumerate() {
            let is_last = i == self.history.len() - 1;
            let judgment = if is_last && self.is_plateau() { "✅ PLATEAU" } else { "継続" };
            out.push_str(&format!(
                "| {} | {:>12} | {:>8} | {:>6.1}% | {} |\n",
                b.batch_no, b.tests_run, b.new_divergences, b.rate * 100.0, judgment
            ));
        }
        out
    }
}
```

**実行（COBOLは金融精度が命なので閾値を厳しく設定）:**

```bash
./target/release/cobol-to-rust shadow \
    --original /tmp/cobol-batchupd \
    --converted ./output/batch-update/target/release/batch-update \
    --batch-size 10000 --window 5 --threshold 0.0001 \
    | tee results/convergence-batchupd.txt
```

**期待する出力（COBOLは厳しい基準）:**

```
| バッチ | 累積テスト数 | 新規差分 | 発見率 | 判定 |
|--------|------------|---------|--------|------|
| 1      |       10,000 |        8 |   0.1% | 継続 |
| 2      |       20,000 |        2 |   0.0% | 継続 |
| 3      |       30,000 |        0 |   0.0% | 継続 |
| 4      |       40,000 |        0 |   0.0% | 継続 |
| 5      |       50,000 |        0 |   0.0% | ✅ PLATEAU |

→ 50,000件でプラトー到達。金融精度での完全一致を統計的に証明済み。
```

---

### Step 9: 変換エンジン改善のPR作成

```bash
cd ~/cobol-to-rust
git checkout main && git pull origin main
git checkout -b feat/oss-test-improvements

# PIC句変換の精度向上
# crates/cobol-parser/src/data_division.rs を改善

# PERFORM変換パターンの追加
# crates/rust-generator/src/prompt.rs を更新

git add -p
git commit -m "feat: Improve PIC clause conversion accuracy based on OSS testing"
git push -u origin feat/oss-test-improvements
```

---

## レポート形式

`results/oss-conversion-report.md` に以下の形式で出力:

```markdown
# cobol-to-rust OSS変換テスト結果

実施日: YYYY-MM-DD

## サマリー

| プログラム | 行数 | 変換完走 | cargo check | 出力一致 | TODO数 |
|---|---|---|---|---|---|
| HELLO.cbl | ~20 | ✅ | ✅ | ✅ | 0 |
| CBL0001.cbl | ~80 | ✅ | ✅ | ✅ | 2 |
| CBL0004.cbl | ~150 | ✅ | ⚠️ | ❌ | 8 |
| BATCHUPD.cbl | ~60 | ✅ | ✅ | ✅ | 3 |

## 数値精度テスト結果

| PIC句 | COBOL出力 | Rust出力 | 一致 |
|---|---|---|---|
| PIC 9(5)V99 (12345.67 + 987.89) | 13333.56 | 13333.56 | ✅ |

## 未対応パターン一覧

（上記テーブルを記載）

## 変換エンジン改善提案

1. **優先度高**: REDEFINES句のサポート
2. **優先度中**: COPY文のインライン展開
3. **優先度低**: GO TO の再構成ロジック
```

---

## 完了条件

- [ ] GnuCOBOLサンプル5本以上の変換を試みる
- [ ] PIC句の数値精度が100%一致することを確認
- [ ] バッチプログラムの入出力比較が通る
- [ ] `results/oss-conversion-report.md` が出力される
- [ ] 変換エンジンの改善点を特定し、少なくとも1件のPRを作成
- [ ] `cargo test --workspace` が通る
- [ ] `cargo clippy --workspace -- -D warnings` が通る

---

## ブランチ

```bash
cd ~/cobol-to-rust
git checkout main && git pull origin main
git checkout -b feat/oss-test-improvements
```

PR作成後、QA #09 レビュー → オーナー承認でマージ。

---

## 判断の優先順位

1. **金融精度は絶対。** PIC句の数値変換でf64を使っていたら即修正。rust_decimal必須。
2. **GnuCOBOLで出力を検証。** 見た目で動いていても出力が違えば失敗。
3. **GO TOはスキップしてよい。** スパゲッティCOBOLの解析は後回し。PERFORM/PERFORMループを先に完成させる。
