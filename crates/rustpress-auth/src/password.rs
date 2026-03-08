use argon2::{
    password_hash::{
        rand_core::OsRng, PasswordHash, PasswordHasher as ArgonHasher, PasswordVerifier, SaltString,
    },
    Argon2,
};
use md5::{Digest, Md5};
use thiserror::Error;
use tracing::debug;

/// The itoa64 alphabet used by PHPass for custom base64 encoding.
const ITOA64: &[u8] = b"./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

#[derive(Error, Debug)]
pub enum PasswordError {
    #[error("Password hash error: {0}")]
    Hash(String),
    #[error("Password verification failed")]
    VerificationFailed,
}

/// Password hasher supporting Argon2 (new) and bcrypt (legacy WP compatibility).
pub struct PasswordHasher;

impl PasswordHasher {
    /// Hash a password using Argon2id (recommended for new users).
    pub fn hash_argon2(password: &str) -> Result<String, PasswordError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| PasswordError::Hash(e.to_string()))?;
        Ok(hash.to_string())
    }

    /// Hash a password using bcrypt (WordPress legacy compatibility).
    pub fn hash_bcrypt(password: &str) -> Result<String, PasswordError> {
        bcrypt::hash(password, bcrypt::DEFAULT_COST).map_err(|e| PasswordError::Hash(e.to_string()))
    }

    /// Verify a password against a stored hash.
    /// Automatically detects hash type (Argon2, bcrypt, or WordPress PHPass).
    pub fn verify(password: &str, hash: &str) -> Result<bool, PasswordError> {
        // WordPress 6.8+ argon2id hashes start with $wp$
        // Format: $wp$argon2id$v=19$m=65536,t=1,p=1$<salt>$<hash>
        if let Some(inner) = hash.strip_prefix("$wp$") {
            debug!("verifying WordPress 6.8+ argon2id hash");
            let parsed =
                PasswordHash::new(inner).map_err(|e| PasswordError::Hash(e.to_string()))?;
            return Ok(Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok());
        }

        // Argon2 hashes start with $argon2
        if hash.starts_with("$argon2") {
            debug!("verifying argon2 hash");
            let parsed = PasswordHash::new(hash).map_err(|e| PasswordError::Hash(e.to_string()))?;
            return Ok(Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok());
        }

        // bcrypt hashes start with $2a$, $2b$, or $2y$
        if hash.starts_with("$2a$") || hash.starts_with("$2b$") || hash.starts_with("$2y$") {
            debug!("verifying bcrypt hash");
            return bcrypt::verify(password, hash).map_err(|e| PasswordError::Hash(e.to_string()));
        }

        // WordPress PHPass hashes start with $P$ or $H$
        if hash.starts_with("$P$") || hash.starts_with("$H$") {
            debug!("verifying WordPress PHPass hash");
            return phpass_verify(password, hash);
        }

        // MD5 hashes (very old WordPress)
        if hash.len() == 32 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
            debug!("verifying legacy MD5 hash");
            let mut hasher = Md5::new();
            hasher.update(password.as_bytes());
            let result = hasher.finalize();
            let computed = format!("{result:x}");
            return Ok(computed == hash.to_lowercase());
        }

        Ok(false)
    }

    /// Upgrade a password hash to Argon2 if it's using an older algorithm.
    pub fn needs_rehash(hash: &str) -> bool {
        // $argon2 = RustPress native, $wp$ = WordPress 6.8+ argon2id — both are modern
        !hash.starts_with("$argon2") && !hash.starts_with("$wp$")
    }
}

