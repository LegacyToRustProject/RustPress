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
export SCREENSHOT_DIR="${SCREENSHOT_DIR:-test-screenshots}"

# --- Banner ---------------------------------------------------------------

echo "========================================================"
echo "  RustPress E2E Comparison Test Suite"
echo "========================================================"
echo ""
echo "  WordPress:    $WORDPRESS_URL"
echo "  RustPress:    $RUSTPRESS_URL"
echo "  Admin User:   $ADMIN_USER"
echo "  WebDriver:    $WEBDRIVER_URL"
echo "  Screenshots:  $SCREENSHOT_DIR"
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

# Create screenshot output directory
mkdir -p "$SCREENSHOT_DIR"

# --- Run tests ------------------------------------------------------------

TEST_FILTER="${1:-}"

if [ "$TEST_FILTER" = "visual" ]; then
    echo "--- Running VISUAL comparison tests only ---"
    echo ""
    RUST_LOG=warn cargo test -p rustpress-e2e visual -- --ignored --nocapture 2>&1 | tee /tmp/rustpress-e2e-results.txt
elif [ "$TEST_FILTER" = "api" ]; then
    echo "--- Running API comparison tests only ---"
    echo ""
    RUST_LOG=warn cargo test -p rustpress-e2e api -- --ignored --nocapture 2>&1 | tee /tmp/rustpress-e2e-results.txt
elif [ "$TEST_FILTER" = "frontend" ]; then
    echo "--- Running frontend comparison tests only ---"
    echo ""
    RUST_LOG=warn cargo test -p rustpress-e2e frontend -- --ignored --nocapture 2>&1 | tee /tmp/rustpress-e2e-results.txt
else
    echo "--- Running ALL E2E tests ---"
    echo ""
    echo "Hint: pass 'visual', 'api', or 'frontend' as first arg to filter."
    echo ""
    RUST_LOG=warn cargo test -p rustpress-e2e -- --ignored --nocapture 2>&1 | tee /tmp/rustpress-e2e-results.txt
fi

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

if [ -d "$SCREENSHOT_DIR" ] && [ "$(ls -A "$SCREENSHOT_DIR" 2>/dev/null)" ]; then
    echo "Screenshots and diff images saved to: $SCREENSHOT_DIR/"
    echo "Files:"
    ls -lh "$SCREENSHOT_DIR"/*.png 2>/dev/null || true
fi

exit "$EXIT_CODE"
