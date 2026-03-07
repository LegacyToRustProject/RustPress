//! WordPress-compatible image sizes and responsive image (srcset) support.
//!
//! WordPress generates multiple sizes for each uploaded image and produces
//! `srcset` / `sizes` attributes so browsers can load appropriately-sized
//! versions.  This module provides the default size definitions plus helpers
//! for computing proportional dimensions and building the HTML attributes.

use serde::{Deserialize, Serialize};

/// Describes a single registered image size (e.g. "thumbnail", "medium").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSize {
    /// Slug used to identify this size (e.g. "thumbnail", "medium_large").
    pub name: String,
    /// Maximum width in pixels. `0` means unconstrained.
    pub width: u32,
    /// Maximum height in pixels. `0` means unconstrained.
    pub height: u32,
    /// If `true` the image is hard-cropped to the exact dimensions;
    /// otherwise it is proportionally resized to fit within the box.
    pub crop: bool,
}

/// Return the default WordPress image sizes.
///
/// These match the built-in sizes that WordPress registers out of the box
/// (including the `medium_large` size added in WP 4.4 and the 1536/2048
/// sizes added in WP 5.3).
pub fn default_image_sizes() -> Vec<ImageSize> {
    vec![
        ImageSize {
            name: "thumbnail".to_string(),
            width: 150,
            height: 150,
            crop: true,
        },
        ImageSize {
            name: "medium".to_string(),
            width: 300,
            height: 300,
            crop: false,
        },
        ImageSize {
            name: "medium_large".to_string(),
            width: 768,
            height: 0,
            crop: false,
        },
        ImageSize {
            name: "large".to_string(),
            width: 1024,
            height: 1024,
            crop: false,
        },
        ImageSize {
            name: "1536x1536".to_string(),
            width: 1536,
            height: 1536,
            crop: false,
        },
        ImageSize {
            name: "2048x2048".to_string(),
            width: 2048,
            height: 2048,
            crop: false,
        },
    ]
}

