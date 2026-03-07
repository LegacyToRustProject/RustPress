//! PHP serialized data parser.
//!
//! WordPress stores many values as PHP serialized strings in wp_postmeta
//! and wp_options. This module provides a parser that can read these values
//! into Rust types.
//!
//! ## Supported PHP Types
//! - `s:N:"string";` — String
//! - `i:N;` — Integer
//! - `d:N;` — Double/Float
//! - `b:0;` / `b:1;` — Boolean
//! - `N;` — Null
//! - `a:N:{...}` — Array (associative or indexed)
//!
//! ## Examples
//! ```
//! use rustpress_core::php_serialize::{PhpValue, php_unserialize};
//!
//! // Parse a serialized string
//! let val = php_unserialize(r#"s:5:"hello";"#).unwrap();
//! assert_eq!(val.as_str(), Some("hello"));
//!
//! // Parse a serialized array
//! let val = php_unserialize(r#"a:2:{s:4:"name";s:5:"Alice";s:3:"age";i:30;}"#).unwrap();
//! assert_eq!(val.get("name").and_then(|v| v.as_str()), Some("Alice"));
//! ```

use std::collections::HashMap;

/// A deserialized PHP value.
#[derive(Debug, Clone, PartialEq)]
pub enum PhpValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<(PhpValue, PhpValue)>),
}

impl PhpValue {
    /// Get as string reference.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            PhpValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Get as i64.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            PhpValue::Int(n) => Some(*n),
            _ => None,
        }
    }

    /// Get as f64.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            PhpValue::Float(n) => Some(*n),
            PhpValue::Int(n) => Some(*n as f64),
            _ => None,
        }
    }

    /// Get as bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            PhpValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// For arrays, look up a value by string key.
    pub fn get(&self, key: &str) -> Option<&PhpValue> {
        match self {
            PhpValue::Array(pairs) => pairs.iter().find_map(|(k, v)| {
                if k.as_str() == Some(key) {
                    Some(v)
                } else {
                    None
                }
            }),
            _ => None,
        }
    }

    /// For arrays, look up a value by integer index.
    pub fn get_index(&self, index: i64) -> Option<&PhpValue> {
        match self {
            PhpValue::Array(pairs) => pairs.iter().find_map(|(k, v)| {
                if k.as_int() == Some(index) {
                    Some(v)
                } else {
                    None
                }
            }),
            _ => None,
        }
    }

    /// For arrays, return as a HashMap of string keys to values.
    pub fn as_map(&self) -> Option<HashMap<String, &PhpValue>> {
        match self {
            PhpValue::Array(pairs) => {
                let mut map = HashMap::new();
                for (k, v) in pairs {
                    if let Some(key) = k.as_str() {
                        map.insert(key.to_string(), v);
                    }
                }
                Some(map)
            }
            _ => None,
        }
    }

    /// For indexed arrays, collect values as Vec<u64>.
    pub fn as_id_list(&self) -> Vec<u64> {
        match self {
            PhpValue::Array(pairs) => pairs
                .iter()
                .filter_map(|(_, v)| match v {
                    PhpValue::Int(n) if *n >= 0 => Some(*n as u64),
                    PhpValue::String(s) => s.parse::<u64>().ok(),
                    _ => None,
                })
                .collect(),
            _ => vec![],
        }
    }

    /// For indexed arrays, collect values as Vec<String>.
    pub fn as_string_list(&self) -> Vec<String> {
        match self {
            PhpValue::Array(pairs) => pairs
                .iter()
                .filter_map(|(_, v)| v.as_str().map(|s| s.to_string()))
                .collect(),
            _ => vec![],
        }
    }
}

/// Parse a PHP serialized string into a PhpValue.
pub fn php_unserialize(input: &str) -> Result<PhpValue, PhpSerializeError> {
    let bytes = input.as_bytes();
    let (val, _) = parse_value(bytes, 0)?;
    Ok(val)
}

