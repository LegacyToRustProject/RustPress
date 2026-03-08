#!/usr/bin/env bash
# WordPress REST API 互換性テスト
# RustPress (:8080) vs WordPress (:8081) の応答比較
#
# Usage:
#   ./scripts/api-compat-test.sh [OPTIONS]
#   WP_URL=http://localhost:8081 RP_URL=http://localhost:8080 JWT_TOKEN=xxx ./scripts/api-compat-test.sh
#
# Exit codes:
#   0  全テスト合格（または通過率が閾値以上）
#   1  クリティカルテスト失敗 / 通過率が閾値未満
#   2  RustPress サーバーに接続できない（テスト不能）

set -euo pipefail

# ─── 設定 ────────────────────────────────────────────────────────────────
WP_URL="${WP_URL:-http://localhost:8081}"
RP_URL="${RP_URL:-http://localhost:8080}"
JWT_TOKEN="${JWT_TOKEN:-}"
WP_USER="${WP_USER:-admin}"
WP_PASS="${WP_PASS:-admin}"
MIN_PASS_RATE="${MIN_PASS_RATE:-60}"   # 通過率の最低閾値 (%) — 環境変数または --min-pass-rate で変更可

# 引数パース
while [[ $# -gt 0 ]]; do
    case "$1" in
        --wp-url)         WP_URL="$2"; shift 2 ;;
        --rp-url)         RP_URL="$2"; shift 2 ;;
        --token)          JWT_TOKEN="$2"; shift 2 ;;
        --wp-user)        WP_USER="$2"; shift 2 ;;
        --wp-pass)        WP_PASS="$2"; shift 2 ;;
        --min-pass-rate)  MIN_PASS_RATE="$2"; shift 2 ;;
        *) echo "Unknown arg: $1"; exit 1 ;;
    esac
done

# ─── カラー出力 ───────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

PASS=0
FAIL=0
SKIP=0
CRITICAL_FAIL=0   # Gutenberg 必須など、1件でも落ちたら exit 1
RESULTS=()
CRITICAL_RESULTS=()

# ─── JWT トークン取得 ─────────────────────────────────────────────────────
if [[ -z "$JWT_TOKEN" ]]; then
    echo -e "${CYAN}JWT トークンを取得中...${NC}"
    JWT_TOKEN=$(curl -s -X POST "${RP_URL}/wp-json/jwt-auth/v1/token" \
        -H "Content-Type: application/json" \
        -d "{\"username\":\"${WP_USER}\",\"password\":\"${WP_PASS}\"}" \
        | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('token',''))" 2>/dev/null || true)
    if [[ -z "$JWT_TOKEN" ]]; then
        echo -e "${YELLOW}警告: JWT トークン取得失敗。認証なしエンドポイントのみテストします。${NC}"
    else
        echo -e "${GREEN}JWT 取得成功${NC}"
    fi
fi

AUTH_HEADER=""
[[ -n "$JWT_TOKEN" ]] && AUTH_HEADER="Authorization: Bearer ${JWT_TOKEN}"

WP_AUTH=""
[[ -n "$WP_USER" ]] && WP_AUTH="-u ${WP_USER}:${WP_PASS}"

# ─── サーバー接続確認 ────────────────────────────────────────────────────
# curl は接続失敗でも -w "%{http_code}" → "000" を出力 (exit 7 を ; true で吸収)
RP_REACHABLE=$(curl -s --connect-timeout 3 -o /dev/null -w "%{http_code}" "${RP_URL}/wp-json" 2>/dev/null; true)
if [[ "$RP_REACHABLE" == "000" ]]; then
    echo -e "${RED}エラー: RustPress サーバー (${RP_URL}) に接続できません。${NC}"
    echo -e "${YELLOW}サーバーを起動してから再実行してください。${NC}"
    exit 2
fi

# ─── ヘルパー関数 ─────────────────────────────────────────────────────────

