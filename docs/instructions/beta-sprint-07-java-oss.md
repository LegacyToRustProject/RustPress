# ベータスプリント指示書 — #07 java-to-rust OSS変換テスト

## このスプリントのミッション

変換エンジン（java-to-rust）を**実際のJava OSSプロジェクト**で試し、変換成功率・JVMメモリ削減効果をレポートにまとめる。

Java→Rustの最大の価値は**JVMメモリ削減**（GBクラス → MBクラス）と**起動時間の短縮**。変換後にこの効果が実現しているかを計測する。

---

## 対象プロジェクト（難易度順）

### Phase 1: Apache Commons Lang（ユーティリティ・最小）

**なぜ**: `StringUtils`, `NumberUtils`, `ArrayUtils` など、状態を持たない静的メソッド群。クラス構造がシンプルで変換パイプラインのE2E確認に最適。

```bash
git clone https://github.com/apache/commons-lang.git ./test-projects/commons-lang
```

**ターゲット（難易度低順）**:
1. `StringUtils.java` — 文字列操作メソッド群（静的メソッドのみ）
2. `NumberUtils.java` — 数値変換・検証
3. `ArrayUtils.java` — 配列操作

**変換の焦点**:
- `static` メソッド → `pub fn`（impl不要）
- `null` チェック → `Option<T>`
- `String` → `String` / `&str`
- 例外（`IllegalArgumentException`）→ `Result<T, Error>`

---

### Phase 2: Google Guava（コレクション・イベント）

**なぜ**: `com.google.common.collect` パッケージは Java標準コレクションの拡張。Rustの標準ライブラリとの対応関係が明確。

```bash
git clone https://github.com/google/guava.git ./test-projects/guava
```

**ターゲット**:
- `ImmutableList.java` → Rustの`Vec<T>`（読み取り専用）
- `Optional.java` → Rustの`Option<T>`（ほぼ1対1）
- `Preconditions.java` → `assert!` / `anyhow::ensure!`

---

### Phase 3: Spring Boot RESTサンプル（Webアプリ・難関）

**なぜ**: Spring Boot → Axumの変換パターン確立。企業のJavaマイクロサービス移行を代表する。

```bash
git clone https://github.com/spring-guides/gs-rest-service.git \
    ./test-projects/spring-rest-service
```

**変換の焦点**:
- `@RestController` → Axum Router
- `@GetMapping("/greeting")` → `.route("/greeting", get(handler))`
- `@RequestParam` → Axumのクエリパラメータ抽出
- `@Service` / `@Autowired` → 依存性注入をState/引数に変換

---

### Phase 4: JUnit 5（テストフレームワーク・難関）

**なぜ**: アノテーション駆動の典型。`@Test`, `@BeforeEach`, `@ParameterizedTest` → Rustのテストフレームワークへの変換を試す。コア部分のみを対象とする。

```bash
git clone https://github.com/junit-team/junit5.git ./test-projects/junit5
# 特定のモジュールのみ
ls ./test-projects/junit5/junit-jupiter-api/src/main/java/org/junit/jupiter/api/
```

---

## 作業手順

### Step 1: 変換エンジンのセットアップ確認

```bash
cd ~/java-to-rust
cargo build --release
./target/release/java-to-rust --help

# Java環境の確認（出力比較に必要）
java --version
javac --version
mvn --version || gradle --version
```

---

### Step 2: StringUtils で E2Eパイプライン確認

```bash
# 変換実行
./target/release/java-to-rust convert-file \
    ./test-projects/commons-lang/src/main/java/org/apache/commons/lang3/StringUtils.java \
    --profile generic \
    --output ./output/string-utils/

# コンパイル確認
cd ./output/string-utils && cargo check 2>&1 | tee ../../results/string-utils-check.txt

# テスト作成して比較
# StringUtils.isEmpty("") → true
# StringUtils.isEmpty(null) → true (Option<&str>をNoneで表現)
cat > ./output/string-utils/src/main_test.rs << 'EOF'
#[test]
fn test_is_empty() {
    assert_eq!(StringUtils::is_empty(""), true);
    assert_eq!(StringUtils::is_empty("hello"), false);
}
EOF

cd ./output/string-utils && cargo test
```

---

### Step 3: Optional変換の検証

Java の `Optional<T>` → Rust の `Option<T>` はほぼ1対1対応。精度を測る。

```bash
./target/release/java-to-rust convert-file \
    ./test-projects/guava/guava/src/com/google/common/base/Optional.java \
    --output ./output/optional/

# 変換後のコードで Rustの Option<T> と同じAPIが使えるか確認
grep -n "pub fn\|fn " ./output/optional/src/lib.rs
```

---

### Step 4: Spring Boot RESTサンプルの変換

```bash
./target/release/java-to-rust convert \
    ./test-projects/spring-rest-service/complete/src \
    --profile spring-boot \
    --output ./output/spring-rest-service/

cd ./output/spring-rest-service && cargo check 2>&1 | tee ../../results/spring-check.txt

# 生成されたAxumルーターを確認
grep -A 20 "fn main\|Router::new" ./output/spring-rest-service/src/main.rs
```

