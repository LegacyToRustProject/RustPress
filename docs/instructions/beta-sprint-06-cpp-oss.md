# ベータスプリント指示書 — #06 cpp-to-rust OSS変換テスト

## このスプリントのミッション

変換エンジン（cpp-to-rust）を**実際のOSSプロジェクト**で試し、変換成功率・安全性向上効果をレポートにまとめる。

C/C++→Rustの最大の価値は**メモリ安全性の向上**。変換後のRustコードが`unsafe`を使わず、所有権システムに沿っているかを重視する。

---

## 対象プロジェクト（難易度順）

### Phase 1: cJSON（純C・最小）

**なぜ**: ~750行の純C。依存なし。JSONパーサーという明確な仕様がある。出力比較が容易。

```bash
git clone https://github.com/DaveGamble/cJSON.git ./test-projects/cjson
wc -l ./test-projects/cjson/cJSON.c  # ~750行
```

**変換の焦点**:
- `malloc/free` → `Vec<u8>` / `Box<T>`
- `NULL`チェック → `Option<T>`
- `char*` 文字列 → `String` / `&str`
- 連結リスト（`struct cJSON { struct cJSON *next, *prev; ... }`）→ `Vec<JsonNode>` or `Rc<RefCell<...>>`

---

### Phase 2: libcsv（C・ファイル処理）

**なぜ**: ~1000行。CSVパーサー。コールバックベースのAPIパターンを試す。

```bash
git clone https://github.com/rgamble/libcsv.git ./test-projects/libcsv
```

**変換の焦点**:
- 関数ポインタ（コールバック）→ `Fn` trait / クロージャ
- `void*` 汎用ポインタ → ジェネリクス

---

### Phase 3: TinyXML2（C++・クラス階層）

**なぜ**: ~3600行のC++。クラス継承・仮想関数・イテレータ・例外処理が含まれる。C++の典型的なパターンを試す。

```bash
git clone https://github.com/leethomason/tinyxml2.git ./test-projects/tinyxml2
wc -l ./test-projects/tinyxml2/tinyxml2.cpp  # ~3600行
```

**変換の焦点**:
- 仮想関数（`virtual void Visit()`) → `trait` + `dyn`
- クラス継承 → `trait` + 合成
- 例外（`TINYXML2_LIB_API`）→ `Result<T, E>`
- メモリプール（`MemPool`）→ `Vec<T>`でシミュレート

---

### Phase 4: sqlite3（大規模・難関）

**やるかどうかは自分で判断してよい。** 150KLOC超なので全体変換は非現実的。

代わりに、**特定のモジュール**だけを変換してみる:
- `utf.c` — UTF-8変換ロジック (~400行)
- `date.c` — 日付計算 (~1000行)

```bash
# sqliteのソースを入手
curl -O https://www.sqlite.org/2024/sqlite-amalgamation-3460000.zip
unzip sqlite-amalgamation-3460000.zip
```

---

## 作業手順

### Step 1: 変換エンジンのセットアップ確認

```bash
cd ~/cpp-to-rust
cargo build --release
./target/release/cpp-to-rust --help

# Cコンパイラの確認（出力比較に必要）
gcc --version
g++ --version
clang --version  # あれば
```

---

### Step 2: cJSON で E2Eパイプライン確認

```bash
# 変換実行
./target/release/cpp-to-rust convert-file \
    ./test-projects/cjson/cJSON.c \
    --profile c99 \
    --output ./output/cjson/

# コンパイル確認
cd ./output/cjson && cargo check 2>&1 | tee ../../results/cjson-check.txt

# オリジナルCのテストを確認
ls ./test-projects/cjson/tests/

# テストプログラムを書いて比較
cat > /tmp/test_cjson.c << 'EOF'
#include "cJSON.h"
#include <stdio.h>

int main() {
    const char* json = "{\"name\":\"John\",\"age\":30}";
    cJSON* root = cJSON_Parse(json);
    cJSON* name = cJSON_GetObjectItem(root, "name");
    cJSON* age = cJSON_GetObjectItem(root, "age");
    printf("name: %s\n", name->valuestring);
    printf("age: %d\n", (int)age->valuedouble);
    cJSON_Delete(root);
    return 0;
}
EOF

gcc /tmp/test_cjson.c ./test-projects/cjson/cJSON.c -I./test-projects/cjson -o /tmp/test-cjson-orig
/tmp/test-cjson-orig > /tmp/cjson-orig-output.txt

# Rustで同じテストを実装して比較
cd ./output/cjson && cargo run > /tmp/cjson-rust-output.txt
diff /tmp/cjson-orig-output.txt /tmp/cjson-rust-output.txt
```