/// Build an HTML-ready `srcset` attribute value from a list of
/// `(url, pixel_width)` pairs.
///
/// # Example
///
/// ```
/// use rustpress_core::media_sizes::generate_srcset;
///
/// let sizes = vec![
///     ("https://example.com/img-300x200.jpg".to_string(), 300),
///     ("https://example.com/img-768x512.jpg".to_string(), 768),
/// ];
/// let srcset = generate_srcset(&sizes);
/// assert_eq!(
///     srcset,
///     "https://example.com/img-300x200.jpg 300w, https://example.com/img-768x512.jpg 768w"
/// );
/// ```
pub fn generate_srcset(sizes: &[(String, u32)]) -> String {
    sizes
        .iter()
        .map(|(url, w)| format!("{url} {w}w"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Build an HTML-ready `sizes` attribute value.
///
/// WordPress uses a simple heuristic: the image can be up to 100 vw on
/// screens narrower than `max_width`, and exactly `max_width` px otherwise.
pub fn generate_sizes_attr(max_width: u32) -> String {
    format!("(max-width: {max_width}px) 100vw, {max_width}px")
}

/// Calculate proportional (aspect-ratio-preserving) output dimensions for a
/// resize operation when `crop` is `false`.
///
/// Both `max_width` and `max_height` may be `0` to indicate "unconstrained
/// on that axis".  If the original image already fits within the box the
/// original dimensions are returned unchanged.
///
/// # Examples
///
/// ```
/// use rustpress_core::media_sizes::calculate_dimensions;
///
/// // Landscape 2000x1000 into 300x300 box -> 300x150
/// assert_eq!(calculate_dimensions(2000, 1000, 300, 300), (300, 150));
///
/// // Portrait 1000x2000 into 300x300 box -> 150x300
/// assert_eq!(calculate_dimensions(1000, 2000, 300, 300), (150, 300));
///
/// // Height-unconstrained (medium_large style): 2000x1000 into 768x0 -> 768x384
/// assert_eq!(calculate_dimensions(2000, 1000, 768, 0), (768, 384));
/// ```
pub fn calculate_dimensions(
    orig_width: u32,
    orig_height: u32,
    max_width: u32,
    max_height: u32,
) -> (u32, u32) {
    if orig_width == 0 || orig_height == 0 {
        return (0, 0);
    }

    let mut new_width = orig_width;
    let mut new_height = orig_height;

    // Constrain width first.
    if max_width > 0 && new_width > max_width {
        new_height = (new_height as f64 * max_width as f64 / new_width as f64).round() as u32;
        new_width = max_width;
    }

    // Then constrain height (may further shrink width).
    if max_height > 0 && new_height > max_height {
        new_width = (new_width as f64 * max_height as f64 / new_height as f64).round() as u32;
        new_height = max_height;
    }

    (new_width, new_height)
}

/// Calculate output dimensions for a crop operation.
///
/// When `crop` is `true` the image is hard-cropped to exactly `width x height`.
/// If the original is smaller than the target on either axis the crop is
/// skipped and `None` is returned.
pub fn calculate_crop_dimensions(
    orig_width: u32,
    orig_height: u32,
    crop_width: u32,
    crop_height: u32,
) -> Option<(u32, u32)> {
    if orig_width >= crop_width && orig_height >= crop_height {
        Some((crop_width, crop_height))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- default_image_sizes -------------------------------------------------

    #[test]
    fn test_default_image_sizes_count() {
        let sizes = default_image_sizes();
        assert_eq!(sizes.len(), 6);
    }

    #[test]
    fn test_default_image_sizes_names() {
        let sizes = default_image_sizes();
        let names: Vec<&str> = sizes.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "thumbnail",
                "medium",
                "medium_large",
                "large",
                "1536x1536",
                "2048x2048"
            ]
        );
    }

    #[test]
    fn test_default_image_sizes_thumbnail_is_cropped() {
        let sizes = default_image_sizes();
        let thumb = sizes.iter().find(|s| s.name == "thumbnail").unwrap();
        assert_eq!(thumb.width, 150);
        assert_eq!(thumb.height, 150);
        assert!(thumb.crop);
    }

    #[test]
    fn test_default_image_sizes_medium_not_cropped() {
        let sizes = default_image_sizes();
        let medium = sizes.iter().find(|s| s.name == "medium").unwrap();
        assert_eq!(medium.width, 300);
        assert_eq!(medium.height, 300);
        assert!(!medium.crop);
    }

    #[test]
    fn test_default_image_sizes_medium_large_height_zero() {
        let sizes = default_image_sizes();
        let ml = sizes.iter().find(|s| s.name == "medium_large").unwrap();
        assert_eq!(ml.width, 768);
        assert_eq!(ml.height, 0);
        assert!(!ml.crop);
    }

    // -- generate_srcset -----------------------------------------------------

    #[test]
    fn test_generate_srcset_single_entry() {
        let sizes = vec![("https://example.com/img-300x200.jpg".to_string(), 300)];
        let srcset = generate_srcset(&sizes);
        assert_eq!(srcset, "https://example.com/img-300x200.jpg 300w");
    }

    #[test]
    fn test_generate_srcset_multiple_entries() {
        let sizes = vec![
            ("https://example.com/img-300x200.jpg".to_string(), 300),
            ("https://example.com/img-768x512.jpg".to_string(), 768),
            ("https://example.com/img-1024x683.jpg".to_string(), 1024),
        ];
        let srcset = generate_srcset(&sizes);
        assert_eq!(
            srcset,
            "https://example.com/img-300x200.jpg 300w, \
             https://example.com/img-768x512.jpg 768w, \
             https://example.com/img-1024x683.jpg 1024w"
        );
    }

    #[test]
    fn test_generate_srcset_empty() {
        let sizes: Vec<(String, u32)> = vec![];
        let srcset = generate_srcset(&sizes);
        assert_eq!(srcset, "");
    }

    // -- generate_sizes_attr -------------------------------------------------

    #[test]
    fn test_generate_sizes_attr_large() {
        let attr = generate_sizes_attr(1024);
        assert_eq!(attr, "(max-width: 1024px) 100vw, 1024px");
    }

    #[test]
    fn test_generate_sizes_attr_medium() {
        let attr = generate_sizes_attr(300);
        assert_eq!(attr, "(max-width: 300px) 100vw, 300px");
    }

    #[test]
    fn test_generate_sizes_attr_very_large() {
        let attr = generate_sizes_attr(2048);
        assert_eq!(attr, "(max-width: 2048px) 100vw, 2048px");
    }

    // -- calculate_dimensions ------------------------------------------------

    #[test]
    fn test_calculate_dimensions_landscape_into_box() {
        // 2000x1000 into 300x300 -> constrain width first: 300x150
        assert_eq!(calculate_dimensions(2000, 1000, 300, 300), (300, 150));
    }

    #[test]
    fn test_calculate_dimensions_portrait_into_box() {
        // 1000x2000 into 300x300 -> constrain width: 300x600, then height: 150x300
        assert_eq!(calculate_dimensions(1000, 2000, 300, 300), (150, 300));
    }

    #[test]
    fn test_calculate_dimensions_height_unconstrained() {
        // medium_large style: 2000x1000 into 768x0 -> 768x384
        assert_eq!(calculate_dimensions(2000, 1000, 768, 0), (768, 384));
    }

    #[test]
    fn test_calculate_dimensions_already_fits() {
        // Image smaller than the box -- no change
        assert_eq!(calculate_dimensions(200, 100, 300, 300), (200, 100));
    }

    #[test]
    fn test_calculate_dimensions_exact_fit() {
        assert_eq!(calculate_dimensions(300, 300, 300, 300), (300, 300));
    }

    #[test]
    fn test_calculate_dimensions_zero_original() {
        assert_eq!(calculate_dimensions(0, 0, 300, 300), (0, 0));
        assert_eq!(calculate_dimensions(0, 500, 300, 300), (0, 0));
        assert_eq!(calculate_dimensions(500, 0, 300, 300), (0, 0));
    }

    #[test]
    fn test_calculate_dimensions_square_into_landscape_box() {
        // 2000x2000 into 1024x1024 -> 1024x1024
        assert_eq!(calculate_dimensions(2000, 2000, 1024, 1024), (1024, 1024));
    }

    #[test]
    fn test_calculate_dimensions_width_unconstrained() {
        // 1000x2000 into 0x768 -> constrain only height: 384x768
        assert_eq!(calculate_dimensions(1000, 2000, 0, 768), (384, 768));
    }

    // -- calculate_crop_dimensions -------------------------------------------

    #[test]
    fn test_crop_dimensions_sufficient() {
        assert_eq!(
            calculate_crop_dimensions(2000, 1000, 150, 150),
            Some((150, 150))
        );
    }

    #[test]
    fn test_crop_dimensions_too_small_width() {
        assert_eq!(calculate_crop_dimensions(100, 1000, 150, 150), None);
    }

    #[test]
    fn test_crop_dimensions_too_small_height() {
        assert_eq!(calculate_crop_dimensions(1000, 100, 150, 150), None);
    }

    #[test]
    fn test_crop_dimensions_exact_match() {
        assert_eq!(
            calculate_crop_dimensions(150, 150, 150, 150),
            Some((150, 150))
        );
    }
}
