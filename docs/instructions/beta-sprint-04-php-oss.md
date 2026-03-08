# ベータスプリント指示書 — #04 php-to-rust OSS変換テスト

## このスプリントのミッション

変換エンジン（php-to-rust）を**実際のOSSプロジェクト**で試し、変換成功率・手動修正量を計測してレポートにまとめる。

変換エンジンが「動く」だけでは不十分。**本物のWordPressプラグインで動くか**を検証し、改善点を特定する。

---

## 対象プロジェクト（難易度順）

### Phase 1: Hello Dolly（最小確認）

**なぜ**: WordPress同梱の最小プラグイン。~100行。変換パイプラインのE2E確認に最適。

```bash
# WordPressのGitリポジトリから取得
curl -O https://raw.githubusercontent.com/WordPress/WordPress/master/wp-content/plugins/hello.php
```

**期待する成果**:
- `php-to-rust convert-file hello.php --profile wordpress` が完走する
- `cargo check` が通るRustコードが生成される
- 手動修正が必要な箇所をTODOコメントで特定できる

---

### Phase 2: WP Super Cache（中規模・バッチ処理型）

**なぜ**: ~8000行。キャッシュ制御・ファイルI/O・wp_options操作が中心。
WordPressの代表的な「裏方プラグイン」パターン。

```bash
# OSSリポジトリから取得
git clone https://github.com/Automattic/wp-super-cache.git ./test-projects/wp-super-cache
```

**注意すべきPHPパターン**:
- `file_put_contents` / `file_get_contents` → `std::fs`
- `$_SERVER` グローバル変数 → Axumリクエストから取得
- `wp_cache_*` 関数 → rustpress-cache クレート

---

### Phase 3: Akismet（API通信・DB操作）

**なぜ**: ~3000行。外部API通信（HTTP POST）・DBへのスパム記録が中心。
変換エンジンのHTTP/DB変換能力を試す。

```bash
git clone https://github.com/Automattic/akismet-wordpress-plugin.git ./test-projects/akismet
```

**注意すべきPHPパターン**:
- `wp_remote_post` → reqwest
- `$wpdb->insert` / `$wpdb->get_results` → SeaORM
- `add_action('comment_post', ...)` → フックシステム

---

### Phase 4: Query Monitor（デバッグ・インストルメンテーション）

**なぜ**: ~5000行。フックの大量登録・出力バッファリング・データ収集が中心。
変換エンジンのフックシステム変換能力の限界を測る。

```bash
git clone https://github.com/johnbillion/query-monitor.git ./test-projects/query-monitor
```

---

## 作業手順

### Step 1: 変換エンジンのセットアップ確認

```bash
cd ~/php-to-rust
cargo build --release
./target/release/php-to-rust --help
```

ビルドが通ること、`analyze` / `convert` / `convert-file` サブコマンドが存在することを確認。

---

### Step 2: Hello Dolly で E2Eパイプライン確認

```bash
# 変換実行
./target/release/php-to-rust convert-file \
    ./test-projects/hello.php \
    --profile wordpress \
    --output ./output/hello-dolly/

# コンパイル確認
cd ./output/hello-dolly && cargo check 2>&1 | tee ../../results/hello-dolly-check.txt

# 結果記録
echo "手動修正が必要な箇所:" >> ../../results/hello-dolly-report.md
grep -n "TODO\|FIXME\|unimplemented" src/**/*.rs >> ../../results/hello-dolly-report.md
```

---

### Step 3: 各プロジェクトで変換を試す

```bash
# WP Super Cache
./target/release/php-to-rust convert \
    ./test-projects/wp-super-cache \
    --profile wordpress \
    --output ./output/wp-super-cache/

# Akismet
./target/release/php-to-rust convert \
    ./test-projects/akismet \
    --profile wordpress \
    --output ./output/akismet/
```

各プロジェクトで以下を記録:
1. 変換が完走したか（Yes/No + エラーログ）
2. `cargo check` の結果（成功/エラー数）
3. TODOコメントの数（未変換箇所）
4. 手動で修正した箇所とその内容

---

### Step 4: 変換失敗パターンの分析

変換できなかったPHPパターンを分類:

```markdown
## 未対応パターン（例）

| PHPパターン | 出現頻度 | 対応難度 | 対応方針 |
|---|---|---|---|
| `$wpdb->prepare()` (SQL format) | 高 | 中 | WordPressプロファイルに追加 |
| `wp_remote_post()` | 中 | 低 | reqwest への直接マッピング |
| `ob_start()` / `ob_get_clean()` | 中 | 高 | バッファリング戦略が必要 |
| `extract($array)` | 低 | 高 | 静的解析が困難、TODOコメントで対応 |
```

---

### Step 5: 内部テスト — バイナリ挙動比較（シャドウ実行）

変換後のRustバイナリとオリジナルPHPを**同じ入力**で実行し、stdout・stderr・終了コードを全て比較する。

```bash
# シャドウ実行スクリプト
cat > ./scripts/shadow-run.sh << 'EOF'
#!/bin/bash
INPUT="$1"
PHP_SCRIPT="$2"
RUST_BIN="$3"

php "$PHP_SCRIPT" "$INPUT" > /tmp/php-out.txt 2>/tmp/php-err.txt; PHP_EXIT=$?
"$RUST_BIN"       "$INPUT" > /tmp/rust-out.txt 2>/tmp/rust-err.txt; RUST_EXIT=$?

if diff -q /tmp/php-out.txt /tmp/rust-out.txt > /dev/null && [ "$PHP_EXIT" = "$RUST_EXIT" ]; then
    echo "PASS"
else
    echo "FAIL"
    diff /tmp/php-out.txt /tmp/rust-out.txt
fi
EOF
chmod +x ./scripts/shadow-run.sh
```