---

### Step 3: メモリ安全性の検証

変換後のRustコードで**unsafe を使っていない**かを確認:

```bash
grep -rn "unsafe" ./output/cjson/src/
grep -rn "unsafe" ./output/libcsv/src/
grep -rn "unsafe" ./output/tinyxml2/src/

# unsafe が少ない = 変換品質が高い
echo "unsafe count:" $(grep -rn "unsafe" ./output/cjson/src/ | wc -l)
```

---

### Step 4: TinyXML2 の変換

```bash
./target/release/cpp-to-rust convert-file \
    ./test-projects/tinyxml2/tinyxml2.cpp \
    --profile cpp17 \
    --output ./output/tinyxml2/

cd ./output/tinyxml2 && cargo check 2>&1 | tee ../../results/tinyxml2-check.txt

# エラー数と種類を分析
cargo check 2>&1 | grep "^error" | sed 's/\[E[0-9]*\]//' | sort | uniq -c | sort -rn
```

---

### Step 5: 変換失敗パターンの分析

```markdown
## 未対応パターン（例）

| C/C++パターン | 出現頻度 | 対応難度 | 対応方針 |
|---|---|---|---|
| `void*` (汎用ポインタ) | 高 | 高 | ジェネリクス or Any trait |
| 関数ポインタ | 高 | 中 | Fn trait |
| ビットフィールド (`unsigned int x:4`) | 中 | 中 | ビット演算で再実装 |
| 可変引数 (`...`, `va_list`) | 中 | 高 | 個別対応 or TODOコメント |
| `setjmp/longjmp` | 低 | 高 | panic/catch_unwind で近似 |
| プリプロセッサ条件コンパイル (`#ifdef`) | 高 | 中 | cfg! マクロ |
| グローバル可変変数 | 高 | 中 | Mutex<T> / OnceLock<T> |
```

---

### Step 6: 内部テスト — strace でシステムコール比較

C/C++バイナリの挙動は stdout だけでなく**システムコールレベル**で一致させる。

```bash
# strace でシステムコールを記録・比較
strace -f -e trace=file,network,memory \
    /tmp/cjson-orig < /tmp/test-input.json > /tmp/strace-orig.txt 2>&1

strace -f -e trace=file,network,memory \
    ./output/cjson/target/release/cjson < /tmp/test-input.json > /tmp/strace-rust.txt 2>&1

# ファイルアクセスパターンを比較（アドレスは除外して比較）
grep "^open\|^read\|^write\|^close" /tmp/strace-orig.txt | \
    sed 's/0x[0-9a-f]*/ADDR/g' > /tmp/strace-orig-clean.txt
grep "^open\|^read\|^write\|^close" /tmp/strace-rust.txt | \
    sed 's/0x[0-9a-f]*/ADDR/g' > /tmp/strace-rust-clean.txt

diff /tmp/strace-orig-clean.txt /tmp/strace-rust-clean.txt \
    && echo "PASS (syscalls)" || echo "FAIL (syscalls)"
```

**Address Sanitizer でメモリ安全性を確認:**

```bash
# オリジナルC（メモリバグを検出）
clang -fsanitize=address,undefined -o /tmp/cjson-asan /tmp/test_cjson.c \
    ./test-projects/cjson/cJSON.c -I./test-projects/cjson
/tmp/cjson-asan < /tmp/test-input.json

# Rust版（unsafeがない限りASANは不要だが確認）
RUSTFLAGS="-Z sanitizer=address" cargo +nightly build --target x86_64-unknown-linux-gnu
```

---

### Step 7: 外部テスト — libとしてのAPI比較

C/C++ライブラリをRustからFFIで呼び出し、Rust実装と同じ引数で比較する。

```bash
# cJSONをC版とRust版の両方から呼び出すテストドライバ
cat > ./tests/api_comparison.rs << 'EOF'
// C版をFFIで呼び出す
extern "C" {
    fn cJSON_Parse(value: *const std::ffi::c_char) -> *mut std::ffi::c_void;
    fn cJSON_GetObjectItem(object: *mut std::ffi::c_void, string: *const std::ffi::c_char)
        -> *mut std::ffi::c_void;
}

#[test]
fn compare_parse_results() {
    let json = r#"{"name":"John","age":30}"#;

    // C版
    let c_result = unsafe { /* FFI呼び出し */ };

    // Rust実装
    let rust_result = cjson::parse(json).unwrap();

    assert_eq!(c_result, rust_result);
}
EOF
```

---

### Step 8: 収束分析 — プラトー検出（libFuzzer連携）

C/C++はファジングと組み合わせることで入力空間を効率的に探索できる。

```bash
# cargo-fuzz で収束を測定
cargo install cargo-fuzz
cargo fuzz init
cargo fuzz add fuzz_cjson

