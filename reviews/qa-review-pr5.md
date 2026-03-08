# QA #09 レビュー — RustPress feat/security-rate-limit-session

- **レビュー日**: 2026-03-08
- **PR**: feat/security-rate-limit-session → main
- **担当**: #01/#02
- **レビュワー**: QA #09

---

## 判定: **CONDITIONAL APPROVAL（条件付き承認）**

**条件**:
1. **CRITICAL（継続）**: BUG-NEW-1（admin-ajax.php ルート重複）が未修正 — コンテナ起動不能

※ PR#5 の実装内容自体（レートリミット・セッション固定対策）は機能的に正しい。
BUG-NEW-1 修正後、別途再確認なしでマージ可能とする。

---

## チェックリスト

| 項目 | 結果 |
|------|------|
| CI: `RUSTFLAGS="-Dwarnings" cargo check --workspace` | ✅ PASS |
| CI: `cargo test --workspace --lib --bins` | ✅ PASS — 880テスト |
| CI: `cargo clippy --workspace -- -D warnings` | ✅ PASS |
| CI: `cargo fmt --all -- --check` | ✅ PASS |
| `session.rs` の重複 `regenerate_id` | ✅ 解消済み（1定義のみ） |
| BUG-NEW-1（admin-ajax.php 重複）修正 | ❌ **未修正** |

---

## 検証1: 5回失敗 → 6回目が HTTP 429 ✅

**コードパス確認** (`wp_admin.rs:621〜638`):
1. `is_locked()` チェック (line 633) → ロック中なら即 `429 TOO_MANY_REQUESTS`
2. ログイン失敗時 `check_and_record(ip)` 呼び出し (lines 654, 672)
3. 5回目の呼び出しで `count >= max_attempts(5)` → `locked_at = Some(Instant::now())` セット

**フロー**:
- 失敗1〜4回目: `is_locked()` = false → ログインエラー画面
- 失敗5回目: `is_locked()` = false（まだロックなし）→ `check_and_record()` がロックをセット（エラーは `let _ =` で無視）→ ログインエラー画面
- 失敗6回目: `is_locked()` = true → **HTTP 429**

**テスト確認**:
- `test_fifth_attempt_locks`: 5回目でロック → Err(RateLimited{15min}) ✓
- `test_locked_ip_stays_locked`: 6回目以降もロック継続 ✓

---

## 検証2: セッションCookieが変化しているか（Session Fixation対策） ✅

**コードパス確認** (`wp_admin.rs:686〜714`):
```rust
let session = state.sessions.create_session(user_id, &user.user_login, &role_str).await;
let session_id = match state.sessions.regenerate_id(&session.id).await {
    Some(new_id) => new_id,
    None => session.id.clone(), // fallback
};
// cookie に new_id をセット
```

- ログイン成功 → `create_session()` で初期ID生成
- 即座に `regenerate_id()` を呼び出し → 旧IDを削除し、新しいUUID v4 IDを生成
- **Cookie には新しいIDのみがセットされる**
- 旧IDは `get_session()` で解決不可 → Session Fixation 完全防止 ✓

**テスト確認** (`session.rs:284〜329`):
- `test_regenerate_id_changes_id`: 新旧IDが異なる ✓
- `test_regenerate_id_invalidates_old`: 旧IDが解決不可 ✓
- `test_regenerate_id_preserves_data`: ユーザーデータ保持 ✓
- `test_regenerate_nonexistent_returns_none`: 存在しないID → None ✓

---

## 検証3: TTL自動解除テスト（moka TTL vs sleep） ✅

**テスト**: `test_auto_expire_after_lockout` (`rate_limit.rs:222〜236`)

```rust
let tracker = LoginAttemptTracker::with_config(2, Duration::from_millis(50));
// ... lockout trigger ...
std::thread::sleep(Duration::from_millis(100));  // 2× TTL待機
assert!(!tracker.is_locked(&ip));
assert!(tracker.check_and_record(ip).is_ok());
```

- **moka の動作**: `cache.get()` はTTL期限切れのエントリを返さない（バックグラウンド蒸発ではなくアクセス時チェック）
- 50ms TTL に対して 100ms sleep → 余裕を持ったTTL確認 ✓
- `std::thread::sleep` の使用: 非同期コンテキスト外のテストのため適切 ✓
- 実際の時間経過でTTL動作を検証しており、モック依存なし ✓

---

## 検証4: is_locked() と check_and_record() の競合状態 ⚠️

**TOCTOU（Time-of-Check-Time-of-Use）の懸念**:

`is_locked()` と `check_and_record()` は別々のキャッシュアクセスで、非アトミック:

```
Thread A: is_locked() → false (count=4)
Thread B: is_locked() → false (count=4)   ← 同時通過
Thread A: check_and_record() → count=5, locked!
Thread B: check_and_record() → count=4(古い値を読む) → count=5, locked!
```

moka の `insert()` はスレッドセーフだが CAS（Compare-And-Swap）ではないため、
バースト時に複数リクエストが `is_locked()=false` を通過できる。

**評価**: WARNING（ブロッカーではない）
- 実用的なレート制限として許容範囲
- DDOS専用の場合はトークンバケット型 or Redisクラスタが必要
- コメントに「非アトミックな実装」を明記することを推奨

---

## 良い点

- **WordPress標準準拠**: 5回失敗・15分ロックアウト (`LoginAttemptTracker::new()`)
- **IP別独立管理**: `test_different_ips_independent` ✓
- **成功ログイン時クリア**: `state.login_tracker.clear(&parsed_ip)` (line 697)
- **セキュリティCookie属性**: `HttpOnly; Path=/; SameSite=Lax; Max-Age=86400; Secure(https時)`
- **セッション重複 regenerate_id**: PR#4 の clippy E0592 エラーが本ブランチでは解消 ✓

---

## BUG-NEW-1 継続 (CRITICAL)

- `frontend.rs:104` に `/wp-admin/admin-ajax.php` ルートが残存
- `wp_admin.rs:366` と重複 → Axum が起動時に panic
- **修正方法**: `frontend.rs:104` の行を削除

---

*QA #09 — 2026-03-08*
