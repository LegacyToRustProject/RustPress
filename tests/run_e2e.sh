#!/bin/bash
# ===========================================================================
# RustPress E2E Test Runner
# ===========================================================================
#
# Compares RustPress behaviour against a real WordPress instance to verify
# that RustPress is a faithful clone.
#
# Usage:
#   ./tests/run_e2e.sh [wordpress_url] [rustpress_url]
#
# Environment variables (override with export before running):
#   WORDPRESS_URL   - WordPress base URL   (default: http://localhost:8081)
#   RUSTPRESS_URL   - RustPress base URL   (default: http://localhost:8080)
#   ADMIN_USER      - Admin username        (default: admin)
#   ADMIN_PASSWORD  - Admin password        (default: password)
#   WEBDRIVER_URL   - Selenium WebDriver    (default: http://localhost:9515)
#
# Prerequisites:
#   1. A running WordPress instance (e.g. via Docker)
#   2. A running RustPress instance (cargo run -p rustpress-server)
#   3. ChromeDriver running on port 9515 (for Selenium tests)
#
# Test categories:
#   api_comparison      - WP REST API structure comparison
#   frontend_comparison - HTML page structure comparison
#   admin_selenium      - Admin panel Selenium browser tests
#   headers_comparison  - HTTP response header comparison
# ===========================================================================

set -euo pipefail

# --- Configuration --------------------------------------------------------

export WORDPRESS_URL="${1:-${WORDPRESS_URL:-http://localhost:8081}}"
export RUSTPRESS_URL="${2:-${RUSTPRESS_URL:-http://localhost:8080}}"
export ADMIN_USER="${ADMIN_USER:-admin}"
export ADMIN_PASSWORD="${ADMIN_PASSWORD:-password}"
export WEBDRIVER_URL="${WEBDRIVER_URL:-http://localhost:9515}"

# --- Banner ---------------------------------------------------------------

echo "========================================================"
echo "  RustPress E2E Comparison Test Suite"
echo "========================================================"
echo ""
echo "  WordPress:  $WORDPRESS_URL"
echo "  RustPress:  $RUSTPRESS_URL"
echo "  Admin User: $ADMIN_USER"
echo "  WebDriver:  $WEBDRIVER_URL"
echo ""

# --- Pre-flight checks ----------------------------------------------------

echo "--- Pre-flight checks ---"

check_server() {
    local name="$1"
    local url="$2"
    if curl -s --max-time 5 -o /dev/null -w "%{http_code}" "$url" | grep -q "200\|301\|302"; then
        echo "  [OK]   $name is reachable at $url"
        return 0
    else
        echo "  [WARN] $name is NOT reachable at $url"
        return 1
    fi
}

WP_OK=true
RP_OK=true
check_server "WordPress" "$WORDPRESS_URL" || WP_OK=false
check_server "RustPress" "$RUSTPRESS_URL" || RP_OK=false

# Check WebDriver
WEBDRIVER_OK=true
if curl -s --max-time 3 -o /dev/null "$WEBDRIVER_URL/status" 2>/dev/null; then
    echo "  [OK]   WebDriver is reachable at $WEBDRIVER_URL"
else
    echo "  [WARN] WebDriver is NOT reachable at $WEBDRIVER_URL (Selenium tests will be skipped)"
    WEBDRIVER_OK=false
fi

echo ""

if [ "$WP_OK" = false ] || [ "$RP_OK" = false ]; then
    echo "WARNING: One or more servers are not reachable."
    echo "Tests that require unavailable servers will be skipped."
    echo ""
fi

# --- Run tests ------------------------------------------------------------

echo "--- Running E2E tests ---"
echo ""

# Run all ignored tests (they are #[ignore] by default and require servers)
# --nocapture ensures eprintln! output is visible for diagnostics

RUST_LOG=warn cargo test -p rustpress-e2e -- --ignored --nocapture 2>&1 | tee /tmp/rustpress-e2e-results.txt

EXIT_CODE=${PIPESTATUS[0]}

echo ""
echo "========================================================"
if [ "$EXIT_CODE" -eq 0 ]; then
    echo "  ALL E2E TESTS PASSED"
else
    echo "  SOME E2E TESTS FAILED (exit code: $EXIT_CODE)"
fi
echo "========================================================"
echo ""
echo "Full output saved to /tmp/rustpress-e2e-results.txt"

exit "$EXIT_CODE"
