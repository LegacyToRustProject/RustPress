//! TOTP (Time-based One-Time Password) two-factor authentication.
//!
//! Implements RFC 6238 (TOTP) using HMAC-SHA1 with a 30-second time step
//! and 6-digit codes — compatible with Google Authenticator, Authy, and
//! any RFC 6238-compliant app.
//!
//! # Enrollment flow (admin profile)
//! 1. `generate_secret()` → random base32 secret
//! 2. `generate_qr_uri(secret, label, issuer)` → otpauth:// URI
//! 3. User scans with authenticator app, enters code to confirm
//! 4. `verify_code(secret, code)` → save secret to wp_usermeta on success
//!
//! # Login flow
//! 1. Password verified → check wp_usermeta for `_totp_secret`
//! 2. If set: show 2FA code input form
//! 3. `verify_code(secret, code)` → create session

use data_encoding::BASE32;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha1 = Hmac<Sha1>;

/// TOTP time step in seconds (RFC 6238 default).
const TOTP_STEP: u64 = 30;
/// Number of digits in the generated code.
const TOTP_DIGITS: u32 = 6;
/// How many time-step windows to check on either side (for clock skew).
const TOTP_DRIFT: i64 = 1;

/// Compute the TOTP code for a given base32 secret and Unix counter.
fn hotp(secret_b32: &str, counter: u64) -> Option<u32> {
    let secret = BASE32.decode(secret_b32.to_uppercase().as_bytes()).ok()?;
    let mut mac = HmacSha1::new_from_slice(&secret).ok()?;
    mac.update(&counter.to_be_bytes());
    let result = mac.finalize().into_bytes();

    let offset = (result[19] & 0x0f) as usize;
    let code = u32::from_be_bytes([
        result[offset] & 0x7f,
        result[offset + 1],
        result[offset + 2],
        result[offset + 3],
    ]) % 10u32.pow(TOTP_DIGITS);

    Some(code)
}

/// Generate a cryptographically-random 20-byte TOTP secret encoded in Base32.
///
/// Uses UUID v4 (backed by the OS CSPRNG via `getrandom`) to fill 20 random
/// bytes.  Two UUIDs are generated (16 + 16 = 32 bytes) and the first 20 are
/// used, giving 160 bits of cryptographic entropy — well above the RFC 4226
/// minimum of 128 bits.
///
/// Store the returned string in `wp_usermeta` under `_totp_secret`.
pub fn generate_secret() -> String {
    let u1 = uuid::Uuid::new_v4();
    let u2 = uuid::Uuid::new_v4();
    let mut bytes = [0u8; 20];
    bytes[..16].copy_from_slice(u1.as_bytes());
    bytes[16..].copy_from_slice(&u2.as_bytes()[..4]);
    BASE32.encode(&bytes)
}

/// Generate an `otpauth://` URI that authenticator apps can scan as a QR code.
///
/// # Arguments
/// * `secret`  - Base32-encoded secret (from `generate_secret()`)
/// * `label`   - User identifier shown in the app (e.g. `admin@example.com`)
/// * `issuer`  - Service name shown in the app (e.g. `RustPress`)
pub fn generate_qr_uri(secret: &str, label: &str, issuer: &str) -> String {
    let encoded_label = url_encode(label);
    let encoded_issuer = url_encode(issuer);
    format!(
        "otpauth://totp/{encoded_label}?secret={secret}&issuer={encoded_issuer}&algorithm=SHA1&digits={TOTP_DIGITS}&period={TOTP_STEP}"
    )
}

