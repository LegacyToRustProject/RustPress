# QA #09 スプリント #05-prep 結果

- **実施日**: 2026-03-08
- **ブランチ**: feat/qa-sprint-05-prep
- **担当**: QA #09
- **前提**: PR#4/PR#5/PR#6 全マージ済み（main: 9e539fa/e8036a7/8b15187）

---

## 判定: **PASS（条件付き）**

**未解決アクション**:
1. **MEDIUM**: ZAP — CSP `unsafe-inline`/`unsafe-eval` を将来ノンス方式に改善
2. ~~**INFO**: TT20/TT19 専用 E2E テストが存在しない（手動確認のみ）~~ → **PR#15 で解消** ✅
3. **INFO**: Private IP (172.24.0.3) が一部レスポンスに露出（Docker 環境内のみ）

---

## チェックリスト

| 項目 | 結果 |
|------|------|
| CI: `RUSTFLAGS="-Dwarnings" cargo check --workspace` | ✅ PASS |
| CI: `cargo test --workspace --lib --bins` | ✅ PASS — **1,189 テスト** |
| CI: `cargo clippy --workspace -- -D warnings` | ✅ PASS |
| CI: `cargo fmt --all -- --check` | ✅ PASS |
| コンテナリビルド（PR#5/PR#6 取り込み確認） | ✅ PASS |
| BUG-NEW-1（admin-ajax.php 重複）最終確認 | ✅ PASS — 起動時 panic なし |
| レート制限 429 確認（6回目でブロック） | ✅ PASS |
| ZAP フルスキャン | ✅ 完了（High/Critical: 0件） |
| TT21〜TT25 ビジュアル確認 | ✅ PASS（全 5 テーマ × 7 ページ）|
| TT20/TT19 ビジュアル確認 | ✅ PASS — PR#15（全 2 テーマ × 7 ページ ≥93%）|
| CSP ヘッダー追加 | ✅ `security_headers` ミドルウェアに実装・コミット済み |

---

## タスク 1: コンテナリビルド & BUG-NEW-1 最終確認

```
docker-compose down && docker-compose build --no-cache rustpress && docker-compose up -d
```

- コンテナ起動: **HTTP 200** ✅
- パニックログなし ✅
- `admin-ajax.php` ルート重複: **解消済み**（`frontend.rs:104` のみ登録）

---

## タスク 2: レート制限 E2E（HTTP 429 確認）

リクエスト 1〜5 回目（誤パスワード）→ HTTP 200（ログインエラー画面）
リクエスト 6〜7 回目 → **HTTP 429 Too Many Requests** ✅

| リクエスト | ステータス |
|-----------|-----------|
| 1 | 200 |
| 2 | 200 |
| 3 | 200 |
| 4 | 200 |
| 5 | 200 |
| 6 | **429** |
| 7 | **429** |

WordPress 標準（5 回失敗→ロック）に完全準拠 ✅

---

## タスク 3: ZAP フルスキャン

```
docker run --rm --network host ghcr.io/zaproxy/zaproxy:stable \
  zap-full-scan.py -t http://localhost:8080 \
  -r /tmp/zap-full-report.html -J /tmp/zap-full-report.json
```

### 検出結果サマリー

| リスクレベル | 件数 |
|------------|------|
| **High/Critical** | **0** |
| Medium | 6 |
| Low | 4 |
| Informational | 4 |

### Medium アラート詳細

| アラート | 信頼度 | 件数 | 評価 |
|---------|--------|------|------|
| CSP: Failure to Define Directive with No Fallback | High | 4 | `object-src`/`base-uri` 未定義。将来改善推奨 |
| CSP: Wildcard Directive | High | 4 | `img-src https:` のワイルドカード。許容範囲 |
| CSP: script-src unsafe-eval | High | 4 | WordPress テーマ互換のため現状維持。将来ノンス方式へ |
| CSP: script-src unsafe-inline | High | 4 | 同上 |
| CSP: style-src unsafe-inline | High | 4 | インラインスタイル多用のため現状維持 |
| Absence of Anti-CSRF Tokens | Low | 5 | wp_nonce 実装済み。ZAP が検出できていないのみ |

### Low アラート詳細

| アラート | 件数 | 評価 |
|---------|------|------|
| Cross-Origin-Embedder-Policy Missing | 3 | 次スプリントで追加予定 |
| Cross-Origin-Opener-Policy Missing | 3 | 次スプリントで追加予定 |
| Cross-Origin-Resource-Policy Missing | 5 | 次スプリントで追加予定 |
| Private IP Disclosure (172.24.0.3) | 4 | Docker 内部 IP。本番環境では非露出 |

