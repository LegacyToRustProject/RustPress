# ベータスプリント指示書 — #08 perl-to-rust OSS変換テスト

## このスプリントのミッション

変換エンジン（perl-to-rust）を**実際のPerl OSSプロジェクト**で試し、変換成功率・CPANマッピングの充実度をレポートにまとめる。

Perl→Rustの最大の難敵は**暗黙の挙動**と**正規表現の複雑さ**。実際のコードでどこまで自動変換できるかを計測する。

---

## 対象プロジェクト（難易度順）

### Phase 1: List::Util（最小・純粋関数）

**なぜ**: PerlコアモジュールのList::Utilは、`sum`, `min`, `max`, `first`, `any`, `all` など、副作用のない純粋な関数群。変換パイプラインのE2E確認に最適。

```bash
# PerlのList::Utilのソースを取得
# cpanmでインストール後にソースを確認
cpanm --look List::Util  # または
curl -L https://cpan.metacpan.org/authors/id/P/PE/PEVANS/Scalar-List-Utils-1.63.tar.gz | tar xz
ls Scalar-List-Utils-1.63/lib/List/
```

**変換の焦点**:
- `sub sum { my @nums = @_; ... }` → `pub fn sum(nums: &[f64]) -> f64`
- `grep { $_ > 0 } @list` → `list.iter().filter(|&&x| x > 0)`
- `map { $_ * 2 } @list` → `list.iter().map(|&x| x * 2)`
- `$_` 暗黙変数の明示化

---

### Phase 2: Getopt::Long（CLIオプション解析）

**なぜ**: PerlのCLIプログラムほぼ全てが使うモジュール。Rustの`clap`クレートとの対応関係が明確。~1500行。

```bash
# ソースを取得
cpan -g Getopt::Long  # または
curl -L https://cpan.metacpan.org/authors/id/J/JV/JV/Getopt-Long-2.57.tar.gz | tar xz
ls Getopt-Long-2.57/lib/
```

**変換の焦点**:
- `GetOptions("verbose!" => \$verbose, "file=s" => \$file)` → `clap::Parser` derive
- `\$variable` (スカラーリファレンス) → `&mut` 参照

---

### Phase 3: CGI.pm（Webアプリ基礎・歴史的重要性）

**なぜ**: 世界中の古いWebアプリで使われているPerlのCGIモジュール。~4000行。`$q->param()`, `$q->header()`, `$q->start_html()` など、HTTPの基本操作が含まれる。Axumへの変換パターン確立に重要。

```bash
curl -L https://cpan.metacpan.org/authors/id/L/LE/LEEJO/CGI-4.66.tar.gz | tar xz
ls CGI-4.66/lib/
```

**変換の焦点**:
- `$q->param('name')` → Axumクエリパラメータ
- `$q->header(-type => 'text/html')` → Axum Response builder
- `$ENV{REQUEST_METHOD}` → Axum Request
- `print $q->start_html(...)` → Tera テンプレート

---

### Phase 4: DBI（データベースインターフェース・難関）

**なぜ**: PerlのDB操作の標準。`$dbh->prepare()`, `$sth->execute()`, `$sth->fetchrow_hashref()` → SeaORMへの変換パターン。

```bash
# DBIのソースを取得
curl -L https://cpan.metacpan.org/authors/id/T/TI/TIMB/DBI-1.645.tar.gz | tar xz
```

対象は`DBI.pm`の主要メソッドのみ。全体変換は不要。

---

## 作業手順

### Step 1: 変換エンジンのセットアップ確認

```bash
cd ~/perl-to-rust
cargo build --release
./target/release/perl-to-rust --help

# Perl環境の確認（出力比較に必要）
perl --version
cpan --version || cpanm --version
```

---

### Step 2: List::Util で E2Eパイプライン確認