---

### Step 6: 外部テスト — HTTP/Selenium でブラックボックス比較

PHPプラグインがWordPressを通じて提供する**HTTPレスポンスそのもの**をRustPressと比較する。内部バイナリ比較では検出できない「レンダリング差分」や「DBへの副作用差分」を捕捉できる。

```bash
# WordPress（オリジナル）とRustPress（変換後）を並行起動
# WordPress: :8081, RustPress: :8080（既存のdocker-compose構成と同じ）

docker-compose up -d wordpress rustpress

# curl でHTTPレスポンスを比較
ENDPOINTS=(
    "/wp-json/wp/v2/posts"
    "/wp-json/wp/v2/posts/1"
    "/?p=1"
    "/wp-sitemap.xml"
)

for EP in "${ENDPOINTS[@]}"; do
    curl -s http://localhost:8081"$EP" | jq -S . > /tmp/wp-"$(echo $EP | tr '/' '_')".json
    curl -s http://localhost:8080"$EP" | jq -S . > /tmp/rp-"$(echo $EP | tr '/' '_')".json
    diff /tmp/wp-*.json /tmp/rp-*.json && echo "PASS: $EP" || echo "FAIL: $EP"
done
```

**Seleniumビジュアル比較（既存の E2E テストを流用）:**

```bash
# crates/rustpress-e2e の既存テストにプラグイン有効化シナリオを追加
# Akismet有効時のコメント送信フォームが両方で同じ見た目か確認
docker-compose --profile e2e up -d
cargo test --package rustpress-e2e -- plugin_akismet_comment_form
```

---

### Step 7: 収束分析 — プラトー検出

「何件テストすれば十分か」を定量的に判定する。

**`crates/verifier/src/convergence.rs` を実装:**

```rust
pub struct ConvergenceTracker {
    pub batch_size: usize,   // 1バッチのテスト件数（例: 1000）
    pub window: usize,       // 連続N回閾値以下なら停止（例: 3）
    pub threshold: f64,      // 新規差分率の閾値（例: 0.001 = 0.1%）
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

**実行:**

```bash
./target/release/php-to-rust shadow \
    --original ./test-projects/hello.php \
    --converted ./output/hello-dolly/target/release/hello-dolly \
    --batch-size 1000 --window 3 --threshold 0.001 \
    | tee results/convergence-hello-dolly.txt
```

**期待する出力:**

```
| バッチ | 累積テスト数 | 新規差分 | 発見率 | 判定 |
|--------|------------|---------|--------|------|
| 1      |        1,000 |       12 |   1.2% | 継続 |
| 2      |        2,000 |        4 |   0.4% | 継続 |
| 3      |        3,000 |        1 |   0.1% | 継続 |
| 4      |        4,000 |        0 |   0.0% | 継続 |
| 5      |        5,000 |        0 |   0.0% | 継続 |
| 6      |        6,000 |        0 |   0.0% | ✅ PLATEAU |

→ 6,000件でプラトー到達。本番稼働の挙動一致を統計的に証明済み。
```

---

### Step 8: 変換エンジン改善のPR作成

分析結果をもとに、変換エンジン（php-to-rust）のコードを改善してPRを作成:

```bash
cd ~/php-to-rust
git checkout main && git pull origin main
git checkout -b feat/oss-test-improvements

# WordPressプロファイルのAPIマッピングを追加
# profiles/wordpress/api_mappings.toml を更新

# プロンプトテンプレートを改善
# crates/rust-generator/src/prompt.rs を更新

git add -p
git commit -m "feat: Improve WordPress profile mappings based on OSS testing"
git push -u origin feat/oss-test-improvements
# → PRを作成
```

---

## レポート形式

`results/oss-conversion-report.md` に以下の形式で出力:

```markdown
# php-to-rust OSS変換テスト結果

実施日: YYYY-MM-DD

## サマリー

| プロジェクト | 行数 | 変換完走 | cargo check | TODO数 | 成功率推定 |
|---|---|---|---|---|---|
| Hello Dolly | ~100 | ✅ | ✅ | 3 | 90% |
| WP Super Cache | ~8000 | ✅ | ❌ (12エラー) | 47 | 60% |
| Akismet | ~3000 | ✅ | ⚠️ (3エラー) | 18 | 75% |

## 詳細: Hello Dolly

### 変換されたコード（抜粋）
```rust
// 変換結果の例
```

### 未対応パターン
- TODO #1: ...
- TODO #2: ...

### 手動修正内容
- 修正1: ...

## 変換エンジン改善提案

1. **優先度高**: `$wpdb->prepare()` のサポート追加
2. **優先度中**: `wp_remote_post/get` → reqwest の自動変換
3. **優先度低**: `extract()` の対処方針
```

---

## 完了条件

- [ ] Hello Dollyの変換が完走し、`cargo check`が通る
- [ ] WP Super Cache / Akismetで変換を試み、失敗パターンを文書化
- [ ] `results/oss-conversion-report.md` が出力される
- [ ] 変換エンジンの改善点を特定し、少なくとも1件のPRを作成
- [ ] PR: `cargo test --workspace` が通る
- [ ] `cargo clippy --workspace -- -D warnings` が通る

---

## ブランチ

```bash
cd ~/php-to-rust
git checkout main && git pull origin main
git checkout -b feat/oss-test-improvements
```

PR作成後、QA #09 レビュー → オーナー承認でマージ。