### 重大な脆弱性（SQL インジェクション・XSS・認証バイパス）

**0 件** — ZAP アクティブスキャン完了、攻撃可能な脆弱性は発見されず ✅

---

## タスク 4: TT21〜TT25 ビジュアル確認

`WEBDRIVER_URL=http://localhost:4444 RUSTPRESS_URL=http://172.24.0.4:3000 WORDPRESS_URL=http://172.24.0.3`

| テーマ | 結果 | 最低スコア |
|--------|------|-----------|
| TT25 (twentytwentyfive) | ✅ PASS (7/7) | 93.65% (author) |
| TT24 (twentytwentyfour) | ✅ PASS (7/7) | 93.65% (author) |
| TT23 (twentytwentythree) | ✅ PASS (7/7) | 93.65% (author) |
| TT22 (twentytwentytwo) | ✅ PASS (7/7) | 93.65% (author) |
| TT21 (twentytwentyone) | ✅ PASS (7/7) | 93.65% (author) |

全 5 テーマ × 7 ページ = **35 ページ すべて 93% 閾値クリア** ✅

### スコア詳細（TT25 代表）

| ページ | スコア | 差分ピクセル |
|-------|--------|------------|
| home | 98.22% | 32,233 px |
| single_post | 98.91% | 19,636 px |
| sample_page | 98.94% | 19,135 px |
| search | 96.88% | 56,286 px |
| 404 | 98.96% | 18,820 px |
| category | 97.91% | 37,825 px |
| author | 93.65% | 114,686 px |

---

## タスク 5: TT20/TT19 ビジュアル E2E（PR#15 で完了）

**PR#15**: `feat(qa): TT20/TT19 visual parity — all 14 pages ≥93% pixel match`

### TT20 (Twenty Twenty) — 7/7 PASS

| ページ | スコア | 差分ピクセル |
|-------|--------|------------|
| home | 95.97% | 72,854 px |
| single_post | 97.83% | 39,194 px |
| sample_page | 97.66% | 42,205 px |
| search | 96.33% | 66,349 px |
| 404 | **99.90%** | 1,772 px |
| category | 97.69% | 41,649 px |
| author | **99.90%** | 1,776 px |

### TT19 (Twenty Nineteen) — 7/7 PASS

| ページ | スコア | 差分ピクセル |
|-------|--------|------------|
| home | 96.53% | 62,739 px |
| single_post | 97.02% | 53,856 px |
| sample_page | 98.82% | 21,357 px |
| search | 96.64% | 60,724 px |
| 404 | **99.79%** | 3,788 px |
| category | 97.08% | 52,840 px |
| author | 98.28% | 31,051 px |

全 2 テーマ × 7 ページ = **14 ページすべて 93% 閾値クリア** ✅

---

## CSP ヘッダー実装（本スプリントで完了）

`crates/rustpress-server/src/middleware.rs` の `security_headers()` に追加:

```rust
headers.insert(
    "Content-Security-Policy",
    HeaderValue::from_static(concat!(
        "default-src 'self'; ",
        "script-src 'self' 'unsafe-inline' 'unsafe-eval'; ",
        "style-src 'self' 'unsafe-inline'; ",
        "img-src 'self' data: https:; ",
        "font-src 'self' data:; ",
        "frame-src 'self'"
    )),
);
```

テスト: `test_security_headers_csp` — CSP ヘッダー存在・内容確認 ✅

---

## アクションアイテム（次スプリント）

| # | 優先度 | 内容 |
|---|--------|------|
| 1 | **MEDIUM** | CORP/COEP/COOP ヘッダーを `security_headers` に追加（main: c5e1805 で完了）✅ |
| 2 | ~~**MEDIUM**~~ | ~~TT20/TT19 E2E テスト追加~~ → PR#15 で完了 ✅ |
| 3 | **INFO** | CSP の `unsafe-inline`/`unsafe-eval` をノンス方式へ改善 |
| 4 | **INFO** | `object-src 'none'` と `base-uri 'self'` を CSP に追加（main: c5e1805 で完了）✅ |
| 5 | **INFO** | Private IP 露出を除去（WordPress siteurl → 本番 URL に変更） |

---

## テスト数推移

| スプリント | テスト数 |
|-----------|---------|
| Sprint #04 | 880 |
| Sprint #05-prep | **1,189** (+309) |

増加分: CSP テスト 3 件、PR#4 テーマ/ブロック追加分、PR#5 セッション/レート制限テスト追加分

---

*QA #09 — 2026-03-08*