```bash
# 変換実行（List::Util の Perl実装部分）
./target/release/perl-to-rust convert-file \
    ./Scalar-List-Utils-1.63/lib/List/Util.pm \
    --output ./output/list-util/

# コンパイル確認
cd ./output/list-util && cargo check 2>&1 | tee ../../results/list-util-check.txt

# オリジナルPerlで出力を記録
perl -e '
use List::Util qw(sum min max first any all);
my @nums = (1..10);
print "sum: ", sum(@nums), "\n";
print "min: ", min(@nums), "\n";
print "max: ", max(@nums), "\n";
print "first > 5: ", first { $_ > 5 } @nums, "\n";
' > /tmp/perl-list-util-output.txt

# Rustで同じ出力
cd ./output/list-util && cargo run > /tmp/rust-list-util-output.txt
diff /tmp/perl-list-util-output.txt /tmp/rust-list-util-output.txt
```

---

### Step 3: 正規表現変換の精度検証

Perlの正規表現変換はエンジンの品質を最も測定できる箇所。

```bash
# 正規表現テスト用のPerlスクリプト
cat > /tmp/regex-test.pl << 'EOF'
#!/usr/bin/perl
use strict;
use warnings;

# キャプチャグループ
my $date = "2024-03-08";
if ($date =~ /^(\d{4})-(\d{2})-(\d{2})$/) {
    print "year=$1, month=$2, day=$3\n";
}

# 置換
my $text = "Hello World Hello";
(my $result = $text) =~ s/Hello/Goodbye/g;
print "$result\n";

# 名前付きキャプチャ
my $log = "[ERROR] Connection failed at line 42";
if ($log =~ /\[(?P<level>\w+)\] (?P<message>.+) at line (?P<line>\d+)/) {
    print "level=$+{level}, line=$+{line}\n";
    print "message=$+{message}\n";
}
EOF

perl /tmp/regex-test.pl > /tmp/perl-regex-output.txt

# 変換
./target/release/perl-to-rust convert-file /tmp/regex-test.pl \
    --output ./output/regex-test/

cd ./output/regex-test && cargo run > /tmp/rust-regex-output.txt
diff /tmp/perl-regex-output.txt /tmp/rust-regex-output.txt
```

---

### Step 4: CPANマッピング表の充実

実際のOSSコードで使われているCPANモジュールを調査し、マッピング表を更新:

```bash
# 変換対象プロジェクトのCPAN依存を収集
grep -rh "^use " ./test-projects/ | \
    sed 's/use \([A-Za-z:]*\).*/\1/' | \
    sort | uniq -c | sort -rn | head 50

# 現在のマッピング表
cat ./cpan-mappings/mappings.toml
```

よく使われるのに未マッピングのモジュールを`mappings.toml`に追加:

```toml
# 追加候補
"File::Basename" = "# std::path::Path"
"File::Find" = "# walkdir crate"
"Storable" = "# serde + bincode"
"Data::Dumper" = "# dbg! macro / serde_json::to_string_pretty"
"Encode" = "# std::string / encoding_rs crate"
"POSIX" = "# std::* / nix crate"
"Scalar::Util" = "# std::* (looks_like_number → str::parse)"
"List::MoreUtils" = "# Iterator methods"
"Time::HiRes" = "# std::time::Instant"
"Carp" = "# anyhow / thiserror"
```

---

### Step 5: CGI.pm → Axum の変換パターン確立

```bash
# CGI.pm のコアメソッドを変換
./target/release/perl-to-rust convert-file \
    ./CGI-4.66/lib/CGI.pm \
    --output ./output/cgi-pm/

# 生成されたコードでAxumの構造が出ているか確認
grep -n "axum\|Router\|get\|post\|Response" ./output/cgi-pm/src/lib.rs | head 20

# cargo check
cd ./output/cgi-pm && cargo check 2>&1 | tee ../../results/cgi-check.txt
```

---

### Step 6: 変換失敗パターンの分析

```markdown
## 未対応パターン（例）

| Perlパターン | 出現頻度 | 対応難度 | 対応方針 |
|---|---|---|---|
| `$_` 暗黙変数 | 非常に高 | 低 | 明示的変数名に展開済み |
| `wantarray` (コンテキスト判定) | 中 | 高 | 関数を2つに分割 |
| `local $_` | 中 | 中 | スコープ付き変数に変換 |
| `@_` (引数配列) | 非常に高 | 低 | 引数リストに変換済み |
| `eval { }` (例外捕捉) | 高 | 中 | Result + catch_unwind |
| `AUTOLOAD` | 低 | 非常に高 | TODOコメント |
| `tie` / `untie` | 低 | 非常に高 | TODOコメント |
| ヒアドキュメント (`<<EOF`) | 高 | 低 | 文字列リテラルに変換済み |
| 正規表現修飾子 `/x` (コメント付き) | 中 | 低 | verboseモード対応 |
| Perl OOP (`bless`) | 高 | 中 | struct + impl に変換 |
```

