# QA #09 レビュー — RustPress feat/security-xmlrpc-endpoint-hardening

- **レビュー日**: 2026-03-08
- **PR**: feat/security-xmlrpc-endpoint-hardening → main
- **担当**: #01/#02
- **レビュワー**: QA #09

---

## 判定: **CONDITIONAL APPROVAL（条件付き承認）**

**条件**:
1. **CRITICAL（継続）**: BUG-NEW-1（admin-ajax.php ルート重複）が未修正 — コンテナ起動不能

※ PR#6 の実装内容自体（XML-RPC ブロック・オーサー列挙防止）は機能的に正しい。
BUG-NEW-1 修正後、別途再確認なしでマージ可能とする。

---

## チェックリスト

| 項目 | 結果 |
|------|------|
| CI: `RUSTFLAGS="-Dwarnings" cargo check --workspace` | ✅ PASS |
| CI: `cargo test --workspace --lib --bins` | ✅ PASS — 880テスト |
| CI: `cargo clippy --workspace -- -D warnings` | ✅ PASS |
| CI: `cargo fmt --all -- --check` | ✅ PASS |
| BUG-NEW-1（admin-ajax.php 重複）修正 | ❌ **未修正** |

---

## 検証1: GET /xmlrpc.php も 405 を返すか ✅

**ルート定義** (`xmlrpc.rs:38〜46`):
```rust
Router::new().route(
    "/xmlrpc.php",
    get(xmlrpc_blocked)
        .post(xmlrpc_blocked)
        .put(xmlrpc_blocked)
        .delete(xmlrpc_blocked)
        .patch(xmlrpc_blocked),
)
```

- GET, POST, PUT, DELETE, PATCH すべてで `xmlrpc_blocked()` (405) を返す ✓
- HEAD: Axum は GET ハンドラを自動適用 → 405 ✓

**注意**: OPTIONS メソッドは Axum が自動処理するため 200 No Content が返る可能性あり（攻撃者が `Allow: GET,POST,...` ヘッダでXML-RPC存在を発見できる）。実害は軽微だが、将来 `.options(xmlrpc_blocked)` で塞ぐことを推奨。

**テスト確認**:
- `test_xmlrpc_blocked_returns_405`: 405 ✓
- `test_xmlrpc_blocked_body`: "XML-RPC services are disabled" ✓

---

## 検証2: X-Pingback ヘッダがレスポンスに含まれないことを確認 ✅

**コード確認**:
- `xmlrpc_blocked()` は `(StatusCode::METHOD_NOT_ALLOWED, "XML-RPC services are disabled on this site.")` のみ返す — X-Pingback ヘッダなし ✓
- `security_headers` ミドルウェア: X-Content-Type-Options, X-Frame-Options, X-XSS-Protection, Referrer-Policy, Permissions-Policy を追加するが X-Pingback は追加しない ✓
- コードベース全体で `X-Pingback` ヘッダを挿入する箇所なし ✓

**テスト確認**:
```rust
// xmlrpc.rs:2694
async fn test_xmlrpc_blocked_no_pingback_header() {
    let resp = xmlrpc_blocked().await.into_response();
    assert!(resp.headers().get("X-Pingback").is_none())  // ✓
}
```

---

## 検証3: /?author=1 が 403 を返すか ✅

**コードパス確認** (`frontend.rs:650〜658`):
```rust
// ?author=N — block user enumeration (security hardening).
// WordPress normally redirects /?author=1 to /author/{nicename}/, leaking usernames.
if qv.author.is_some() {
    return (
        StatusCode::FORBIDDEN,
        Html("Author enumeration is disabled.".to_string()),
    ).into_response();
}
```

- `/?author=1` → `StatusCode::FORBIDDEN` (403) ✓
- WordPress の `/?author=N → /author/{slug}` リダイレクト（301/302）は **発生しない** ✓
- `qv.author: Option<u64>` — 数値以外は Some にならず問題なし ✓

**⚠️ WARNING**: このコードパスのユニットテストが存在しない。
次スプリントで `test_author_enumeration_returns_403` テストの追加を推奨。

---

## 検証4: test_extract_role_from_serialized_php の混入確認 ✅

**結論**: 別PRの変更の混入ではない。正当な追加。

**根拠**:
- `middleware.rs` はこのPRの変更ファイル (`git diff main...HEAD --name-only` に含まれる)
- `extract_role_from_serialized()` は XML-RPC エンドポイントの認証チェック（ユーザーロール解決）に使用される関数
- テスト `test_extract_role_from_serialized_php` は当該関数の動作を検証するユニットテスト
- 同 `middleware.rs` に含まれる `security_headers` テストも同様にセキュリティハードニングの一部 ✓

---

## 良い点

- **全メソッドブロック**: GET/POST/PUT/DELETE/PATCH すべてを `xmlrpc_blocked()` で統一処理
- **レガシーコード保存**: `xmlrpc_get_handler` / `xmlrpc_post_handler` を dead code として保持（将来のオプション有効化に備える）
- **オーサー列挙完全防止**: リダイレクトではなく直接 403 を返し、username 漏洩を防止
- **`security_headers` ミドルウェア**: X-Content-Type-Options, X-Frame-Options, X-XSS-Protection, Referrer-Policy, Permissions-Policy すべて追加 ✓
- **880テスト全通過**

---

## BUG-NEW-1 継続 (CRITICAL)

- `frontend.rs:104` に `/wp-admin/admin-ajax.php` ルートが残存
- `wp_admin.rs:366` と重複 → Axum が起動時に panic
- **修正方法**: `frontend.rs:104` の行を削除

---

## アクションアイテム

| # | 優先度 | 内容 |
|---|--------|------|
| 1 | **CRITICAL** | `frontend.rs:104` の admin-ajax.php ルートを削除（BUG-NEW-1） |
| 2 | **WARNING** | `/?author=1` の 403 返却テスト追加（`test_author_enumeration_returns_403`） |
| 3 | **INFO** | OPTIONS メソッドの `xmlrpc_blocked` 登録を検討（現在は Axum 自動処理で 200） |

---

*QA #09 — 2026-03-08*