# フィードバック駆動でテストケースを生成し続ける
cargo fuzz run fuzz_cjson -- -max_total_time=3600  # 1時間

# 収束グラフを生成（libFuzzerの出力をパース）
grep "^#" fuzz/artifacts/fuzz_cjson/default/*.txt | \
    awk '{print $1, $4}' > results/fuzzing-progress.txt
```

**手動バッチ収束テスト（libFuzzerを使わない場合）:**

```rust
// crates/verifier/src/convergence.rs （#04/#05と共通実装）
pub struct ConvergenceTracker {
    pub batch_size: usize,
    pub window: usize,
    pub threshold: f64,
    history: Vec<BatchResult>,
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
}
```

```bash
./target/release/cpp-to-rust shadow \
    --original /tmp/cjson-orig \
    --converted ./output/cjson/target/release/cjson \
    --batch-size 10000 --window 3 --threshold 0.001 \
    | tee results/convergence-cjson.txt
```

**期待する出力:**

```
| バッチ | 累積テスト数 | 新規差分 | 発見率 | 判定 |
|--------|------------|---------|--------|------|
| 1      |       10,000 |       23 |   0.2% | 継続 |
| 2      |       20,000 |        6 |   0.1% | 継続 |
| 3      |       30,000 |        1 |   0.0% | 継続 |
| 4      |       40,000 |        0 |   0.0% | 継続 |
| 5      |       50,000 |        0 |   0.0% | ✅ PLATEAU |

→ 50,000件でプラトー到達。C/Rust間の挙動完全一致を統計的に証明済み。
```

---

### Step 9: 変換エンジン改善のPR作成

```bash
cd ~/cpp-to-rust
git checkout main && git pull origin main
git checkout -b feat/oss-test-improvements

# malloc/free パターンの検出精度向上
# crates/cpp-parser/src/memory.rs を改善

# 所有権推論プロンプトの改善
# crates/rust-generator/src/ownership.rs を更新

git add -p
git commit -m "feat: Improve malloc/free → Vec/Box conversion based on OSS testing"
git push -u origin feat/oss-test-improvements
```

---

## レポート形式

`results/oss-conversion-report.md` に以下の形式で出力:

```markdown
# cpp-to-rust OSS変換テスト結果

実施日: YYYY-MM-DD

## サマリー

| プロジェクト | 言語 | 行数 | 変換完走 | cargo check | unsafe数 | TODO数 |
|---|---|---|---|---|---|---|
| cJSON | C99 | ~750 | ✅ | ✅ | 2 | 5 |
| libcsv | C99 | ~1000 | ✅ | ⚠️ (3エラー) | 0 | 12 |
| TinyXML2 | C++17 | ~3600 | ✅ | ❌ (28エラー) | 8 | 47 |

## メモリ安全性向上

| プロジェクト | 元のmalloc/free数 | 変換後unsafe数 | 安全性向上率 |
|---|---|---|---|
| cJSON | 45箇所 | 2 | 96% |

## 未対応パターン一覧

（上記テーブルを記載）

## 変換エンジン改善提案

1. **優先度高**: 関数ポインタ → Fn trait の自動変換
2. **優先度中**: `void*` の型推論
3. **優先度低**: setjmp/longjmp の対処
```

---

## 完了条件

- [ ] cJSON の変換が完走し、`cargo check` が通る
- [ ] cJSON の出力がオリジナルCと一致する
- [ ] libcsv / TinyXML2 で変換を試み、失敗パターンを文書化
- [ ] unsafe使用箇所を数値化し、安全性向上率を算出
- [ ] `results/oss-conversion-report.md` が出力される
- [ ] 変換エンジンの改善点を特定し、少なくとも1件のPRを作成
- [ ] `cargo test --workspace` が通る
- [ ] `cargo clippy --workspace -- -D warnings` が通る

---

## ブランチ

```bash
cd ~/cpp-to-rust
git checkout main && git pull origin main
git checkout -b feat/oss-test-improvements
```

PR作成後、QA #09 レビュー → オーナー承認でマージ。

---

## 判断の優先順位

1. **unsafeを最小化する。** C→Rustの変換でunsafeだらけになったら変換の意味がない。
2. **Cから始める。** C++のテンプレートや継承は複雑。cJSON・libcsv（純C）で先にパイプラインを完成させる。
3. **出力比較が命。** 動いているように見えても、オリジナルと同じ結果を出すまで完成ではない。
