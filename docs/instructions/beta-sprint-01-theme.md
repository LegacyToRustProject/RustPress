# ベータスプリント指示書 — #01 テーマ担当

## このスプリントのミッション

Betaに向けて以下の2つを並行で進める:

1. **Gutenberg JS統合** → `docs/instructions/gutenberg-01-theme.md` を参照
2. **テーマ互換拡張** → TT25以外の主要テーマをカバーする

優先度: Gutenberg統合を先に完成させ、その後テーマ互換拡張へ。

---

## Part 1: Gutenberg JS統合

`docs/instructions/gutenberg-01-theme.md` の手順通りに実装する。

**完了条件:**
- ブラウザで `/wp-admin/post-new.php` を開くとGutenbergエディタが表示される
- ブロックの追加・編集・保存が動作する

---

## Part 2: テーマ互換拡張 (B-1)

### 現状
- TT25 (Twenty Twenty-Five): 98.27% 達成済み
- その他のテーマ: 未対応

### ターゲットテーマ（優先度順）

| テーマ | 難易度 | 特徴 |
|--------|--------|------|
| Twenty Twenty-Four (TT24) | 低 | TT25と同系統、FSE |
| Twenty Twenty-Three (TT23) | 低 | クラシックブロックテーマ |
| Twenty Twenty-Two (TT22) | 低 | 最初のFSEテーマ |
| Astra | 中 | 最人気テーマ（100万+インストール） |
| GeneratePress | 中 | 軽量・高速 |
| OceanWP | 中 | 多目的テーマ |
| Storefront | 中 | WooCommerceデフォルト |
| Kadence | 高 | ブロックベース |

### 作業手順

#### Step 1: TT24を追加（最優先）

```bash
# WordPressコンテナからTT24をコピー
docker exec rustpress-1_wordpress_1 ls /var/www/html/wp-content/themes/
```

TT24のtemplate-partsとtheme.jsonを確認し、TT25の実装をベースに:
1. `themes/twentytwentyfour/` ディレクトリを作成
2. `theme.json` をコピー
3. Teraテンプレートを作成（TT25の `templates/` をベースに調整）

#### Step 2: テーマ切り替え機能の確認

`wp_options` の `stylesheet` / `template` でテーマを切り替えられることを確認:
```sql
SELECT option_value FROM wp_options WHERE option_name = 'stylesheet';
```

RustPressが現在のアクティブテーマを正しく読み取って対応するテンプレートを選択することを確認。

#### Step 3: E2Eテストをテーマ別に拡張

`crates/rustpress-e2e/tests/visual_comparison.rs` を確認し、
TT24・TT23用のテストケースを追加。

### 完了条件 (B-1 一部達成)
- [ ] TT24で主要ページ（トップ、投稿、固定ページ、アーカイブ）が97%+表示
- [ ] TT23で主要ページが97%+表示
- [ ] アクティブテーマの自動検出・切り替えが動作
- [ ] `cargo test --workspace --lib --bins` が通る

---

## ブランチ

```bash
cd ~/RustPress
git checkout main && git pull origin main
git checkout -b feat/gutenberg-and-theme-compat
```

PR作成後、QA #09 レビュー → オーナー承認でマージ。
