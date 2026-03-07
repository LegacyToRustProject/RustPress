# ベータスプリント指示書 — #03 セキュリティ担当

## このスプリントのミッション

Betaに向けて以下の2つを進める:

1. **OWASP Top 10 未対応項目の実装** (B-6)
2. **`rustpress migrate` コマンドの整備** (B-5)

---

## Part 1: OWASP Top 10 残存項目 (B-6)

`docs/security-checklist.md` で `[ ]` になっている項目を実装する。

### 優先度高 (Betaブロッカー)

#### 1. ファイルアップロードMIMEタイプ検証

**場所**: `crates/rustpress-server/src/media.rs` および `/wp-json/wp/v2/media` ハンドラ

**実装内容:**
- アップロードされたファイルのContent-Typeヘッダーだけでなく、ファイルのマジックバイトを検証
- 許可するMIMEタイプ: image/jpeg, image/png, image/gif, image/webp, image/avif, application/pdf, video/mp4等
- `infer` クレートを使用してバイト列からMIMEタイプを判定

```rust
// Cargo.toml に追加
infer = "0.16"

// 実装例
use infer;

fn validate_mime_type(bytes: &[u8], declared_content_type: &str) -> Result<String, AppError> {
    let detected = infer::get(bytes)
        .map(|t| t.mime_type())
        .unwrap_or("application/octet-stream");

    let allowed = ["image/jpeg", "image/png", "image/gif", "image/webp",
                   "image/avif", "application/pdf", "video/mp4"];

    if !allowed.contains(&detected) {
        return Err(AppError::BadRequest("File type not allowed".into()));
    }
    Ok(detected.to_string())
}
```

#### 2. パスワードリセットトークンの有効期限強制

**場所**: `crates/rustpress-auth/src/` 内のリセットトークン処理

**現状**: トークンは生成・保存されているが有効期限チェックが不完全

**実装内容:**
- `wp_options` の `_transient_` テーブルに保存されているリセットトークンの有効期限（24時間）を確認
- 期限切れトークンはDB削除してエラー返却
- `wp_admin.rs` の `lost_password_submit` / `reset_password_submit` ハンドラを確認

#### 3. JWTトークンブラックリスト（ログアウト時）

**現状**: JWTはExpiry依存で、ログアウトしても期限内なら有効

**場所**: `crates/rustpress-auth/src/session.rs`

**実装内容:**
- ログアウト時にJWTの `jti`（JWT ID）をブラックリスト（Moka cache）に追加
- API認証ミドルウェアでブラックリストを確認
- TTL: JWTの残り有効期限まで保持

```rust
// crates/rustpress-auth/src/jwt_blacklist.rs (新規)
use moka::sync::Cache;
use std::sync::OnceLock;
use std::time::Duration;

static BLACKLIST: OnceLock<Cache<String, ()>> = OnceLock::new();

pub fn get_blacklist() -> &'static Cache<String, ()> {
    BLACKLIST.get_or_init(|| {
        Cache::builder()
            .time_to_live(Duration::from_secs(86400))
            .build()
    })
}

pub fn blacklist_token(jti: &str) {
    get_blacklist().insert(jti.to_string(), ());
}

pub fn is_blacklisted(jti: &str) -> bool {
    get_blacklist().contains_key(jti)
}
```

### 優先度中 (Beta後でも可)

#### 4. TOTP 2FA (二要素認証)

- `totp-rs` クレートを使用
- 管理画面のプロフィールページでQRコード生成・設定
- ログイン時にTOTPコード入力フォームを追加
- Alpha段階では「実装済みだが任意」で可

#### 5. プラグイン・テーマの署名検証

- WASMプラグインのSHA256ハッシュをマニフェストファイルで検証
- Beta段階では未実装でも許容（Alphaのプラグインは全て内製）

---

## Part 2: `rustpress migrate` コマンド整備 (B-5)

### 目標
```bash
rustpress migrate --from wordpress --db-url mysql://... --site-url https://example.com
```
このコマンド一発でWordPressのサイトが5分以内にRustPressで動く状態にする。

### 現状確認

```bash
cargo run -p rustpress-cli -- --help
cargo run -p rustpress-cli -- migrate --help
```