/// Verify a password against a WordPress PHPass hash.
///
/// PHPass hash format: `$P$` or `$H$` + iteration char + 8-byte salt + 22-char encoded hash
///
/// The algorithm:
/// 1. Extract the iteration count from character at position 3 (index into ITOA64)
/// 2. Iteration count = 1 << (index in ITOA64)
/// 3. Extract the 8-byte salt from positions 4..12
/// 4. Compute: digest = MD5(salt + password)
/// 5. Iterate `count` times: digest = MD5(digest + password)
/// 6. Encode the 16-byte digest using PHPass custom base64
/// 7. Compare encoded result with stored hash characters at positions 12..34
fn phpass_verify(password: &str, hash: &str) -> Result<bool, PasswordError> {
    let hash_bytes = hash.as_bytes();

    // Hash must be at least 34 characters: $P$ + iter_char + 8 salt + 22 encoded
    if hash_bytes.len() < 34 {
        return Ok(false);
    }

    // Get iteration count from character at position 3
    let iter_char = hash_bytes[3];
    let count_log2 = match ITOA64.iter().position(|&c| c == iter_char) {
        Some(pos) => pos,
        None => return Ok(false),
    };
    let count: u64 = 1u64 << count_log2;

    // Extract salt (positions 4..12, 8 bytes)
    let salt = &hash_bytes[4..12];

    // Compute initial MD5: MD5(salt + password)
    let mut hasher = Md5::new();
    hasher.update(salt);
    hasher.update(password.as_bytes());
    let mut digest = hasher.finalize();

    // Iterate: digest = MD5(digest + password)
    for _ in 0..count {
        let mut hasher = Md5::new();
        hasher.update(digest);
        hasher.update(password.as_bytes());
        digest = hasher.finalize();
    }

    // Encode using PHPass custom base64
    let encoded = phpass_encode64(&digest);

    // The stored hash portion is at positions 12..34 (22 characters for 16 bytes)
    let stored_encoded = &hash[12..34];

    Ok(constant_time_eq(&encoded, stored_encoded))
}

/// Constant-time string comparison to prevent timing attacks.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }
    result == 0
}

/// Encode bytes using PHPass's custom base64 encoding (itoa64 alphabet).
///
/// This processes 3 bytes at a time, producing 4 output characters per group.
/// For the last group, only the necessary characters are emitted:
/// - 1 remaining byte  -> 2 characters
/// - 2 remaining bytes -> 3 characters
/// - 3 remaining bytes -> 4 characters
fn phpass_encode64(input: &[u8]) -> String {
    let mut output = String::new();
    let mut i = 0;
    let len = input.len();

    while i < len {
        // First byte is always available
        let mut value = input[i] as u32;
        output.push(ITOA64[(value & 0x3f) as usize] as char);
        i += 1;

        if i < len {
            value |= (input[i] as u32) << 8;
        }
        output.push(ITOA64[((value >> 6) & 0x3f) as usize] as char);

        if i >= len {
            break;
        }
        i += 1;

        if i < len {
            value |= (input[i] as u32) << 16;
        }
        output.push(ITOA64[((value >> 12) & 0x3f) as usize] as char);

        if i >= len {
            break;
        }
        i += 1;

        output.push(ITOA64[((value >> 18) & 0x3f) as usize] as char);
    }

    output
}

/// Password strength requirements.
///
/// Enforces minimum standards to prevent weak passwords (OWASP A07).
pub struct PasswordPolicy;

impl PasswordPolicy {
    /// Minimum password length.
    pub const MIN_LENGTH: usize = 8;

