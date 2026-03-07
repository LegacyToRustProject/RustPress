# 開発ワークフロー

## 全体図

```
┌─────────────────────────────────────────────────────┐
│                    リポジトリ構成                       │
├─────────────────────────────────────────────────────┤
│                                                     │
│  LegacyToRustProject/RustPress (本体)               │
│    担当: #01 テーマ, #02 API, #03 セキュリティ        │
│    QA:   #09 (レビュー)                              │
│                                                     │
│  LegacyToRustProject/php-to-rust    ← #04           │
│  LegacyToRustProject/cobol-to-rust  ← #05           │
│  LegacyToRustProject/cpp-to-rust    ← #06           │
│  LegacyToRustProject/java-to-rust   ← #07           │
│  LegacyToRustProject/perl-to-rust   ← #08           │
│                                                     │
└─────────────────────────────────────────────────────┘
```

## 開発フロー

```
┌──────────┐    PR     ┌──────────┐   レポート   ┌──────────┐
│          │ ────────→ │          │ ──────────→ │          │
│ 担当者   │           │  QA #09  │             │ プロジェクト│
│ #01〜#08 │           │          │             │ オーナー   │
│          │ ←──────── │          │ ←────────── │          │
└──────────┘  差し戻し  └──────────┘   判断・指示  └──────────┘
```

### ステップ詳細

```
1. 担当者がfeatureブランチで開発
   $ git checkout -b feat/theme-compat

2. 担当者がPRを作成
   $ git push -u origin feat/theme-compat
   → PR: "feat: Add theme switching mechanism"

3. CI が自動実行（check, test, clippy, fmt）
   → 全ジョブ green 必須

4. QAが検証
   #09 ─→ コードレビュー + 統合テスト

5. 問題があれば → QAがGitHub Issueで報告
   #09 ─→ Issue: "[QA] theme switching breaks REST API routes"

6. プロジェクトオーナーが判断
   オーナー ─→ 修正方針を決定 + 担当者を指定

7. 担当者が修正
   → 修正コミット → PRを更新

8. QAが再検証 → 問題なければApprove

9. プロジェクトオーナーがマージ承認
   オーナー ─→ main にマージ
```

## ブランチ運用

```
main (常にビルド・テスト通過)
 ├── feat/theme-compat      (#01)
 ├── feat/rest-api           (#02)
 ├── feat/security-owasp     (#03)
 ├── fix/logout-session      (#03)
 └── ...
```

### ルール

1. **mainに直接pushしない。** 必ずPR経由。
2. **featureブランチで作業。** 1つのクローンでブランチを切り替える。
3. **PRはQAレビュー + オーナー承認の2段階。**
4. **mainは常にグリーン。** ビルド・テストが通らない状態にしない。
5. **作業前に `git pull origin main` を実行。** 常に最新のmainから分岐する。

## セットアップ

### RustPress本体

```bash
# 初回クローン（全担当者共通）
git clone https://github.com/LegacyToRustProject/RustPress.git ~/RustPress
cd ~/RustPress

# 作業開始時
git checkout main
git pull origin main
git checkout -b feat/<feature-name>

# 作業完了後
git push -u origin feat/<feature-name>
# → GitHub上でPR作成
```

### 変換エンジン

```bash
# 各担当者が自分のリポジトリをクローン
git clone https://github.com/LegacyToRustProject/php-to-rust.git ~/php-to-rust
git clone https://github.com/LegacyToRustProject/cobol-to-rust.git ~/cobol-to-rust
git clone https://github.com/LegacyToRustProject/cpp-to-rust.git ~/cpp-to-rust
git clone https://github.com/LegacyToRustProject/java-to-rust.git ~/java-to-rust
git clone https://github.com/LegacyToRustProject/perl-to-rust.git ~/perl-to-rust
```

全リポジトリ共通ルール:
1. **mainに直接pushしない。** 必ずPR経由。
2. **featureブランチで作業。** 例: `feat/next-iteration`
3. **PRはQAレビュー + オーナー承認の2段階。**
4. **mainは常にグリーン。** ビルド・テストが通らない状態にしない。
5. **GitHub Actions CI** が全リポジトリに設定済み（check, test, fmt, clippy）。PRマージ前にCIグリーン必須。

## CI/CD

全リポジトリに GitHub Actions CI を設定済み。PRおよびmainへのpush時に自動実行:

| ジョブ | コマンド | 必須 |
|--------|---------|------|
| check | `cargo check --workspace` | Yes |
| test | `cargo test --workspace --lib --bins` | Yes |
| clippy | `cargo clippy --workspace -- -D warnings` | Yes |
| fmt | `cargo fmt --all -- --check` | Yes |
| audit | `cargo audit` (RustPress本体のみ) | Yes |
| build | `cargo build --release` (RustPress本体のみ) | Yes |