---

### Step 5: メモリ使用量の比較（定量評価）

変換が成功したプロジェクトで、JVM版とRust版のメモリ使用量を比較:

```bash
# Spring RESTサービスの場合
# JVM版を起動（Dockerで）
docker run -p 8080:8080 \
    -v $(pwd)/test-projects/spring-rest-service:/app \
    maven:3.9-eclipse-temurin-17 \
    sh -c "cd /app && mvn spring-boot:run" &

sleep 30
# JVMのメモリ使用量を記録
curl -s http://localhost:8080/actuator/metrics/jvm.memory.used 2>/dev/null || \
    ps aux | grep java | awk '{print $6}' > /tmp/java-memory.txt

# Rust版を起動
cd ./output/spring-rest-service && cargo build --release
./target/release/spring-rest-service &
sleep 5
ps aux | grep spring-rest | awk '{print $6}' > /tmp/rust-memory.txt

echo "JVM memory: $(cat /tmp/java-memory.txt) kB"
echo "Rust memory: $(cat /tmp/rust-memory.txt) kB"
```

---

### Step 6: 変換失敗パターンの分析

```markdown
## 未対応パターン（例）

| Javaパターン | 出現頻度 | 対応難度 | 対応方針 |
|---|---|---|---|
| `instanceof` + キャスト | 高 | 中 | パターンマッチ |
| `equals()` / `hashCode()` | 高 | 低 | PartialEq / Hash derive |
| `Comparable<T>` | 中 | 低 | PartialOrd / Ord |
| `Iterator<T>` パターン | 高 | 中 | impl Iterator |
| `checked/unchecked exception` 混在 | 高 | 高 | Result<T, Box<dyn Error>> |
| `synchronized` + `wait/notify` | 中 | 高 | Mutex + Condvar |
| リフレクション | 低 | 非常に高 | TODOコメントで対応 |
| アノテーションプロセッサ | 低 | 非常に高 | proc-macro で対応方針を示す |
```

---

### Step 7: 内部テスト — JVM バイナリと Rust バイナリの出力比較

```bash
# Spring RESTサービスの場合: 同じHTTPリクエストを両方に送る
# JVM版を起動（Docker）
docker run -d -p 8081:8080 \
    -v $(pwd)/test-projects/spring-rest-service/complete:/app \
    maven:3.9-eclipse-temurin-17 \
    sh -c "cd /app && mvn -q spring-boot:run"

# Rust版を起動
cd ./output/spring-rest-service && cargo build --release
./target/release/spring-rest-service &
sleep 5

# シャドウ実行スクリプト
cat > ./scripts/shadow-http.sh << 'EOF'
#!/bin/bash
ENDPOINT="$1"
JVM_PORT=8081
RUST_PORT=8080

JVM_RESPONSE=$(curl -s "http://localhost:${JVM_PORT}${ENDPOINT}")
RUST_RESPONSE=$(curl -s "http://localhost:${RUST_PORT}${ENDPOINT}")

JVM_NORMALIZED=$(echo "$JVM_RESPONSE" | jq -S .)
RUST_NORMALIZED=$(echo "$RUST_RESPONSE" | jq -S .)

if [ "$JVM_NORMALIZED" = "$RUST_NORMALIZED" ]; then
    echo "PASS: $ENDPOINT"
else
    echo "FAIL: $ENDPOINT"
    diff <(echo "$JVM_NORMALIZED") <(echo "$RUST_NORMALIZED")
fi
EOF
chmod +x ./scripts/shadow-http.sh

# 各エンドポイントをテスト
./scripts/shadow-http.sh "/greeting"
./scripts/shadow-http.sh "/greeting?name=World"
./scripts/shadow-http.sh "/greeting?name=RustPress"
```

**純粋関数（StringUtils等）の直接比較:**

```bash
# Java版とRust版で同じ入力 → 同じ出力を確認
cat > /tmp/StringUtilsTest.java << 'EOF'
public class StringUtilsTest {
    public static void main(String[] args) {
        System.out.println(org.apache.commons.lang3.StringUtils.isEmpty(""));
        System.out.println(org.apache.commons.lang3.StringUtils.isEmpty("hello"));
        System.out.println(org.apache.commons.lang3.StringUtils.reverse("Rust"));
        System.out.println(org.apache.commons.lang3.StringUtils.repeat("ab", 3));
    }
}
EOF
java -cp commons-lang3.jar:. StringUtilsTest > /tmp/java-output.txt

./output/string-utils/target/release/string-utils > /tmp/rust-output.txt
diff /tmp/java-output.txt /tmp/rust-output.txt && echo "PASS" || echo "FAIL"
```

---

### Step 8: 外部テスト — k6 で負荷時の挙動比較

JVMとRustの最大の違いは**高負荷時の挙動**。k6で同じ負荷をかけて比較する。

