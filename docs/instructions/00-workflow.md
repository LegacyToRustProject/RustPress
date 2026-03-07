# 開発ワークフロー

## 全体図

```
┌─────────────────────────────────────────────────────┐
│                    リポジトリ構成                       │
├─────────────────────────────────────────────────────┤
│                                                     │
│  LegacyToRustProject/RustPress (本体)               │
│  ├── ~/RustPress-theme/     ← #01 テーマ互換        │
│  ├── ~/RustPress-api/       ← #02 API・移行・CI     │
│  ├── ~/RustPress-security/  ← #03 セキュリティ       │
│  └── ~/RustPress-qa/        ← #09 QA (読み取り専用) │
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
1. 担当者が作業ブランチで開発
   #01 ─→ feat/theme-compat ブランチで作業

2. 担当者がPRを作成
   #01 ─→ PR: "feat: Add theme switching mechanism"

3. QAが検証
   #09 ─→ cargo check / test / clippy / audit
   #09 ─→ コードレビュー
   #09 ─→ 統合テスト

4. 問題があれば → QAがGitHub Issueで報告
   #09 ─→ Issue: "[QA] theme switching breaks REST API routes"

5. プロジェクトオーナーが判断
   オーナー ─→ 修正方針を決定
   オーナー ─→ 担当者を指定（元の担当者 or 別の担当者）

6. 担当者が修正
   #01 ─→ 修正コミット → PRを更新

7. QAが再検証 → 問題なければApprove

8. プロジェクトオーナーがマージ承認
   オーナー ─→ main にマージ
```

## ブランチ運用

```
main (常にビルド・テスト通過)
 ├── feat/theme-compat      (#01)
 ├── feat/rest-api           (#02)
 ├── feat/security-owasp     (#03)
 └── ...
```

### ルール

1. **mainに直接pushしない。** 必ずPR経由。
2. **各担当者は自分のクローンで作業。** 同じディレクトリを共有しない。
3. **PRはQAレビュー + オーナー承認の2段階。**
4. **mainは常にグリーン。** ビルド・テストが通らない状態にしない。

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

一般ユーザー向けにテンプレートを用意する:

**バグ報告テンプレート:**
```markdown
## 概要
（何が起きたか）

## 再現手順
1.
2.
3.

## 期待される動作
（本来どうなるべきか）

## 実際の動作
（実際に何が起きたか）

## 環境
- RustPress バージョン:
- OS:
- ブラウザ:
- 元WordPressテーマ:
```

**機能要望テンプレート:**
```markdown
## 概要
（何が欲しいか）

## 動機
（なぜ必要か、どんな場面で使うか）

## WordPress での動作
（WordPressではどう動いているか、該当する場合）
```

## RustPress本体のクローン構成

```bash
# 各担当者の初期セットアップ
git clone https://github.com/LegacyToRustProject/RustPress.git ~/RustPress-theme
cd ~/RustPress-theme
git checkout -b feat/theme-compat

git clone https://github.com/LegacyToRustProject/RustPress.git ~/RustPress-api
cd ~/RustPress-api
git checkout -b feat/rest-api

git clone https://github.com/LegacyToRustProject/RustPress.git ~/RustPress-security
cd ~/RustPress-security
git checkout -b feat/security-owasp

git clone https://github.com/LegacyToRustProject/RustPress.git ~/RustPress-qa
cd ~/RustPress-qa
# QAはmainを見るだけ。ブランチ不要。
```

## 変換エンジンのクローン構成

```bash
# 各担当者が自分のリポジトリをクローン
git clone https://github.com/LegacyToRustProject/php-to-rust.git ~/php-to-rust
git clone https://github.com/LegacyToRustProject/cobol-to-rust.git ~/cobol-to-rust
git clone https://github.com/LegacyToRustProject/cpp-to-rust.git ~/cpp-to-rust
git clone https://github.com/LegacyToRustProject/java-to-rust.git ~/java-to-rust
git clone https://github.com/LegacyToRustProject/perl-to-rust.git ~/perl-to-rust
```

変換エンジンはリポジトリが別なので、ブランチ運用は各担当者の判断に委ねる。
ただし**mainは常にグリーン**のルールは同じ。