---

### Step 7: 内部テスト — Perl バイナリと Rust バイナリの stdout 比較

```bash
# シャドウ実行スクリプト
cat > ./scripts/shadow-run.sh << 'EOF'
#!/bin/bash
PERL_SCRIPT="$1"
RUST_BIN="$2"
INPUT="$3"

perl "$PERL_SCRIPT" "$INPUT" > /tmp/perl-out.txt 2>/tmp/perl-err.txt; PERL_EXIT=$?
"$RUST_BIN"         "$INPUT" > /tmp/rust-out.txt 2>/tmp/rust-err.txt; RUST_EXIT=$?

# stdout・終了コードを比較
if diff -q /tmp/perl-out.txt /tmp/rust-out.txt > /dev/null && \
   [ "$PERL_EXIT" = "$RUST_EXIT" ]; then
    echo "PASS"
else
    echo "FAIL"
    diff /tmp/perl-out.txt /tmp/rust-out.txt
fi
EOF
chmod +x ./scripts/shadow-run.sh

# List::Util のシャドウ実行
./scripts/shadow-run.sh \
    ./test-projects/list-util-test.pl \
    ./output/list-util/target/release/list-util \
    ""
```

**書き出しファイルがある場合（CGI.pm等）:**

```bash
# CGI.pm がHTMLファイルを生成する場合
perl ./test-projects/cgi-test.pl > /tmp/perl-cgi.html
./output/cgi-pm/target/release/cgi-pm > /tmp/rust-cgi.html

# HTMLを正規化して比較（空白・改行の差異を無視）
xmllint --format /tmp/perl-cgi.html > /tmp/perl-cgi-norm.html 2>/dev/null || true
xmllint --format /tmp/rust-cgi.html > /tmp/rust-cgi-norm.html 2>/dev/null || true
diff /tmp/perl-cgi-norm.html /tmp/rust-cgi-norm.html \
    && echo "PASS (HTML)" || echo "FAIL (HTML)"
```

---

### Step 8: 外部テスト — HTTPレベルでのブラックボックス比較

CGI.pm を Axum に変換した場合、HTTPレベルで同じ出力を返すことを確認。

```bash
# Perl CGI をApache/nginx経由で動かす（オリジナル）
docker run -d -p 8081:80 \
    -v $(pwd)/test-projects/cgi-pm-app:/var/www/cgi-bin \
    httpd:2.4

# Rust版のAxumサービス
cd ./output/cgi-pm-axum && cargo build --release
./target/release/cgi-pm-axum &

# 同じリクエストを両方に送る
cat > ./scripts/shadow-http.sh << 'EOF'
#!/bin/bash
PATH_QUERY="$1"

PERL_RESP=$(curl -s "http://localhost:8081/cgi-bin/app.cgi${PATH_QUERY}")
RUST_RESP=$(curl -s "http://localhost:8080${PATH_QUERY}")

if [ "$PERL_RESP" = "$RUST_RESP" ]; then
    echo "PASS: $PATH_QUERY"
else
    echo "FAIL: $PATH_QUERY"
    diff <(echo "$PERL_RESP") <(echo "$RUST_RESP")
fi
EOF

./scripts/shadow-http.sh "/?name=World"
./scripts/shadow-http.sh "/?name=Rust&count=3"
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

**Perlの場合: 正規表現が主体なので入力にランダム文字列を多用する:**

```bash
./target/release/perl-to-rust shadow \
    --original "perl ./test-projects/list-util-test.pl" \
    --converted ./output/list-util/target/release/list-util \
    --batch-size 5000 --window 3 --threshold 0.001 \
    --input-type random-strings \
    | tee results/convergence-list-util.txt