`crates/rustpress-cli/src/main.rs` と `crates/rustpress-migrate/src/` を確認する。

### 実装すべき機能

#### migrate コマンドのチェックリスト

```
rustpress migrate 実行時のフロー:
1. DB接続確認 → エラーなら分かりやすいメッセージ
2. WordPressバージョン検出 → 非対応バージョンなら警告
3. wp_options から基本設定を読み取り (.env に書き出し)
4. テーマ確認 → 対応テーマなら続行、非対応なら警告+TT25にフォールバック
5. メディアファイルのパス確認
6. 動作確認URL表示 → "RustPress is running at http://localhost:8080"
```

**実装場所**: `crates/rustpress-migrate/src/lib.rs`、`crates/rustpress-cli/src/main.rs`

#### 具体的な実装例

```rust
// crates/rustpress-migrate/src/lib.rs に追加

pub async fn run_migration(config: MigrateConfig) -> Result<MigrationReport> {
    let mut report = MigrationReport::default();

    // Step 1: DB接続確認
    println!("Checking database connection...");
    let pool = connect_db(&config.db_url).await
        .context("Cannot connect to database. Check DB_URL and credentials.")?;
    report.db_connected = true;

    // Step 2: WordPressバージョン検出
    let wp_version = get_wp_version(&pool).await?;
    println!("WordPress version: {}", wp_version);
    if wp_version < "6.0" {
        eprintln!("Warning: WordPress < 6.0 may have compatibility issues");
    }
    report.wp_version = wp_version;

    // Step 3: 設定読み取り
    let options = load_wp_options(&pool).await?;
    report.site_url = options.siteurl.clone();
    report.site_name = options.blogname.clone();

    // Step 4: テーマ確認
    let active_theme = options.stylesheet.clone();
    let supported_themes = ["twentytwentyfive", "twentytwentyfour", "twentytwentythree"];
    if !supported_themes.contains(&active_theme.as_str()) {
        println!("Warning: Theme '{}' not supported. Falling back to TT25.", active_theme);
        report.theme_fallback = true;
    }

    // Step 5: 投稿数カウント
    let post_count = count_posts(&pool).await?;
    println!("Found {} published posts", post_count);
    report.post_count = post_count;

    println!("Migration complete!");
    println!("Start RustPress: cargo run -p rustpress-server");
    println!("Preview: http://localhost:8080");

    Ok(report)
}
```

### .env 自動生成

`migrate` 実行時に `.env` を自動生成する機能:

```rust
fn generate_env_file(options: &WpOptions, config: &MigrateConfig) -> String {
    format!(
        r#"DATABASE_URL={db_url}
SITE_URL={site_url}
JWT_SECRET={jwt_secret}
RUSTPRESS_HOST=0.0.0.0
RUSTPRESS_PORT=3000
"#,
        db_url = config.db_url,
        site_url = options.siteurl,
        jwt_secret = generate_random_secret(),
    )
}
```

### 完了条件 (B-5)
- [ ] `rustpress migrate --db-url mysql://... ` が5分以内に完走
- [ ] 実行後に `cargo run -p rustpress-server` でサイトが表示される
- [ ] エラー時に分かりやすいメッセージが表示される
- [ ] `cargo test --workspace --lib --bins` が通る

---

## ブランチ

```bash
cd ~/RustPress
git checkout main && git pull origin main
git checkout -b feat/owasp-fixes-and-migrate
```

### 作業の進め方

1. OWASP優先度高3項目を実装（ファイルアップロード検証、パスワードリセット有効期限、JWTブラックリスト）
2. `rustpress migrate` コマンドの整備
3. `cargo test --workspace --lib --bins` で確認
4. PRを作成 → QA #09 レビュー → オーナー承認

---

## 参考ファイル

- `crates/rustpress-auth/src/session.rs` — セッション管理
- `crates/rustpress-server/src/media.rs` — メディアアップロード
- `crates/rustpress-server/src/routes/wp_admin.rs` — 管理画面ルート
- `crates/rustpress-migrate/src/lib.rs` — マイグレーション処理
- `crates/rustpress-cli/src/main.rs` — CLIエントリポイント
- `docs/security-checklist.md` — セキュリティチェックリスト全体
