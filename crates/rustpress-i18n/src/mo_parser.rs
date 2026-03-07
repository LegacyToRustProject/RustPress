//! GNU gettext `.mo` file parser.
//!
//! Parses binary `.mo` files into an in-memory lookup table of translations.
//! Supports both little-endian and big-endian .mo files, plural forms, and
//! metadata extraction from the empty-string entry.

use std::collections::HashMap;

/// Magic number for little-endian .mo files.
pub const MO_MAGIC_LE: u32 = 0x950412de;
/// Magic number for big-endian .mo files.
pub const MO_MAGIC_BE: u32 = 0xde120495;

/// Separator used between singular and plural forms in .mo files.
const PLURAL_SEPARATOR: u8 = 0x00;

/// Errors that can occur when parsing a .mo file.
#[derive(Debug, thiserror::Error)]
pub enum MoError {
    #[error("invalid magic number: expected 0x950412de or 0xde120495, got 0x{0:08x}")]
    InvalidMagic(u32),

    #[error("invalid .mo file format: {0}")]
    InvalidFormat(String),

    #[error("UTF-8 decoding error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("I/O error: {0}")]
    IoError(String),
}

/// Represents a parsed .mo file with its translations and metadata.
#[derive(Debug, Clone)]
pub struct MoFile {
    /// Singular translations: original -> translated.
    pub translations: HashMap<String, String>,
    /// Plural translations: original -> vec of plural forms.
    pub plural_translations: HashMap<String, Vec<String>>,
    /// Metadata extracted from the empty-string entry (e.g., Content-Type, Plural-Forms).
    pub metadata: HashMap<String, String>,
}

/// Endianness detected from the magic number.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Endian {
    Little,
    Big,
}

impl Endian {
    fn read_u32(self, data: &[u8], offset: usize) -> Result<u32, MoError> {
        if offset + 4 > data.len() {
            return Err(MoError::InvalidFormat(format!(
                "unexpected end of data at offset {offset}, need 4 bytes but only {} available",
                data.len().saturating_sub(offset)
            )));
        }
        let bytes: [u8; 4] = data[offset..offset + 4].try_into().unwrap();
        Ok(match self {
            Endian::Little => u32::from_le_bytes(bytes),
            Endian::Big => u32::from_be_bytes(bytes),
        })
    }
}

/// Parse a GNU gettext .mo file from raw bytes.
///
/// The .mo binary format is documented at:
/// <https://www.gnu.org/software/gettext/manual/html_node/MO-Files.html>
///
/// # Layout
/// ```text
///  offset  type          description
///  0       u32           magic number
///  4       u32           file format revision
///  8       u32           number of strings
///  12      u32           offset of table with original strings
///  16      u32           offset of table with translated strings
///  20      u32           size of hashing table
///  24      u32           offset of hashing table
/// ```
///
/// Each string table entry is a pair (length: u32, offset: u32).
pub fn parse_mo(data: &[u8]) -> Result<MoFile, MoError> {
    if data.len() < 28 {
        return Err(MoError::InvalidFormat(
            "file too small to contain a valid .mo header (need at least 28 bytes)".to_string(),
        ));
    }

    // Detect endianness from magic number
    let magic_le = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let endian = if magic_le == MO_MAGIC_LE {
        Endian::Little
    } else if magic_le == MO_MAGIC_BE {
        Endian::Big
    } else {
        return Err(MoError::InvalidMagic(magic_le));
    };

    let _revision = endian.read_u32(data, 4)?;
    let num_strings = endian.read_u32(data, 8)? as usize;
    let offset_originals = endian.read_u32(data, 12)? as usize;
    let offset_translations = endian.read_u32(data, 16)? as usize;

    let mut translations = HashMap::new();
    let mut plural_translations = HashMap::new();
    let mut metadata = HashMap::new();

    for i in 0..num_strings {
        // Each entry in the string table is 8 bytes: (length: u32, offset: u32)
        let orig_len = endian.read_u32(data, offset_originals + i * 8)? as usize;
        let orig_off = endian.read_u32(data, offset_originals + i * 8 + 4)? as usize;

        let trans_len = endian.read_u32(data, offset_translations + i * 8)? as usize;
        let trans_off = endian.read_u32(data, offset_translations + i * 8 + 4)? as usize;

        // Bounds check
        if orig_off + orig_len > data.len() {
            return Err(MoError::InvalidFormat(format!(
                "original string {i} extends beyond file (offset {orig_off}, len {orig_len})"
            )));
        }
        if trans_off + trans_len > data.len() {
            return Err(MoError::InvalidFormat(format!(
                "translated string {i} extends beyond file (offset {trans_off}, len {trans_len})"
            )));
        }

        let orig_bytes = &data[orig_off..orig_off + orig_len];
        let trans_bytes = &data[trans_off..trans_off + trans_len];

        // The empty original string contains metadata
        if orig_len == 0 {
            let meta_str = String::from_utf8(trans_bytes.to_vec())?;
            metadata = parse_metadata(&meta_str);
            continue;
        }

        // Check if original contains plural separator (NUL byte between singular and plural)
        if orig_bytes.contains(&PLURAL_SEPARATOR) {
            // Plural form: original is "singular\0plural", translation is "form0\0form1\0..."
            let orig_parts: Vec<&[u8]> = orig_bytes.splitn(2, |&b| b == PLURAL_SEPARATOR).collect();
            let singular = String::from_utf8(orig_parts[0].to_vec())?;

            let trans_parts: Vec<String> = trans_bytes
                .split(|&b| b == PLURAL_SEPARATOR)
                .map(|part| String::from_utf8(part.to_vec()))
                .collect::<Result<Vec<_>, _>>()?;

            plural_translations.insert(singular, trans_parts);
        } else {
            let orig = String::from_utf8(orig_bytes.to_vec())?;
            let trans = String::from_utf8(trans_bytes.to_vec())?;
            translations.insert(orig, trans);
        }
    }

    Ok(MoFile {
        translations,
        plural_translations,
        metadata,
    })
}