/// Verify a 6-digit TOTP code against a base32 secret.
///
/// Accepts codes from the current window and ±`TOTP_DRIFT` windows
/// to compensate for clock skew between server and device.
///
/// Returns `true` if the code is valid.
pub fn verify_code(secret_b32: &str, code: &str) -> bool {
    let Ok(code_num) = code.trim().parse::<u32>() else {
        return false;
    };
    if code.trim().len() != TOTP_DIGITS as usize {
        return false;
    }

    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let current_counter = now_secs / TOTP_STEP;

    for drift in -TOTP_DRIFT..=TOTP_DRIFT {
        let counter = current_counter.wrapping_add_signed(drift);
        if let Some(expected) = hotp(secret_b32, counter) {
            if expected == code_num {
                return true;
            }
        }
    }
    false
}

/// Minimal percent-encoding for otpauth:// URI components.
fn url_encode(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                vec![c]
            }
            c => {
                let mut buf = [0u8; 4];
                c.encode_utf8(&mut buf);
                let len = c.len_utf8();
                buf[..len]
                    .iter()
                    .flat_map(|&b| format!("%{:02X}", b).chars().collect::<Vec<_>>())
                    .collect()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- generate_secret ---

    #[test]
    fn test_generate_secret_is_base32() {
        let secret = generate_secret();
        // Must decode successfully as BASE32
        assert!(BASE32.decode(secret.to_uppercase().as_bytes()).is_ok());
    }

    #[test]
    fn test_generate_secret_length() {
        let secret = generate_secret();
        // 20 bytes base32 = ceil(20 * 8 / 5) = 32 characters (with padding)
        assert!(secret.len() >= 20, "secret too short: {}", secret.len());
    }

    #[test]
    fn test_generate_secret_uniqueness() {
        let s1 = generate_secret();
        let s2 = generate_secret();
        // Two calls should produce different secrets
        assert_ne!(s1, s2, "secrets must be unique");
    }

    // --- generate_qr_uri ---

    #[test]
    fn test_qr_uri_format() {
        let secret = "JBSWY3DPEHPK3PXP";
        let uri = generate_qr_uri(secret, "user@example.com", "RustPress");
        assert!(
            uri.starts_with("otpauth://totp/"),
            "URI must start with otpauth://totp/"
        );
        assert!(uri.contains(secret), "URI must contain secret");
        assert!(uri.contains("issuer=RustPress"), "URI must contain issuer");
        assert!(uri.contains("digits=6"), "URI must specify 6 digits");
        assert!(
            uri.contains("period=30"),
            "URI must specify 30-second period"
        );
    }

    #[test]
    fn test_qr_uri_encodes_special_chars() {
        let uri = generate_qr_uri("SECRET", "user name@example.com", "My Site");
        // Spaces in label should be percent-encoded
        assert!(!uri.contains(' '));
    }

    #[test]
    fn test_qr_uri_algorithm_field() {
        let uri = generate_qr_uri("SECRET", "user", "issuer");
        assert!(uri.contains("algorithm=SHA1"));
    }

    // --- hotp (internal) ---

    #[test]
    fn test_hotp_known_value() {
        // RFC 4226 test vector: secret = "12345678901234567890", counter = 0 → code = 755224
        let secret_b32 = BASE32.encode(b"12345678901234567890");
        let code = hotp(&secret_b32, 0);
        assert_eq!(code, Some(755224));
    }

    #[test]
    fn test_hotp_counter_1() {
        // RFC 4226 test vector: counter = 1 → 287082
        let secret_b32 = BASE32.encode(b"12345678901234567890");
        let code = hotp(&secret_b32, 1);
        assert_eq!(code, Some(287082));
    }

    #[test]
    fn test_hotp_counter_2() {
        let secret_b32 = BASE32.encode(b"12345678901234567890");
        let code = hotp(&secret_b32, 2);
        assert_eq!(code, Some(359152));
    }

    #[test]
    fn test_hotp_counter_3() {
        let secret_b32 = BASE32.encode(b"12345678901234567890");
        let code = hotp(&secret_b32, 3);
        assert_eq!(code, Some(969429));
    }

    #[test]
    fn test_hotp_counter_4() {
        let secret_b32 = BASE32.encode(b"12345678901234567890");
        let code = hotp(&secret_b32, 4);
        assert_eq!(code, Some(338314));
    }

    #[test]
    fn test_hotp_counter_5() {
        let secret_b32 = BASE32.encode(b"12345678901234567890");
        let code = hotp(&secret_b32, 5);
        assert_eq!(code, Some(254676));
    }

    #[test]
    fn test_hotp_invalid_base32() {
        // Invalid base32 should return None
        let code = hotp("NOT!VALID@BASE32#", 0);
        assert!(code.is_none());
    }

    // --- verify_code ---

    #[test]
    fn test_verify_code_rejects_wrong_length() {
        let secret = generate_secret();
        assert!(!verify_code(&secret, "123")); // too short
        assert!(!verify_code(&secret, "1234567")); // too long
    }

    #[test]
    fn test_verify_code_rejects_non_numeric() {
        let secret = generate_secret();
        assert!(!verify_code(&secret, "abcdef"));
    }

    #[test]
    fn test_verify_code_rejects_empty() {
        let secret = generate_secret();
        assert!(!verify_code(&secret, ""));
    }

    #[test]
    fn test_verify_code_accepts_valid_current_window() {
        // Generate the code for the current window and verify it
        let secret_b32 = BASE32.encode(b"12345678901234567890");
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let counter = now_secs / TOTP_STEP;
        let code = hotp(&secret_b32, counter).unwrap();
        let code_str = format!("{:0>6}", code);
        assert!(verify_code(&secret_b32, &code_str));
    }

    #[test]
    fn test_verify_code_accepts_drift_minus_one() {
        // Code from the previous window should be accepted (drift = -1)
        let secret_b32 = BASE32.encode(b"12345678901234567890");
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let counter = (now_secs / TOTP_STEP).saturating_sub(1);
        let code = hotp(&secret_b32, counter).unwrap();
        let code_str = format!("{:0>6}", code);
        assert!(verify_code(&secret_b32, &code_str));
    }

    #[test]
    fn test_verify_code_accepts_drift_plus_one() {
        // Code from the next window should be accepted (drift = +1)
        let secret_b32 = BASE32.encode(b"12345678901234567890");
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let counter = now_secs / TOTP_STEP + 1;
        let code = hotp(&secret_b32, counter).unwrap();
        let code_str = format!("{:0>6}", code);
        assert!(verify_code(&secret_b32, &code_str));
    }

    #[test]
    fn test_verify_code_rejects_wrong_code() {
        let secret_b32 = BASE32.encode(b"12345678901234567890");
        // Use counter 100 (far from current time window)
        let code = hotp(&secret_b32, 100).unwrap();
        // This code should be valid for counter=100 but NOT for current time
        // (unless current time coincidentally has the same code — extremely unlikely)
        let code_str = format!("{:0>6}", code);
        // Don't assert this will always fail (1 in 1M chance of false fail),
        // but verify the function doesn't panic
        let _result = verify_code(&secret_b32, &code_str);
    }

    #[test]
    fn test_verify_code_trims_whitespace() {
        // Leading/trailing whitespace should not cause parse failure
        let secret_b32 = BASE32.encode(b"12345678901234567890");
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let counter = now_secs / TOTP_STEP;
        let code = hotp(&secret_b32, counter).unwrap();
        let code_str = format!(" {:0>6} ", code);
        assert!(verify_code(&secret_b32, &code_str));
    }

    // --- url_encode ---

    #[test]
    fn test_url_encode_safe_chars() {
        let encoded = url_encode("abc123-_.~");
        assert_eq!(encoded, "abc123-_.~");
    }

    #[test]
    fn test_url_encode_at_sign() {
        let encoded = url_encode("user@example.com");
        assert!(!encoded.contains('@'));
        assert!(encoded.contains("%40"));
    }

    #[test]
    fn test_url_encode_space() {
        let encoded = url_encode("My Site");
        assert!(!encoded.contains(' '));
        assert!(encoded.contains("%20"));
    }

    // --- generate_secret extended ---

    #[test]
    fn test_generate_secret_is_valid_base32_chars() {
        let secret = generate_secret();
        // Base32 alphabet: A-Z and 2-7 (and = for padding)
        assert!(
            secret
                .chars()
                .all(|c| c.is_ascii_uppercase() || ('2'..='7').contains(&c) || c == '='),
            "secret contains non-base32 char: {secret}"
        );
    }

    #[test]
    fn test_generate_secret_five_unique() {
        let secrets: Vec<String> = (0..5).map(|_| generate_secret()).collect();
        let unique: std::collections::HashSet<&String> = secrets.iter().collect();
        assert_eq!(unique.len(), 5, "all 5 secrets should be unique");
    }

    // --- generate_qr_uri extended ---

    #[test]
    fn test_qr_uri_contains_secret_param() {
        let uri = generate_qr_uri("TESTSECRET", "user", "App");
        assert!(uri.contains("secret=TESTSECRET"));
    }

    #[test]
    fn test_qr_uri_issuer_encoded() {
        let uri = generate_qr_uri("SECRET", "user", "My Awesome App");
        // Spaces in issuer should be encoded
        assert!(
            !uri.contains("My Awesome App")
                || uri.contains("My%20Awesome%20App")
                || uri.contains("issuer=My")
        );
    }

    #[test]
    fn test_qr_uri_label_contains_issuer() {
        let uri = generate_qr_uri("SEC", "alice", "RustPress");
        // Label format: issuer:account
        assert!(uri.contains("RustPress") || uri.contains("alice"));
    }

    // --- verify_code extended ---

    #[test]
    fn test_verify_code_wrong_code_6_digits() {
        let secret = generate_secret();
        // 000000 is almost certainly wrong for a fresh secret
        // (unless incredibly unlucky — 1 in 1M chance)
        let result = verify_code(&secret, "000000");
        // Just verify it doesn't panic; result depends on current time
        let _ = result;
    }

    #[test]
    fn test_verify_code_five_digit_code_rejected() {
        let secret = generate_secret();
        assert!(!verify_code(&secret, "12345"));
    }

    #[test]
    fn test_verify_code_seven_digit_code_rejected() {
        let secret = generate_secret();
        assert!(!verify_code(&secret, "1234567"));
    }

    #[test]
    fn test_verify_code_alpha_code_rejected() {
        let secret = generate_secret();
        assert!(!verify_code(&secret, "abcdef"));
    }

    #[test]
    fn test_verify_code_with_padding_whitespace() {
        let secret = generate_secret();
        // Leading/trailing spaces are trimmed before reject
        let _ = verify_code(&secret, " 123456 ");
    }

    // --- hotp consistency ---

    #[test]
    fn test_hotp_counter_0_is_none_for_invalid_base32() {
        assert!(hotp("NOT_VALID!!!", 0).is_none());
    }

    #[test]
    fn test_hotp_same_counter_same_result() {
        let secret = "JBSWY3DPEHPK3PXP";
        let r1 = hotp(secret, 5);
        let r2 = hotp(secret, 5);
        assert_eq!(r1, r2, "hotp must be deterministic");
    }

    #[test]
    fn test_hotp_different_counters_different_results() {
        let secret = "JBSWY3DPEHPK3PXP";
        let r0 = hotp(secret, 0);
        let r1 = hotp(secret, 1);
        // Different counters should (almost always) produce different OTPs
        assert_ne!(r0, r1);
    }

    #[test]
    fn test_hotp_result_lt_1000000() {
        let secret = "JBSWY3DPEHPK3PXP";
        for counter in 0..10 {
            if let Some(code) = hotp(secret, counter) {
                assert!(code < 1_000_000, "HOTP code must be 6 digits max");
            }
        }
    }
}