# JSON から指定キーが存在するか確認
has_key() {
    local json="$1" key="$2"
    echo "$json" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    keys = '${key}'.split('.')
    for k in keys:
        if isinstance(d, list): d = d[0] if d else {}
        d = d.get(k, None)
        if d is None: raise KeyError
    print('yes')
except: print('no')
" 2>/dev/null
}

# JSON の型を確認
get_type() {
    local json="$1" key="$2"
    echo "$json" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    keys = '${key}'.split('.')
    for k in keys:
        if isinstance(d, list): d = d[0] if d else {}
        d = d.get(k, None)
    print(type(d).__name__)
except: print('error')
" 2>/dev/null
}

# HTTP ステータスコードを取得
# curl は接続失敗時も -w "%{http_code}" で "000" を stdout に出力するため
# || フォールバックは不要（二重 "000000" になる）。; true で set -e を回避。
get_status() {
    local url="$1" extra_flags="${2:-}"
    curl -s -o /dev/null -w "%{http_code}" $extra_flags "$url" 2>/dev/null; true
}

# JSON レスポンスを取得（接続失敗時は "{}" を返す）
get_json() {
    local url="$1" extra_flags="${2:-}"
    local body
    body=$(curl -s $extra_flags "$url" 2>/dev/null) || true
    echo "${body:-{}}"
}

# テスト記録 (通常テスト)
record() {
    local status="$1" name="$2" detail="${3:-}"
    if [[ "$status" == "PASS" ]]; then
        PASS=$((PASS+1))
        echo -e "  ${GREEN}✓${NC} $name"
    elif [[ "$status" == "FAIL" ]]; then
        FAIL=$((FAIL+1))
        echo -e "  ${RED}✗${NC} $name"
        [[ -n "$detail" ]] && echo -e "    ${YELLOW}→ $detail${NC}"
    else
        SKIP=$((SKIP+1))
        echo -e "  ${YELLOW}−${NC} $name (スキップ)"
    fi
    RESULTS+=("$status|$name|$detail")
}

# クリティカルテスト記録 — 失敗すると exit 1 が確定する
# Gutenberg 必須エンドポイントなど、これが動かないと編集自体不可能なもの
record_critical() {
    local status="$1" name="$2" detail="${3:-}"
    local label="[CRITICAL] $name"
    if [[ "$status" == "PASS" ]]; then
        PASS=$((PASS+1))
        echo -e "  ${GREEN}✓${NC} ${BOLD}${label}${NC}"
    elif [[ "$status" == "FAIL" ]]; then
        FAIL=$((FAIL+1))
        CRITICAL_FAIL=$((CRITICAL_FAIL+1))
        echo -e "  ${RED}✗${NC} ${BOLD}${label}${NC}"
        [[ -n "$detail" ]] && echo -e "    ${YELLOW}→ $detail${NC}"
    else
        SKIP=$((SKIP+1))
        echo -e "  ${YELLOW}−${NC} ${label} (スキップ)"
    fi
    RESULTS+=("$status|$label|$detail")
    CRITICAL_RESULTS+=("$status|$name|$detail")
}

# ステータスコードテスト
test_status() {
    local name="$1" url="$2" expected="$3" flags="${4:-}"
    local rp_url="${RP_URL}${url}"
    local actual
    actual=$(get_status "$rp_url" "$flags")
    if [[ "$actual" == "$expected" ]]; then
        record PASS "$name (HTTP $expected)"
    else
        record FAIL "$name" "期待: $expected, 実際: $actual (${rp_url})"
    fi
}

# フィールド存在テスト
test_field() {
    local name="$1" url="$2" field="$3" flags="${4:-}"
    local rp_url="${RP_URL}${url}"
    local json result
    json=$(get_json "$rp_url" "$flags")
    result=$(has_key "$json" "$field")
    if [[ "$result" == "yes" ]]; then
        record PASS "$name (.${field} 存在)"
    else
        record FAIL "$name" ".${field} が存在しない (${rp_url})"
    fi
}