/// Serialize a PhpValue back to PHP serialized format.
pub fn php_serialize(value: &PhpValue) -> String {
    match value {
        PhpValue::Null => "N;".to_string(),
        PhpValue::Bool(b) => format!("b:{};", if *b { 1 } else { 0 }),
        PhpValue::Int(n) => format!("i:{};", n),
        PhpValue::Float(n) => format!("d:{};", n),
        PhpValue::String(s) => format!("s:{}:\"{}\";", s.len(), s),
        PhpValue::Array(pairs) => {
            let mut result = format!("a:{}:{{", pairs.len());
            for (k, v) in pairs {
                result.push_str(&php_serialize(k));
                result.push_str(&php_serialize(v));
            }
            result.push('}');
            result
        }
    }
}

#[derive(Debug, Clone)]
pub struct PhpSerializeError {
    pub message: String,
    pub position: usize,
}

impl std::fmt::Display for PhpSerializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PHP unserialize error at {}: {}", self.position, self.message)
    }
}

impl std::error::Error for PhpSerializeError {}

fn parse_value(bytes: &[u8], pos: usize) -> Result<(PhpValue, usize), PhpSerializeError> {
    if pos >= bytes.len() {
        return Err(PhpSerializeError {
            message: "unexpected end of input".into(),
            position: pos,
        });
    }

    match bytes[pos] {
        b'N' => parse_null(bytes, pos),
        b'b' => parse_bool(bytes, pos),
        b'i' => parse_int(bytes, pos),
        b'd' => parse_double(bytes, pos),
        b's' => parse_string(bytes, pos),
        b'a' => parse_array(bytes, pos),
        _ => Err(PhpSerializeError {
            message: format!("unexpected type marker '{}'", bytes[pos] as char),
            position: pos,
        }),
    }
}

fn parse_null(bytes: &[u8], pos: usize) -> Result<(PhpValue, usize), PhpSerializeError> {
    // N;
    expect_byte(bytes, pos, b'N')?;
    expect_byte(bytes, pos + 1, b';')?;
    Ok((PhpValue::Null, pos + 2))
}

fn parse_bool(bytes: &[u8], pos: usize) -> Result<(PhpValue, usize), PhpSerializeError> {
    // b:0; or b:1;
    expect_byte(bytes, pos, b'b')?;
    expect_byte(bytes, pos + 1, b':')?;
    let val = match bytes.get(pos + 2) {
        Some(b'1') => true,
        Some(b'0') => false,
        _ => {
            return Err(PhpSerializeError {
                message: "expected 0 or 1 for bool".into(),
                position: pos + 2,
            })
        }
    };
    expect_byte(bytes, pos + 3, b';')?;
    Ok((PhpValue::Bool(val), pos + 4))
}

fn parse_int(bytes: &[u8], pos: usize) -> Result<(PhpValue, usize), PhpSerializeError> {
    // i:NUMBER;
    expect_byte(bytes, pos, b'i')?;
    expect_byte(bytes, pos + 1, b':')?;
    let (num_str, end) = read_until(bytes, pos + 2, b';')?;
    let n: i64 = num_str.parse().map_err(|_| PhpSerializeError {
        message: format!("invalid integer: {}", num_str),
        position: pos + 2,
    })?;
    Ok((PhpValue::Int(n), end + 1))
}

fn parse_double(bytes: &[u8], pos: usize) -> Result<(PhpValue, usize), PhpSerializeError> {
    // d:NUMBER;
    expect_byte(bytes, pos, b'd')?;
    expect_byte(bytes, pos + 1, b':')?;
    let (num_str, end) = read_until(bytes, pos + 2, b';')?;
    let n: f64 = num_str.parse().map_err(|_| PhpSerializeError {
        message: format!("invalid double: {}", num_str),
        position: pos + 2,
    })?;
    Ok((PhpValue::Float(n), end + 1))
}

fn parse_string(bytes: &[u8], pos: usize) -> Result<(PhpValue, usize), PhpSerializeError> {
    // s:LENGTH:"CONTENT";
    expect_byte(bytes, pos, b's')?;
    expect_byte(bytes, pos + 1, b':')?;
    let (len_str, after_len) = read_until(bytes, pos + 2, b':')?;
    let len: usize = len_str.parse().map_err(|_| PhpSerializeError {
        message: format!("invalid string length: {}", len_str),
        position: pos + 2,
    })?;

    // Expect :"
    let quote_pos = after_len + 1;
    expect_byte(bytes, quote_pos, b'"')?;

    let str_start = quote_pos + 1;
    let str_end = str_start + len;

    if str_end > bytes.len() {
        return Err(PhpSerializeError {
            message: "string extends past end of input".into(),
            position: str_start,
        });
    }

    let content = std::str::from_utf8(&bytes[str_start..str_end]).map_err(|_| {
        PhpSerializeError {
            message: "invalid UTF-8 in string".into(),
            position: str_start,
        }
    })?;

    // Expect ";
    expect_byte(bytes, str_end, b'"')?;
    expect_byte(bytes, str_end + 1, b';')?;

    Ok((PhpValue::String(content.to_string()), str_end + 2))
}