```javascript
// scripts/k6-shadow.js
import http from 'k6/http';
import { check } from 'k6';

export const options = {
    vus: 50,        // 同時接続50
    duration: '60s',
};

export default function() {
    // JVM版とRust版に同じリクエスト
    const jvm  = http.get('http://localhost:8081/greeting?name=World');
    const rust = http.get('http://localhost:8080/greeting?name=World');

    check(jvm,  { 'jvm 200':  (r) => r.status === 200 });
    check(rust, { 'rust 200': (r) => r.status === 200 });

    // レスポンスボディが一致することを確認
    check(rust, {
        'body matches jvm': (r) => r.body === jvm.body,
    });
}
```

```bash
k6 run scripts/k6-shadow.js | tee results/k6-shadow-report.txt

# メモリ使用量の記録
ps aux | grep -E "java|spring" | awk '{print "JVM:", $6, "kB"}'
ps aux | grep spring-rest-service | awk '{print "Rust:", $6, "kB"}'
```

---

### Step 9: 収束分析 — プラトー検出

```rust
// crates/verifier/src/convergence.rs （#04〜#08共通実装）
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
./target/release/java-to-rust shadow \
    --original "java -cp commons-lang3.jar:. StringUtilsTest" \
    --converted ./output/string-utils/target/release/string-utils \
    --batch-size 5000 --window 3 --threshold 0.001 \
    | tee results/convergence-string-utils.txt
```

**期待する出力:**

```
| バッチ | 累積テスト数 | 新規差分 | 発見率 | 判定 |
|--------|------------|---------|--------|------|
| 1      |        5,000 |       15 |   0.3% | 継続 |
| 2      |       10,000 |        3 |   0.1% | 継続 |
| 3      |       15,000 |        0 |   0.0% | 継続 |
| 4      |       20,000 |        0 |   0.0% | 継続 |
| 5      |       25,000 |        0 |   0.0% | ✅ PLATEAU |

→ 25,000件でプラトー到達。JVM/Rust間の挙動完全一致を統計的に証明済み。
```

---

### Step 10: 変換エンジン改善のPR作成

```bash
cd ~/java-to-rust
git checkout main && git pull origin main
git checkout -b feat/oss-test-improvements

# Springプロファイルのアノテーションマッピング改善
# profiles/spring-boot/ を更新

# Optional → Option変換の精度向上
# crates/rust-generator/src/patterns.rs を更新

git commit -m "feat: Improve Spring Boot annotation → Axum handler conversion"
git push -u origin feat/oss-test-improvements
```

---

## レポート形式

`results/oss-conversion-report.md` に以下の形式で出力:

```markdown
# java-to-rust OSS変換テスト結果

実施日: YYYY-MM-DD

## サマリー

| プロジェクト | Java Ver | 行数 | 変換完走 | cargo check | メモリ比較 |
|---|---|---|---|---|---|
| StringUtils | 8 | ~3500 | ✅ | ✅ | N/A (ライブラリ) |
| Optional (Guava) | 8 | ~300 | ✅ | ✅ | N/A |
| Spring REST | 17 | ~200 | ✅ | ⚠️ (5エラー) | JVM: 256MB → Rust: 4MB |

## メモリ削減効果

Spring REST サービス:
- JVM: XX MB
- Rust: XX MB
- 削減率: XX%

## 未対応パターン一覧

（上記テーブルを記載）

## 変換エンジン改善提案

1. **優先度高**: `@GetMapping` → `.route()` の自動生成精度向上
2. **優先度中**: `Optional<T>` → `Option<T>` 変換（ほぼ完成）
3. **優先度低**: リフレクション対処方針の決定
```

---

## 完了条件

- [ ] StringUtils / NumberUtils の変換が完走し、`cargo check` が通る
- [ ] 少なくとも3つの静的メソッドの出力がJava版と一致する
- [ ] Spring RESTサービスの変換を試み、失敗パターンを文書化
- [ ] JVM vs Rust のメモリ使用量比較が1件以上できる
- [ ] `results/oss-conversion-report.md` が出力される
- [ ] 変換エンジンの改善点を特定し、少なくとも1件のPRを作成
- [ ] `cargo test --workspace` が通る
- [ ] `cargo clippy --workspace -- -D warnings` が通る

---

## ブランチ

```bash
cd ~/java-to-rust
git checkout main && git pull origin main
git checkout -b feat/oss-test-improvements
```

PR作成後、QA #09 レビュー → オーナー承認でマージ。

---

## 判断の優先順位

1. **Java 8から始める。** Java 8の静的メソッド（StringUtils等）が最もシンプル。確実に動くものを先に作る。
2. **JVMメモリ削減を数値化する。** 「Rustは速い」は主観。「256MB → 4MB」は事実。この数値を出すことが最大のインパクト。
3. **リフレクションはスキップしてよい。** 変換不可能に近い。TODOコメントで対応し、他を進める。