# WP vs RP フィールド比較
compare_field() {
    local name="$1" url="$2" field="$3" wp_flags="${4:-}" rp_flags="${5:-}"
    local wp_json rp_json wp_val rp_val
    wp_json=$(get_json "${WP_URL}${url}" "$wp_flags")
    rp_json=$(get_json "${RP_URL}${url}" "$rp_flags")
    wp_val=$(has_key "$wp_json" "$field")
    rp_val=$(has_key "$rp_json" "$field")
    if [[ "$wp_val" == "yes" && "$rp_val" == "yes" ]]; then
        record PASS "$name (.${field} WP/RP 両方存在)"
    elif [[ "$wp_val" == "no" && "$rp_val" == "no" ]]; then
        record PASS "$name (.${field} WP/RP 両方なし)"
    elif [[ "$wp_val" == "yes" && "$rp_val" == "no" ]]; then
        record FAIL "$name" "WP にあるが RP にない: .${field}"
    else
        record PASS "$name (.${field} RP のみ存在 — 追加情報)"
    fi
}

# ─── テスト開始 ───────────────────────────────────────────────────────────
echo ""
echo -e "${BOLD}═══════════════════════════════════════════════════════════${NC}"
echo -e "${BOLD}  WordPress REST API 互換性テスト${NC}"
echo -e "${BOLD}  RustPress: ${RP_URL}${NC}"
echo -e "${BOLD}  WordPress: ${WP_URL}${NC}"
echo -e "${BOLD}═══════════════════════════════════════════════════════════${NC}"
echo ""

# ─── 1. Discovery ────────────────────────────────────────────────────────
echo -e "${BOLD}[1] API Discovery${NC}"
# Discovery は Gutenberg 起動の大前提 → critical
DISC_SC=$(get_status "${RP_URL}/wp-json")
[[ "$DISC_SC" == "200" ]] \
    && record_critical PASS "wp-json ルート (HTTP 200)" \
    || record_critical FAIL "wp-json ルート" "期待: 200, 実際: $DISC_SC"
test_field "namespaces フィールド" "/wp-json" "namespaces"
test_field "routes フィールド" "/wp-json" "routes"
echo ""

# ─── 2. Post Types ───────────────────────────────────────────────────────
echo -e "${BOLD}[2] POST TYPES  /wp-json/wp/v2/types${NC}"
TYPES_URL="/wp-json/wp/v2/types"
TYPES_JSON=$(get_json "${RP_URL}${TYPES_URL}")
TYPES_SC=$(get_status "${RP_URL}${TYPES_URL}")
[[ "$TYPES_SC" == "200" ]] \
    && record_critical PASS "types 200 OK" \
    || record_critical FAIL "types HTTP $TYPES_SC" "期待: 200"
# Gutenberg 初期化必須フィールド → critical
for field in post page; do
    HAS=$(has_key "$TYPES_JSON" "$field")
    [[ "$HAS" == "yes" ]] \
        && record_critical PASS "types.$field エントリ" \
        || record_critical FAIL "types.$field エントリ" "Gutenberg初期化に必須"
done
for field in post.rest_base post.rest_namespace post.labels post.supports post.taxonomies post.viewable; do
    HAS=$(has_key "$TYPES_JSON" "$field")
    [[ "$HAS" == "yes" ]] \
        && record_critical PASS "types.$field" \
        || record_critical FAIL "types.$field" "Gutenberg必須フィールドなし"
done
test_field "attachment エントリ" "$TYPES_URL" "attachment"

# 単体取得
test_status "types/{slug} post" "/wp-json/wp/v2/types/post" "200"
test_status "types/{slug} page" "/wp-json/wp/v2/types/page" "200"
test_status "types/{slug} 404" "/wp-json/wp/v2/types/nonexistent" "404"
echo ""