    /// Validate a password meets the minimum strength requirements.
    ///
    /// Returns `Ok(())` if strong enough, or `Err(reason)` describing the failure.
    pub fn validate(password: &str) -> Result<(), String> {
        if password.len() < Self::MIN_LENGTH {
            return Err(format!(
                "Password must be at least {} characters long",
                Self::MIN_LENGTH
            ));
        }

        let has_upper = password.chars().any(|c| c.is_ascii_uppercase());
        let has_lower = password.chars().any(|c| c.is_ascii_lowercase());
        let has_digit = password.chars().any(|c| c.is_ascii_digit());
        let has_special = password.chars().any(|c| !c.is_alphanumeric());

        let complexity = [has_upper, has_lower, has_digit, has_special]
            .iter()
            .filter(|&&b| b)
            .count();

        if complexity < 3 {
            return Err(
                "Password must contain at least 3 of: uppercase, lowercase, digit, special character"
                    .to_string(),
            );
        }

        // Check common passwords
        let common = [
            "password", "12345678", "qwerty12", "admin123", "letmein1", "welcome1", "monkey12",
            "dragon12", "master12", "abc12345",
        ];
        let lower = password.to_lowercase();
        for weak in &common {
            if lower == *weak {
                return Err("Password is too common".to_string());
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_argon2_hash_and_verify() {
        let password = "test_password_123";
        let hash = PasswordHasher::hash_argon2(password).unwrap();
        assert!(hash.starts_with("$argon2"));
        assert!(PasswordHasher::verify(password, &hash).unwrap());
        assert!(!PasswordHasher::verify("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_bcrypt_hash_and_verify() {
        let password = "test_password_123";
        let hash = PasswordHasher::hash_bcrypt(password).unwrap();
        assert!(PasswordHasher::verify(password, &hash).unwrap());
    }

    #[test]
    fn test_needs_rehash() {
        let argon2_hash = PasswordHasher::hash_argon2("test").unwrap();
        let bcrypt_hash = PasswordHasher::hash_bcrypt("test").unwrap();

        assert!(!PasswordHasher::needs_rehash(&argon2_hash));
        assert!(PasswordHasher::needs_rehash(&bcrypt_hash));
    }

    // --- PHPass verification tests ---

    #[test]
    fn test_phpass_verify_known_hash_development() {
        // Known WordPress PHPass hash for "development" (from clausehound/phpass test suite)
        // Iteration char 'B' = position 13 in itoa64, count = 2^13 = 8192
        let hash = "$P$BgUdq1RzEBYd9Tm/uZC7mz/l5F.x4N1";
        assert!(PasswordHasher::verify("development", hash).unwrap());
    }

    #[test]
    fn test_phpass_verify_known_hash_password() {
        // PHPass hash for "password" with salt "N2VnRfBC" (computed with verified algorithm)
        let hash = "$P$BN2VnRfBC8FCdA45VHtLYIT9olJZl3/";
        assert!(PasswordHasher::verify("password", hash).unwrap());
    }

    #[test]
    fn test_phpass_verify_known_hash_password_alt_salt() {
        // PHPass hash for "password" with salt "testsal1" (computed with verified algorithm)
        let hash = "$P$Btestsal1EwriX7RPMrpXrhtqE/w7R1";
        assert!(PasswordHasher::verify("password", hash).unwrap());
    }

    #[test]
    fn test_phpass_verify_wrong_password() {
        let hash = "$P$BgUdq1RzEBYd9Tm/uZC7mz/l5F.x4N1";
        assert!(!PasswordHasher::verify("wrong_password", hash).unwrap());
    }

    #[test]
    fn test_phpass_verify_empty_password_fails() {
        let hash = "$P$BgUdq1RzEBYd9Tm/uZC7mz/l5F.x4N1";
        assert!(!PasswordHasher::verify("", hash).unwrap());
    }

    #[test]
    fn test_phpass_verify_h_prefix() {
        // $H$ is an alternative prefix used by phpBB and some WordPress installs.
        // The algorithm is identical to $P$, only the prefix differs.
        let hash_p = "$P$BgUdq1RzEBYd9Tm/uZC7mz/l5F.x4N1";
        let hash_h = format!("$H${}", &hash_p[3..]);
        assert!(PasswordHasher::verify("development", &hash_h).unwrap());
    }

    #[test]
    fn test_phpass_verify_too_short_hash() {
        // Hash shorter than 34 characters should return false
        assert!(!PasswordHasher::verify("password", "$P$Bshort").unwrap());
    }

    #[test]
    fn test_phpass_verify_invalid_iter_char() {
        // Use a character not in the itoa64 alphabet at position 3 (e.g., '!')
        let hash = "$P$!gUdq1RzEBYd9Tm/uZC7mz/l5F.x4N1";
        assert!(!PasswordHasher::verify("development", hash).unwrap());
    }

    #[test]
    fn test_phpass_needs_rehash() {
        // PHPass hashes should be flagged for rehash (upgrade to Argon2)
        let hash = "$P$BgUdq1RzEBYd9Tm/uZC7mz/l5F.x4N1";
        assert!(PasswordHasher::needs_rehash(hash));
    }

    #[test]
    fn test_phpass_verify_consistency() {
        // Verify the same password+hash pair succeeds repeatedly (no randomness in verify)
        let hash = "$P$BgUdq1RzEBYd9Tm/uZC7mz/l5F.x4N1";
        for _ in 0..3 {
            assert!(PasswordHasher::verify("development", hash).unwrap());
        }
    }

    #[test]
    fn test_phpass_encode64_known_output() {
        // Verify the encode64 function produces expected output for known input.
        // All zero bytes (16 of them) should encode to 22 dots (. is itoa64[0]).
        let zeros = [0u8; 16];
        let encoded = phpass_encode64(&zeros);
        assert_eq!(encoded.len(), 22);
        assert!(encoded.chars().all(|c| c == '.'));
    }

    #[test]
    fn test_phpass_encode64_length() {
        // 16 bytes should produce exactly 22 characters:
        // 5 full groups of 3 bytes (5*4=20 chars) + 1 remaining byte (2 chars) = 22
        let input = [0xFFu8; 16];
        let encoded = phpass_encode64(&input);
        assert_eq!(encoded.len(), 22);
    }

    #[test]
    fn test_phpass_encode64_single_byte() {
        // Single byte should produce 2 characters
        let input = [0x41u8]; // 'A' = 0x41 = 65
        let encoded = phpass_encode64(&input);
        assert_eq!(encoded.len(), 2);
    }

    #[test]
    fn test_phpass_encode64_two_bytes() {
        // Two bytes should produce 3 characters
        let input = [0x41u8, 0x42u8];
        let encoded = phpass_encode64(&input);
        assert_eq!(encoded.len(), 3);
    }

    #[test]
    fn test_phpass_encode64_three_bytes() {
        // Three bytes should produce 4 characters
        let input = [0x41u8, 0x42u8, 0x43u8];
        let encoded = phpass_encode64(&input);
        assert_eq!(encoded.len(), 4);
    }

    // --- Legacy MD5 verification tests ---

    #[test]
    fn test_legacy_md5_verify() {
        // MD5("password") = "5f4dcc3b5aa765d61d8327deb882cf99"
        let hash = "5f4dcc3b5aa765d61d8327deb882cf99";
        assert!(PasswordHasher::verify("password", hash).unwrap());
    }

    #[test]
    fn test_legacy_md5_verify_wrong_password() {
        let hash = "5f4dcc3b5aa765d61d8327deb882cf99";
        assert!(!PasswordHasher::verify("wrong", hash).unwrap());
    }

    #[test]
    fn test_legacy_md5_verify_uppercase_hash() {
        // WordPress might store MD5 in uppercase; our code lowercases the stored hash
        let hash = "5F4DCC3B5AA765D61D8327DEB882CF99";
        assert!(PasswordHasher::verify("password", hash).unwrap());
    }

    #[test]
    fn test_legacy_md5_empty_password() {
        // MD5("") = "d41d8cd98f00b204e9800998ecf8427e"
        let hash = "d41d8cd98f00b204e9800998ecf8427e";
        assert!(PasswordHasher::verify("", hash).unwrap());
    }

    // --- Unknown hash format tests ---

    #[test]
    fn test_unknown_hash_returns_false() {
        assert!(!PasswordHasher::verify("password", "some_random_string").unwrap());
    }

    #[test]
    fn test_empty_hash_returns_false() {
        assert!(!PasswordHasher::verify("password", "").unwrap());
    }

    // --- Password policy tests ---

    #[test]
    fn test_password_policy_too_short() {
        assert!(PasswordPolicy::validate("Ab1!").is_err());
    }

    #[test]
    fn test_password_policy_no_complexity() {
        assert!(PasswordPolicy::validate("abcdefgh").is_err());
    }

    #[test]
    fn test_password_policy_strong() {
        assert!(PasswordPolicy::validate("MyP@ssw0rd").is_ok());
    }

    #[test]
    fn test_password_policy_common() {
        assert!(PasswordPolicy::validate("Password").is_err()); // only 2 complexity
        assert!(PasswordPolicy::validate("Admin123").is_err()); // common
    }

    #[test]
    fn test_password_policy_three_classes() {
        // upper + lower + digit = 3 classes, should pass
        assert!(PasswordPolicy::validate("Abcdef12").is_ok());
        // lower + digit + special = 3 classes, should pass
        assert!(PasswordPolicy::validate("abcdef1!").is_ok());
    }

    // --- PasswordPolicy: length boundary ---

    #[test]
    fn test_password_policy_exactly_min_length() {
        // 8 chars with enough complexity should pass
        assert!(PasswordPolicy::validate("Abc1defg").is_ok());
    }

    #[test]
    fn test_password_policy_seven_chars_fails() {
        // Even with all complexity classes, 7 chars fails
        assert!(PasswordPolicy::validate("Abc1!fg").is_err());
    }

    #[test]
    fn test_password_policy_empty_fails() {
        assert!(PasswordPolicy::validate("").is_err());
    }

    // --- PasswordPolicy: common password list ---

    #[test]
    fn test_password_policy_rejects_password_common() {
        assert!(PasswordPolicy::validate("password").is_err());
    }

    #[test]
    fn test_password_policy_rejects_12345678() {
        assert!(PasswordPolicy::validate("12345678").is_err());
    }

    #[test]
    fn test_password_policy_rejects_qwerty12() {
        assert!(PasswordPolicy::validate("qwerty12").is_err());
    }

    #[test]
    fn test_password_policy_case_insensitive_common_check() {
        // Common password list is checked case-insensitively
        assert!(PasswordPolicy::validate("ADMIN123").is_err());
    }

    // --- PasswordPolicy: complexity ---

    #[test]
    fn test_password_policy_all_lowercase_fails() {
        assert!(PasswordPolicy::validate("abcdefghij").is_err());
    }

    #[test]
    fn test_password_policy_all_digits_fails() {
        assert!(PasswordPolicy::validate("123456789").is_err());
    }

    #[test]
    fn test_password_policy_all_uppercase_fails() {
        assert!(PasswordPolicy::validate("ABCDEFGHIJ").is_err());
    }

    #[test]
    fn test_password_policy_four_classes_passes() {
        // All four: upper + lower + digit + special
        assert!(PasswordPolicy::validate("Abc1!xyz").is_ok());
    }

    #[test]
    fn test_password_policy_unicode_counts_as_special() {
        // Unicode char is non-alphanumeric, counts as special
        assert!(PasswordPolicy::validate("Abc1😀yz").is_ok());
    }

    // --- PasswordHasher: bcrypt extended ---

    #[test]
    fn test_bcrypt_hash_starts_with_dollar() {
        let pw = "bcrypt_test_pw_99_extra";
        let hash = PasswordHasher::hash_bcrypt(pw).unwrap();
        assert!(hash.starts_with("$2"));
    }

    #[test]
    fn test_bcrypt_correct_password_accepted() {
        let hash = PasswordHasher::hash_bcrypt("correct-horse-battery").unwrap();
        assert!(PasswordHasher::verify("correct-horse-battery", &hash).unwrap());
    }

    #[test]
    fn test_bcrypt_wrong_password_rejected_ext() {
        let hash = PasswordHasher::hash_bcrypt("correct-horse-ext").unwrap();
        assert!(!PasswordHasher::verify("wrong-horse-ext", &hash).unwrap());
    }

    // --- PasswordHasher: needs_rehash extended ---

    #[test]
    fn test_needs_rehash_argon2_returns_false() {
        let hash = PasswordHasher::hash_argon2("test_pw_123_ext").unwrap();
        assert!(!PasswordHasher::needs_rehash(&hash));
    }

    #[test]
    fn test_needs_rehash_md5_bare_returns_true() {
        // A bare md5 hex hash should need rehash
        assert!(PasswordHasher::needs_rehash(
            "5f4dcc3b5aa765d61d8327deb882cf99"
        ));
    }

    #[test]
    fn test_needs_rehash_bcrypt_returns_true() {
        // Only argon2 hashes are considered "up to date"; bcrypt hashes need rehash
        let hash = PasswordHasher::hash_bcrypt("some_pw_abc_ext").unwrap();
        assert!(PasswordHasher::needs_rehash(&hash));
    }

    // --- PasswordHasher: argon2 wrong password rejected ---

    #[test]
    fn test_argon2_wrong_password_rejected_ext() {
        let hash = PasswordHasher::hash_argon2("correct_pass_xyz_ext").unwrap();
        assert!(!PasswordHasher::verify("wrong_pass_xyz_ext", &hash).unwrap());
    }

    // --- PasswordHasher: verify with empty hash ---

    #[test]
    fn test_verify_empty_hash_graceful() {
        // Empty hash should not match any password
        assert!(!PasswordHasher::verify("password123", "").unwrap_or(false));
    }

    // --- BUG-NEW-2: WordPress 6.8+ $wp$ argon2id hash support ---

    #[test]
    fn test_verify_wp_argon2id_hash() {
        // Simulate a $wp$ hash: generate argon2id, prepend "$wp$"
        let inner = PasswordHasher::hash_argon2("mysecretpassword").unwrap();
        let wp_hash = format!("$wp${inner}");
        assert!(
            PasswordHasher::verify("mysecretpassword", &wp_hash).unwrap(),
            "$wp$ argon2id hash should verify correctly"
        );
        assert!(
            !PasswordHasher::verify("wrongpassword", &wp_hash).unwrap(),
            "$wp$ argon2id hash should reject wrong password"
        );
    }

    #[test]
    fn test_needs_rehash_wp_argon2id_returns_false() {
        // $wp$ hashes are modern (WordPress 6.8+ argon2id) — no rehash needed
        let inner = PasswordHasher::hash_argon2("test").unwrap();
        let wp_hash = format!("$wp${inner}");
        assert!(
            !PasswordHasher::needs_rehash(&wp_hash),
            "$wp$ hash should not need rehash"
        );
    }

    #[test]
    fn test_verify_wp_prefix_wrong_password_rejected() {
        let inner = PasswordHasher::hash_argon2("correct").unwrap();
        let wp_hash = format!("$wp${inner}");
        assert!(!PasswordHasher::verify("incorrect", &wp_hash).unwrap());
    }
}