fn parse_array(bytes: &[u8], pos: usize) -> Result<(PhpValue, usize), PhpSerializeError> {
    // a:COUNT:{KEY;VALUE;...}
    expect_byte(bytes, pos, b'a')?;
    expect_byte(bytes, pos + 1, b':')?;
    let (count_str, after_count) = read_until(bytes, pos + 2, b':')?;
    let count: usize = count_str.parse().map_err(|_| PhpSerializeError {
        message: format!("invalid array count: {}", count_str),
        position: pos + 2,
    })?;

    expect_byte(bytes, after_count + 1, b'{')?;
    let mut cursor = after_count + 2;
    let mut pairs = Vec::with_capacity(count);

    for _ in 0..count {
        let (key, after_key) = parse_value(bytes, cursor)?;
        let (val, after_val) = parse_value(bytes, after_key)?;
        pairs.push((key, val));
        cursor = after_val;
    }

    expect_byte(bytes, cursor, b'}')?;
    Ok((PhpValue::Array(pairs), cursor + 1))
}

fn expect_byte(
    bytes: &[u8],
    pos: usize,
    expected: u8,
) -> Result<(), PhpSerializeError> {
    match bytes.get(pos) {
        Some(&b) if b == expected => Ok(()),
        Some(&b) => Err(PhpSerializeError {
            message: format!("expected '{}', got '{}'", expected as char, b as char),
            position: pos,
        }),
        None => Err(PhpSerializeError {
            message: format!("expected '{}', got end of input", expected as char),
            position: pos,
        }),
    }
}