# ─── 3. Taxonomies ───────────────────────────────────────────────────────
echo -e "${BOLD}[3] TAXONOMIES  /wp-json/wp/v2/taxonomies${NC}"
TAX_URL="/wp-json/wp/v2/taxonomies"
TAX_SC=$(get_status "${RP_URL}${TAX_URL}")
[[ "$TAX_SC" == "200" ]] \
    && record_critical PASS "taxonomies 200 OK" \
    || record_critical FAIL "taxonomies HTTP $TAX_SC" "期待: 200"
TAX_JSON=$(get_json "${RP_URL}${TAX_URL}")
for field in category post_tag; do
    HAS=$(has_key "$TAX_JSON" "$field")
    [[ "$HAS" == "yes" ]] \
        && record_critical PASS "taxonomies.$field エントリ" \
        || record_critical FAIL "taxonomies.$field エントリ" "Gutenberg初期化に必須"
done
test_field "category.rest_base" "$TAX_URL" "category.rest_base"
test_field "category.labels" "$TAX_URL" "category.labels"
echo ""

# ─── 4. Statuses ─────────────────────────────────────────────────────────
echo -e "${BOLD}[4] STATUSES  /wp-json/wp/v2/statuses${NC}"
STAT_URL="/wp-json/wp/v2/statuses"
if [[ -n "$AUTH_HEADER" ]]; then
    test_status "statuses 200" "$STAT_URL" "200" "-H '$AUTH_HEADER'"
    test_field "publish ステータス" "$STAT_URL" "publish" "-H '$AUTH_HEADER'"
    test_field "draft ステータス" "$STAT_URL" "draft" "-H '$AUTH_HEADER'"
else
    record SKIP "statuses テスト (認証必要)"
fi
echo ""

# ─── 5. Themes ───────────────────────────────────────────────────────────
echo -e "${BOLD}[5] THEMES  /wp-json/wp/v2/themes${NC}"
THEMES_URL="/wp-json/wp/v2/themes"
if [[ -n "$AUTH_HEADER" ]]; then
    test_status "themes 200" "$THEMES_URL" "200" "-H '$AUTH_HEADER'"
    THEMES_JSON=$(get_json "${RP_URL}${THEMES_URL}" "-H '$AUTH_HEADER'")
    TH_KEY=$(has_key "$THEMES_JSON" "stylesheet")
    [[ "$TH_KEY" == "yes" ]] && record PASS "themes.stylesheet 存在" || record FAIL "themes.stylesheet" "フィールドなし"
    # theme_supports は Gutenberg ブロックサポート判定に必須 → critical
    TH_KEY=$(has_key "$THEMES_JSON" "theme_supports")
    [[ "$TH_KEY" == "yes" ]] \
        && record_critical PASS "themes.theme_supports 存在" \
        || record_critical FAIL "themes.theme_supports" "Gutenberg必須フィールドなし"
    TH_KEY=$(has_key "$THEMES_JSON" "theme_supports.align-wide")
    [[ "$TH_KEY" == "yes" ]] && record PASS "theme_supports.align-wide" || record FAIL "theme_supports.align-wide" "フィールドなし"
else
    record SKIP "themes テスト (認証必要)"
fi
echo ""

# ─── 6. Posts ────────────────────────────────────────────────────────────
echo -e "${BOLD}[6] POSTS  /wp-json/wp/v2/posts${NC}"
POSTS_URL="/wp-json/wp/v2/posts"
test_status "posts 200 OK" "$POSTS_URL" "200"
POSTS_JSON=$(get_json "${RP_URL}${POSTS_URL}")
# posts は配列
IS_ARRAY=$(echo "$POSTS_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print('yes' if isinstance(d,list) else 'no')" 2>/dev/null || echo "no")
[[ "$IS_ARRAY" == "yes" ]] && record PASS "posts レスポンスが配列" || record FAIL "posts レスポンス型" "配列を期待、オブジェクトを受信"