```

**期待する出力:**

```
| バッチ | 累積テスト数 | 新規差分 | 発見率 | 判定 |
|--------|------------|---------|--------|------|
| 1      |        5,000 |       34 |   0.7% | 継続 |
| 2      |       10,000 |       12 |   0.2% | 継続 |
| 3      |       15,000 |        3 |   0.1% | 継続 |
| 4      |       20,000 |        0 |   0.0% | 継続 |
| 5      |       25,000 |        0 |   0.0% | 継続 |
| 6      |       30,000 |        0 |   0.0% | ✅ PLATEAU |

→ 30,000件でプラトー到達。Perl/Rust間の挙動完全一致を統計的に証明済み。
```

---

### Step 10: 変換エンジン改善のPR作成

```bash
cd ~/perl-to-rust
git checkout main && git pull origin main
git checkout -b feat/oss-test-improvements

# CPANマッピング表の充実
# cpan-mappings/mappings.toml を更新

# 正規表現変換の精度向上
# crates/perl-parser/src/regex.rs を改善

# wantarray の変換戦略追加
# crates/rust-generator/src/prompt.rs を更新

git add -p
git commit -m "feat: Expand CPAN mappings and improve regex conversion from OSS testing"
git push -u origin feat/oss-test-improvements
```

---

## レポート形式

`results/oss-conversion-report.md` に以下の形式で出力:

```markdown
# perl-to-rust OSS変換テスト結果

実施日: YYYY-MM-DD

## サマリー

| プロジェクト | Perl Ver | 行数 | 変換完走 | cargo check | 出力一致 | TODO数 |
|---|---|---|---|---|---|---|
| List::Util | 5.x | ~500 | ✅ | ✅ | ✅ | 3 |
| Getopt::Long | 5.x | ~1500 | ✅ | ⚠️ (2エラー) | N/A | 12 |
| CGI.pm | 5.x | ~4000 | ✅ | ❌ (25エラー) | N/A | 87 |

## 正規表現変換精度

| テストケース | Perl出力 | Rust出力 | 一致 |
|---|---|---|---|
| 日付キャプチャ | year=2024, month=03, day=08 | year=2024, month=03, day=08 | ✅ |
| グローバル置換 | Goodbye World Goodbye | Goodbye World Goodbye | ✅ |
| 名前付きキャプチャ | level=ERROR, line=42 | level=ERROR, line=42 | ✅ |

## CPANマッピング追加分

| CPANモジュール | Rustクレート | 追加日 |
|---|---|---|
| File::Basename | std::path::Path | YYYY-MM-DD |
| Storable | serde + bincode | YYYY-MM-DD |

## 未対応パターン一覧

（上記テーブルを記載）

## 変換エンジン改善提案

1. **優先度高**: `wantarray` の変換戦略（関数を2つに分割するロジック）
2. **優先度中**: `eval {}` → `std::panic::catch_unwind` の自動変換
3. **優先度低**: `AUTOLOAD` への対処方針
```

---

## 完了条件

- [ ] List::Util の変換が完走し、`cargo check` が通る
- [ ] List::Util の出力がオリジナルPerlと一致する
- [ ] 正規表現変換のテストが全て通る（キャプチャ・置換・名前付きキャプチャ）
- [ ] CPANマッピング表を10件以上追加する
- [ ] `results/oss-conversion-report.md` が出力される
- [ ] 変換エンジンの改善点を特定し、少なくとも1件のPRを作成
- [ ] `cargo test --workspace` が通る
- [ ] `cargo clippy --workspace -- -D warnings` が通る

---

## ブランチ

```bash
cd ~/perl-to-rust
git checkout main && git pull origin main
git checkout -b feat/oss-test-improvements
```

PR作成後、QA #09 レビュー → オーナー承認でマージ。

---

## 判断の優先順位

1. **正規表現が命。** Perlコードの多くは正規表現で構成される。ここの変換精度が全体の品質を決める。List::Utilより正規表現テストを優先してもよい。
2. **CPANマッピングを充実させる。** 実プロジェクトのCPAN依存を調べ、マッピングを追加するだけでも大きな価値がある。コード変換が詰まったらこちらに切り替えよ。
3. **`wantarray`はスキップしてよい。** コンテキスト感度はPerlの最難関。「この関数はコンテキストに依存します」というTODOコメントを生成するだけでも許容。