/// Parse metadata from the empty-string translation entry.
///
/// The metadata is formatted as HTTP-style headers:
/// ```text
/// Content-Type: text/plain; charset=UTF-8\n
/// Plural-Forms: nplurals=2; plural=(n != 1);\n
/// ```
fn parse_metadata(meta: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in meta.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal valid .mo file in memory (little-endian).
    fn build_mo_file(entries: &[(&[u8], &[u8])]) -> Vec<u8> {
        let num_strings = entries.len() as u32;

        // Header is 28 bytes
        let header_size = 28u32;
        // String tables: each entry is 8 bytes (len + offset)
        let table_size = num_strings * 8;
        let offset_originals = header_size;
        let offset_translations = header_size + table_size;

        // String data starts after both tables
        let string_data_start = (header_size + table_size * 2) as usize;

        // Collect string data and table entries
        let mut orig_table: Vec<u8> = Vec::new();
        let mut trans_table: Vec<u8> = Vec::new();
        let mut string_data: Vec<u8> = Vec::new();

        for (orig, trans) in entries {
            let orig_offset = string_data_start + string_data.len();
            orig_table.extend_from_slice(&(orig.len() as u32).to_le_bytes());
            orig_table.extend_from_slice(&(orig_offset as u32).to_le_bytes());
            string_data.extend_from_slice(orig);
            string_data.push(0); // NUL terminator (not counted in length)

            let trans_offset = string_data_start + string_data.len();
            trans_table.extend_from_slice(&(trans.len() as u32).to_le_bytes());
            trans_table.extend_from_slice(&(trans_offset as u32).to_le_bytes());
            string_data.extend_from_slice(trans);
            string_data.push(0); // NUL terminator
        }

        let mut buf = Vec::new();
        // Magic number (LE)
        buf.extend_from_slice(&MO_MAGIC_LE.to_le_bytes());
        // Revision
        buf.extend_from_slice(&0u32.to_le_bytes());
        // Number of strings
        buf.extend_from_slice(&num_strings.to_le_bytes());
        // Offset of originals table
        buf.extend_from_slice(&offset_originals.to_le_bytes());
        // Offset of translations table
        buf.extend_from_slice(&offset_translations.to_le_bytes());
        // Hash table size (unused, 0)
        buf.extend_from_slice(&0u32.to_le_bytes());
        // Hash table offset (unused, 0)
        buf.extend_from_slice(&0u32.to_le_bytes());
        // Tables
        buf.extend_from_slice(&orig_table);
        buf.extend_from_slice(&trans_table);
        // String data
        buf.extend_from_slice(&string_data);

        buf
    }

    #[test]
    fn test_parse_simple_translation() {
        let entries: Vec<(&[u8], &[u8])> = vec![
            (b"Hello", b"Hola"),
            (b"Goodbye", b"Adios"),
        ];
        let data = build_mo_file(&entries);
        let mo = parse_mo(&data).unwrap();

        assert_eq!(mo.translations.get("Hello").unwrap(), "Hola");
        assert_eq!(mo.translations.get("Goodbye").unwrap(), "Adios");
    }

    #[test]
    fn test_parse_metadata() {
        let meta = b"Content-Type: text/plain; charset=UTF-8\nPlural-Forms: nplurals=2; plural=(n != 1);\n";
        let entries: Vec<(&[u8], &[u8])> = vec![
            (b"", meta.as_slice()),
            (b"Yes", b"Si"),
        ];
        let data = build_mo_file(&entries);
        let mo = parse_mo(&data).unwrap();

        assert_eq!(
            mo.metadata.get("Content-Type").unwrap(),
            "text/plain; charset=UTF-8"
        );
        assert_eq!(
            mo.metadata.get("Plural-Forms").unwrap(),
            "nplurals=2; plural=(n != 1);"
        );
        assert_eq!(mo.translations.get("Yes").unwrap(), "Si");
    }

    #[test]
    fn test_parse_plural_forms() {
        // Plural entry: "singular\0plural" -> "form0\0form1"
        let orig = b"%d item\x00%d items";
        let trans = b"%d elemento\x00%d elementos";
        let entries: Vec<(&[u8], &[u8])> = vec![(orig.as_slice(), trans.as_slice())];
        let data = build_mo_file(&entries);
        let mo = parse_mo(&data).unwrap();

        let forms = mo.plural_translations.get("%d item").unwrap();
        assert_eq!(forms.len(), 2);
        assert_eq!(forms[0], "%d elemento");
        assert_eq!(forms[1], "%d elementos");
    }

    #[test]
    fn test_invalid_magic() {
        let data = vec![0x00, 0x00, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let result = parse_mo(&data);
        assert!(matches!(result, Err(MoError::InvalidMagic(_))));
    }

    #[test]
    fn test_file_too_small() {
        let data = vec![0xde, 0x12, 0x04, 0x95];
        let result = parse_mo(&data);
        assert!(matches!(result, Err(MoError::InvalidFormat(_))));
    }

    #[test]
    fn test_big_endian_magic() {
        // Build a file with BE magic. We build LE then swap the magic.
        let entries: Vec<(&[u8], &[u8])> = vec![(b"Hi", b"Hej")];
        let _data = build_mo_file(&entries);
        // Build a proper big-endian .mo file manually.
        let mut buf = Vec::new();
        buf.extend_from_slice(&MO_MAGIC_BE.to_le_bytes()); // When read as LE u32, gives 0xde120495
        buf.extend_from_slice(&0u32.to_be_bytes()); // revision
        buf.extend_from_slice(&1u32.to_be_bytes()); // num_strings = 1
        let offset_originals: u32 = 28;
        let offset_translations: u32 = 36;
        buf.extend_from_slice(&offset_originals.to_be_bytes());
        buf.extend_from_slice(&offset_translations.to_be_bytes());
        buf.extend_from_slice(&0u32.to_be_bytes()); // hash size
        buf.extend_from_slice(&0u32.to_be_bytes()); // hash offset
        // Original table entry: len=2, offset=44
        let string_start: u32 = 44;
        buf.extend_from_slice(&2u32.to_be_bytes()); // orig len
        buf.extend_from_slice(&string_start.to_be_bytes()); // orig offset
        // Translation table entry: len=3, offset=47
        let trans_start: u32 = 47;
        buf.extend_from_slice(&3u32.to_be_bytes()); // trans len
        buf.extend_from_slice(&trans_start.to_be_bytes()); // trans offset
        // String data
        buf.extend_from_slice(b"Hi\x00");   // original + NUL
        buf.extend_from_slice(b"Hej\x00");  // translation + NUL

        let mo = parse_mo(&buf).unwrap();
        assert_eq!(mo.translations.get("Hi").unwrap(), "Hej");
    }
}