if [[ "$IS_ARRAY" == "yes" ]]; then
    FIRST_POST=$(echo "$POSTS_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print(json.dumps(d[0]) if d else '{}')" 2>/dev/null || echo "{}")
    for field in id title content excerpt status slug date modified author type link; do
        HAS=$(has_key "$FIRST_POST" "$field")
        [[ "$HAS" == "yes" ]] && record PASS "post.$field" || record FAIL "post.$field" "フィールドなし"
    done
fi

# auto-draft テスト — Gutenberg が新規投稿時に必ず使う → critical
if [[ -n "$AUTH_HEADER" ]]; then
    AUTO_STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "${RP_URL}${POSTS_URL}" \
        -H "$AUTH_HEADER" -H "Content-Type: application/json" \
        -d '{"title":"","content":"","status":"auto-draft"}' 2>/dev/null || echo "000")
    if [[ "$AUTO_STATUS" == "201" ]]; then
        record_critical PASS "auto-draft 投稿作成 (201)"
    else
        record_critical FAIL "auto-draft 投稿作成" "期待: 201, 実際: $AUTO_STATUS"
    fi
fi
echo ""

# ─── 7. Pages ────────────────────────────────────────────────────────────
echo -e "${BOLD}[7] PAGES  /wp-json/wp/v2/pages${NC}"
test_status "pages 200 OK" "/wp-json/wp/v2/pages" "200"
PAGES_JSON=$(get_json "${RP_URL}/wp-json/wp/v2/pages")
IS_ARRAY=$(echo "$PAGES_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print('yes' if isinstance(d,list) else 'no')" 2>/dev/null || echo "no")
[[ "$IS_ARRAY" == "yes" ]] && record PASS "pages レスポンスが配列" || record FAIL "pages レスポンス型" "配列を期待"
echo ""

# ─── 8. Categories ───────────────────────────────────────────────────────
echo -e "${BOLD}[8] CATEGORIES  /wp-json/wp/v2/categories${NC}"
test_status "categories 200 OK" "/wp-json/wp/v2/categories" "200"
CAT_JSON=$(get_json "${RP_URL}/wp-json/wp/v2/categories")
IS_ARRAY=$(echo "$CAT_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print('yes' if isinstance(d,list) else 'no')" 2>/dev/null || echo "no")
[[ "$IS_ARRAY" == "yes" ]] && record PASS "categories 配列" || record FAIL "categories 型" "配列を期待"
if [[ "$IS_ARRAY" == "yes" ]]; then
    FIRST=$(echo "$CAT_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print(json.dumps(d[0]) if d else '{}')" 2>/dev/null || echo "{}")
    for field in id name slug count description link; do
        HAS=$(has_key "$FIRST" "$field")
        [[ "$HAS" == "yes" ]] && record PASS "category.$field" || record FAIL "category.$field" "フィールドなし"
    done
fi
echo ""

# ─── 9. Tags ─────────────────────────────────────────────────────────────
echo -e "${BOLD}[9] TAGS  /wp-json/wp/v2/tags${NC}"
test_status "tags 200 OK" "/wp-json/wp/v2/tags" "200"
echo ""

# ─── 10. Users ───────────────────────────────────────────────────────────
echo -e "${BOLD}[10] USERS  /wp-json/wp/v2/users${NC}"
test_status "users/me 認証あり" "/wp-json/wp/v2/users/me" "200" \
    "$([ -n "$AUTH_HEADER" ] && echo "-H '$AUTH_HEADER'" || echo "")"
test_status "users/me 認証なし → 401" "/wp-json/wp/v2/users/me" "401"

if [[ -n "$AUTH_HEADER" ]]; then
    ME_JSON=$(get_json "${RP_URL}/wp-json/wp/v2/users/me" "-H '$AUTH_HEADER'")
    for field in id name slug email roles capabilities; do
        HAS=$(has_key "$ME_JSON" "$field")
        [[ "$HAS" == "yes" ]] && record PASS "users/me.$field" || record FAIL "users/me.$field" "フィールドなし"
    done