## Issue管理とトリアージ

すべての報告はGitHub Issuesに集約する。入口は3種類。

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│              GitHub Issues (一元管理)                        │
│                                                             │
│  ┌────────────────┐                                         │
│  │ 一般ユーザー     │─→ バグ報告・機能要望                    │
│  └────────────────┘                                         │
│          │                                                  │
│  ┌────────────────┐                                         │
│  │ QA #09         │─→ コードレビュー指摘・統合テスト不具合     │
│  └────────────────┘                                         │
│          │                                                  │
│  ┌────────────────┐                                         │
│  │ 担当者 #01〜#08 │─→ 他クレートとの統合問題・設計相談       │
│  └────────────────┘                                         │
│          │                                                  │
│          ▼                                                  │
│  ┌────────────────────────────────────────────────────┐     │
│  │            プロジェクトオーナー（トリアージ）          │     │
│  │                                                    │     │
│  │  1. Issueを確認                                    │     │
│  │  2. ラベル付与（重大度 + カテゴリ）                   │     │
│  │  3. 担当者にアサイン                                 │     │
│  │  4. 必要なら方針を議論・決定                          │     │
│  └────────────────────────────────────────────────────┘     │
│          │                                                  │
│          ▼                                                  │
│  担当者が修正 → PR → QA再検証 → マージ                      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### トリアージフロー図

```
Issue作成
  │
  ├── from-qa / from-dev の場合 ──→ 再現確認済み ──→ オーナーへ直行
  │
  ├── from-user の場合
  │     │
  │     ▼
  │   QA #09 が再現確認
  │     │
  │     ├── 再現できた ──→ confirmed ラベル付与 → オーナーへ
  │     │
  │     └── 再現できない ──→ needs-info ラベル → 報告者に質問
  │                           │
  │                           ├── 情報追加で再現 ──→ confirmed → オーナーへ
  │                           └── 応答なし/再現不可 ──→ 30日後クローズ
  │
  ▼
オーナーがトリアージ
  │
  ├── 重複？ ──→ Yes ──→ 既存Issueにリンクして閉じる
  │
  ├── 対応不要？ ──→ Yes ──→ wontfix で閉じる（理由を記載）
  │
  ▼
ラベル付与（重大度 + カテゴリ）
  │
  ▼
担当者アサイン
  │
  ▼
修正開始
  │
  ▼
PR → QAレビュー → マージ → Issueクローズ
```

### Issueラベル体系

**重大度ラベル（必須・1つ選択）:**

| ラベル | 意味 | 対応速度 |
|--------|------|----------|
| `blocker` | ビルド不可・テスト失敗・脆弱性・データ破損 | 即対応 |
| `warning` | 設計不整合・テスト不足・パフォーマンス懸念 | 次のマージ前に対応 |
| `suggestion` | 改善提案・リファクタリング機会 | 任意 |

**カテゴリラベル（必須・1つ以上選択）:**

| ラベル | 対象 |
|--------|------|
| `theme` | テーマ互換 (#01) |
| `api` | REST API・移行・CI (#02) |
| `security` | セキュリティ (#03) |
| `php-to-rust` | PHP変換エンジン (#04) |
| `cobol-to-rust` | COBOL変換エンジン (#05) |
| `cpp-to-rust` | C/C++変換エンジン (#06) |
| `java-to-rust` | Java変換エンジン (#07) |
| `perl-to-rust` | Perl変換エンジン (#08) |
| `integration` | クレート間の統合問題 |

**ソースラベル（必須・1つ選択）:**

| ラベル | 誰が報告したか |
|--------|----------------|
| `from-user` | 一般ユーザー |
| `from-qa` | QA #09 |
| `from-dev` | 担当者 #01〜#08 |

**状態ラベル（任意）:**

| ラベル | 意味 |
|--------|------|
| `needs-info` | 報告者に追加情報を求めている |
| `in-progress` | 修正作業中 |
| `ready-for-review` | 修正完了、QA再検証待ち |

### Issue Template

GitHub上に設定済み（`.github/ISSUE_TEMPLATE/`）:
- **バグ報告**: `bug_report.md`
- **機能要望**: `feature_request.md`

### PR Template

GitHub上に設定済み（`.github/PULL_REQUEST_TEMPLATE.md`）:
- CIチェックリスト（check, test, clippy, fmt）
- QAレビュー確認欄

### CODEOWNERS

`.github/CODEOWNERS` で自動レビュアーを設定済み:
- 全PR → QA #09 が自動アサイン
- クレート別に担当者を指定