fn read_until(
    bytes: &[u8],
    start: usize,
    delimiter: u8,
) -> Result<(String, usize), PhpSerializeError> {
    for i in start..bytes.len() {
        if bytes[i] == delimiter {
            let s = std::str::from_utf8(&bytes[start..i]).map_err(|_| PhpSerializeError {
                message: "invalid UTF-8".into(),
                position: start,
            })?;
            return Ok((s.to_string(), i));
        }
    }
    Err(PhpSerializeError {
        message: format!("expected '{}'", delimiter as char),
        position: start,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null() {
        let val = php_unserialize("N;").unwrap();
        assert_eq!(val, PhpValue::Null);
    }

    #[test]
    fn test_bool() {
        assert_eq!(php_unserialize("b:1;").unwrap(), PhpValue::Bool(true));
        assert_eq!(php_unserialize("b:0;").unwrap(), PhpValue::Bool(false));
    }

    #[test]
    fn test_int() {
        assert_eq!(php_unserialize("i:42;").unwrap(), PhpValue::Int(42));
        assert_eq!(php_unserialize("i:-5;").unwrap(), PhpValue::Int(-5));
        assert_eq!(php_unserialize("i:0;").unwrap(), PhpValue::Int(0));
    }

    #[test]
    fn test_float() {
        assert_eq!(php_unserialize("d:3.14;").unwrap(), PhpValue::Float(3.14));
    }

    #[test]
    fn test_string() {
        let val = php_unserialize(r#"s:5:"hello";"#).unwrap();
        assert_eq!(val.as_str(), Some("hello"));
    }

    #[test]
    fn test_empty_string() {
        let val = php_unserialize(r#"s:0:"";"#).unwrap();
        assert_eq!(val.as_str(), Some(""));
    }

    #[test]
    fn test_simple_array() {
        // a:2:{i:0;s:3:"foo";i:1;s:3:"bar";}
        let val = php_unserialize(r#"a:2:{i:0;s:3:"foo";i:1;s:3:"bar";}"#).unwrap();
        let list = val.as_string_list();
        assert_eq!(list, vec!["foo", "bar"]);
    }

    #[test]
    fn test_associative_array() {
        let input = r#"a:2:{s:4:"name";s:5:"Alice";s:3:"age";i:30;}"#;
        let val = php_unserialize(input).unwrap();

        assert_eq!(val.get("name").and_then(|v| v.as_str()), Some("Alice"));
        assert_eq!(val.get("age").and_then(|v| v.as_int()), Some(30));
    }

    #[test]
    fn test_wp_capabilities() {
        // WordPress stores capabilities like: a:1:{s:13:"administrator";b:1;}
        let input = r#"a:1:{s:13:"administrator";b:1;}"#;
        let val = php_unserialize(input).unwrap();

        assert_eq!(
            val.get("administrator").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn test_sticky_posts() {
        // a:2:{i:0;i:6;i:1;i:42;}
        let val = php_unserialize("a:2:{i:0;i:6;i:1;i:42;}").unwrap();
        let ids = val.as_id_list();
        assert_eq!(ids, vec![6, 42]);
    }

    #[test]
    fn test_nested_array() {
        // a:1:{s:4:"mail";a:2:{s:2:"to";s:17:"admin@example.com";s:7:"subject";s:5:"Hello";}}
        let input = r#"a:1:{s:4:"mail";a:2:{s:2:"to";s:17:"admin@example.com";s:7:"subject";s:5:"Hello";}}"#;
        let val = php_unserialize(input).unwrap();

        let mail = val.get("mail").unwrap();
        assert_eq!(
            mail.get("to").and_then(|v| v.as_str()),
            Some("admin@example.com")
        );
        assert_eq!(
            mail.get("subject").and_then(|v| v.as_str()),
            Some("Hello")
        );
    }

    #[test]
    fn test_serialize_roundtrip() {
        let original = PhpValue::Array(vec![
            (
                PhpValue::String("name".into()),
                PhpValue::String("Alice".into()),
            ),
            (PhpValue::String("age".into()), PhpValue::Int(30)),
            (PhpValue::String("active".into()), PhpValue::Bool(true)),
        ]);

        let serialized = php_serialize(&original);
        let deserialized = php_unserialize(&serialized).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_serialize_null() {
        assert_eq!(php_serialize(&PhpValue::Null), "N;");
    }

    #[test]
    fn test_serialize_string() {
        assert_eq!(
            php_serialize(&PhpValue::String("hello".into())),
            r#"s:5:"hello";"#
        );
    }

    #[test]
    fn test_acf_repeater_data() {
        // ACF stores repeater count as: i:3; (number of rows)
        // Individual fields as: s:N:"value";
        let input = r#"a:3:{i:0;s:5:"row_1";i:1;s:5:"row_2";i:2;s:5:"row_3";}"#;
        let val = php_unserialize(input).unwrap();
        let list = val.as_string_list();
        assert_eq!(list, vec!["row_1", "row_2", "row_3"]);
    }

    #[test]
    fn test_yoast_titles_options() {
        // Build the test data via serialization to ensure correct lengths
        let data = PhpValue::Array(vec![
            (
                PhpValue::String("title-post".into()),
                PhpValue::String("%%title%% %%sep%% %%sitename%%".into()),
            ),
            (
                PhpValue::String("title-page".into()),
                PhpValue::String("%%title%% %%sep%% %%sitename%%".into()),
            ),
        ]);
        let serialized = php_serialize(&data);
        let val = php_unserialize(&serialized).unwrap();

        assert_eq!(
            val.get("title-post").and_then(|v| v.as_str()),
            Some("%%title%% %%sep%% %%sitename%%")
        );
    }

    #[test]
    fn test_as_map() {
        let input = r#"a:2:{s:1:"a";i:1;s:1:"b";i:2;}"#;
        let val = php_unserialize(input).unwrap();
        let map = val.as_map().unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("a").and_then(|v| v.as_int()), Some(1));
        assert_eq!(map.get("b").and_then(|v| v.as_int()), Some(2));
    }

    #[test]
    fn test_invalid_input() {
        assert!(php_unserialize("x:invalid;").is_err());
        assert!(php_unserialize("").is_err());
        assert!(php_unserialize("s:99:\"short\";").is_err());
    }
}