fi
echo ""

# ─── 11. Media ───────────────────────────────────────────────────────────
echo -e "${BOLD}[11] MEDIA  /wp-json/wp/v2/media${NC}"
test_status "media 200 OK" "/wp-json/wp/v2/media" "200"
MEDIA_JSON=$(get_json "${RP_URL}/wp-json/wp/v2/media")
IS_ARRAY=$(echo "$MEDIA_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print('yes' if isinstance(d,list) else 'no')" 2>/dev/null || echo "no")
[[ "$IS_ARRAY" == "yes" ]] && record PASS "media 配列" || record FAIL "media 型" "配列を期待"
echo ""

# ─── 12. Comments ────────────────────────────────────────────────────────
echo -e "${BOLD}[12] COMMENTS  /wp-json/wp/v2/comments${NC}"
test_status "comments 200 OK" "/wp-json/wp/v2/comments" "200"
echo ""

# ─── 13. Settings ────────────────────────────────────────────────────────
echo -e "${BOLD}[13] SETTINGS  /wp-json/wp/v2/settings${NC}"
if [[ -n "$AUTH_HEADER" ]]; then
    test_status "settings 200" "/wp-json/wp/v2/settings" "200" "-H '$AUTH_HEADER'"
    SETTINGS_JSON=$(get_json "${RP_URL}/wp-json/wp/v2/settings" "-H '$AUTH_HEADER'")
    for field in title description url; do
        HAS=$(has_key "$SETTINGS_JSON" "$field")
        [[ "$HAS" == "yes" ]] && record PASS "settings.$field" || record FAIL "settings.$field" "フィールドなし"
    done
else
    record SKIP "settings テスト (認証必要)"
fi
echo ""

# ─── 14. Block Types ─────────────────────────────────────────────────────
echo -e "${BOLD}[14] BLOCK TYPES  /wp-json/wp/v2/block-types${NC}"
test_status "block-types 200 OK" "/wp-json/wp/v2/block-types" "200"
BT_JSON=$(get_json "${RP_URL}/wp-json/wp/v2/block-types")
IS_ARRAY=$(echo "$BT_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print('yes' if isinstance(d,list) else 'no')" 2>/dev/null || echo "no")
[[ "$IS_ARRAY" == "yes" ]] && record PASS "block-types 配列" || record FAIL "block-types 型" "配列を期待"
COUNT=$(echo "$BT_JSON" | python3 -c "import sys,json; d=json.load(sys.stdin); print(len(d) if isinstance(d,list) else 0)" 2>/dev/null || echo "0")
if [[ "$COUNT" -gt 0 ]]; then
    record PASS "block-types 件数: $COUNT"
else
    record FAIL "block-types 件数" "0 件（Gutenberg はブロック定義が必要）"
fi
echo ""

# ─── 15. Block Patterns ──────────────────────────────────────────────────
echo -e "${BOLD}[15] BLOCK PATTERNS  /wp-json/wp/v2/block-patterns/patterns${NC}"
BP_STATUS=$(get_status "${RP_URL}/wp-json/wp/v2/block-patterns/patterns")
if [[ "$BP_STATUS" == "200" ]]; then
    record PASS "block-patterns/patterns 200 OK"
    BPC_STATUS=$(get_status "${RP_URL}/wp-json/wp/v2/block-patterns/categories")
    [[ "$BPC_STATUS" == "200" ]] && record PASS "block-patterns/categories 200 OK" \
        || record FAIL "block-patterns/categories" "HTTP $BPC_STATUS"
elif [[ "$BP_STATUS" == "404" ]]; then
    record FAIL "block-patterns/patterns" "404 (エンドポイント未実装)"
else
    record FAIL "block-patterns/patterns" "HTTP $BP_STATUS"
fi
echo ""

# ─── 16. Search ──────────────────────────────────────────────────────────
echo -e "${BOLD}[16] SEARCH  /wp-json/wp/v2/search${NC}"
SRCH_STATUS=$(get_status "${RP_URL}/wp-json/wp/v2/search?search=hello")
if [[ "$SRCH_STATUS" == "200" ]]; then
    record PASS "search 200 OK"
elif [[ "$SRCH_STATUS" == "404" ]]; then
    record FAIL "search" "404 (エンドポイント未実装)"
else
    record FAIL "search" "HTTP $SRCH_STATUS"
fi
echo ""

# ─── 17. Templates ───────────────────────────────────────────────────────
echo -e "${BOLD}[17] TEMPLATES  /wp-json/wp/v2/templates${NC}"
TMPL_STATUS=$(get_status "${RP_URL}/wp-json/wp/v2/templates")
if [[ "$TMPL_STATUS" == "200" ]]; then
    record PASS "templates 200 OK"
elif [[ "$TMPL_STATUS" == "404" ]]; then
    record FAIL "templates" "404 (エンドポイント未実装)"
else
    record FAIL "templates" "HTTP $TMPL_STATUS"
fi
echo ""

# ─── 18. Nonce / X-WP-Nonce ──────────────────────────────────────────────
echo -e "${BOLD}[18] NONCE 認証  X-WP-Nonce${NC}"
# nonce エンドポイントが存在するか
NONCE_STATUS=$(get_status "${RP_URL}/wp-admin/admin-ajax.php?action=rest-nonce")
if [[ "$NONCE_STATUS" == "200" ]]; then
    record PASS "nonce エンドポイント 200"
else
    record FAIL "nonce エンドポイント" "HTTP $NONCE_STATUS (期待: 200)"
fi

# api_nonce が settings / wp-json に含まれるか
DISC_JSON=$(get_json "${RP_URL}/wp-json")
HAS_NONCE=$(has_key "$DISC_JSON" "api_nonce")
[[ "$HAS_NONCE" == "yes" ]] && record PASS "wp-json.api_nonce 存在" || record FAIL "wp-json.api_nonce" "Gutenberg CSRFトークン用フィールドなし"
echo ""

# ─── 19. JWT Auth ────────────────────────────────────────────────────────
echo -e "${BOLD}[19] JWT 認証  /wp-json/jwt-auth/v1/token${NC}"
JWT_STATUS=$(get_status "${RP_URL}/wp-json/jwt-auth/v1/token" \
    "-X POST -H 'Content-Type: application/json' -d '{\"username\":\"${WP_USER}\",\"password\":\"${WP_PASS}\"}'")
if [[ "$JWT_STATUS" == "200" ]]; then
    record PASS "JWT token 発行 200"
elif [[ "$JWT_STATUS" == "403" ]]; then
    record FAIL "JWT token 発行" "403 (認証失敗 — ユーザー: ${WP_USER})"
else
    record FAIL "JWT token 発行" "HTTP $JWT_STATUS"
fi
echo ""

# ─── 20. WP vs RP フィールド比較 ─────────────────────────────────────────
if curl -s --connect-timeout 2 "${WP_URL}/wp-json" > /dev/null 2>&1; then
    echo -e "${BOLD}[20] WP vs RP 応答比較${NC}"
    compare_field "posts.title.rendered" "/wp-json/wp/v2/posts" "title.rendered" \
        "$WP_AUTH" "$([ -n "$AUTH_HEADER" ] && echo "-H '$AUTH_HEADER'" || echo "")"
    compare_field "posts.content.rendered" "/wp-json/wp/v2/posts" "content.rendered" \
        "$WP_AUTH" "$([ -n "$AUTH_HEADER" ] && echo "-H '$AUTH_HEADER'" || echo "")"
    compare_field "categories.count" "/wp-json/wp/v2/categories" "count" "$WP_AUTH" ""
    compare_field "types.post.labels" "/wp-json/wp/v2/types" "post.labels" "$WP_AUTH" ""
    compare_field "themes.theme_supports" "/wp-json/wp/v2/themes" "theme_supports" \
        "$WP_AUTH" "$([ -n "$AUTH_HEADER" ] && echo "-H '$AUTH_HEADER'" || echo "")"
    echo ""
else
    echo -e "${YELLOW}[20] WordPress (${WP_URL}) 未接続 — WP比較スキップ${NC}"
    echo ""
fi

# ─── サマリー ─────────────────────────────────────────────────────────────
TOTAL=$((PASS + FAIL + SKIP))
RATE=0
[[ $((PASS + FAIL)) -gt 0 ]] && RATE=$(( (PASS * 100) / (PASS + FAIL) ))

echo -e "${BOLD}═══════════════════════════════════════════════════════════${NC}"
echo -e "${BOLD}  テスト結果サマリー${NC}"
echo -e "${BOLD}═══════════════════════════════════════════════════════════${NC}"
echo -e "  総テスト数   : $TOTAL"
echo -e "  ${GREEN}合格${NC}       : $PASS"
echo -e "  ${RED}不合格${NC}     : $FAIL"
echo -e "  ${YELLOW}スキップ${NC}   : $SKIP"
echo ""
echo -e "  ${BOLD}通過率: ${RATE}% (${PASS}/$((PASS+FAIL)))${NC}  (閾値: ${MIN_PASS_RATE}%)"

# クリティカル失敗の集計
CRITICAL_FAIL_NAMES=()
for r in "${CRITICAL_RESULTS[@]}"; do
    IFS='|' read -r status name detail <<< "$r"
    [[ "$status" == "FAIL" ]] && CRITICAL_FAIL_NAMES+=("$name${detail:+ — $detail}")
done

if [[ ${#CRITICAL_FAIL_NAMES[@]} -gt 0 ]]; then
    echo ""
    echo -e "  ${RED}${BOLD}クリティカル失敗 (${#CRITICAL_FAIL_NAMES[@]} 件) — Gutenberg 動作不可:${NC}"
    for name in "${CRITICAL_FAIL_NAMES[@]}"; do
        echo -e "  ${RED}✗${NC} $name"
    done
fi

if [[ $FAIL -gt 0 ]]; then
    echo ""
    echo -e "${BOLD}  不合格一覧:${NC}"
    for r in "${RESULTS[@]}"; do
        IFS='|' read -r status name detail <<< "$r"
        [[ "$status" == "FAIL" ]] && echo -e "  ${RED}✗${NC} $name${detail:+ — $detail}"
    done
fi

echo ""
echo -e "${BOLD}═══════════════════════════════════════════════════════════${NC}"

# ─── 終了コード判定 ───────────────────────────────────────────────────────
# exit 1: クリティカルテスト失敗 OR 通過率が閾値未満
# exit 0: それ以外（通常失敗は警告扱い）
EXIT_CODE=0
if [[ $CRITICAL_FAIL -gt 0 ]]; then
    echo -e "${RED}→ クリティカルテストが ${CRITICAL_FAIL} 件失敗 (exit 1)${NC}"
    EXIT_CODE=1
elif [[ $RATE -lt $MIN_PASS_RATE ]]; then
    echo -e "${RED}→ 通過率 ${RATE}% が閾値 ${MIN_PASS_RATE}% を下回っています (exit 1)${NC}"
    EXIT_CODE=1
else
    [[ $FAIL -gt 0 ]] \
        && echo -e "${YELLOW}→ 通常テストに失敗あり (${FAIL} 件) — クリティカルは全合格 (exit 0)${NC}" \
        || echo -e "${GREEN}→ 全テスト合格 (exit 0)${NC}"
fi
echo ""
exit $EXIT_CODE
