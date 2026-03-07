//! WordPress-compatible image processing and thumbnail generation.
//!
//! Equivalent to `wp-includes/media.php` and `wp-admin/includes/image.php`.
//! Generates thumbnail, medium, and large image sizes on upload.

use image::imageops::FilterType;
use std::path::Path;
use tracing::{debug, error};

/// WordPress default image sizes.
#[derive(Debug, Clone)]
pub struct ImageSize {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub crop: bool,
}

/// Metadata for a generated image size.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GeneratedSize {
    pub file: String,
    pub width: u32,
    pub height: u32,
    pub mime_type: String,
}

/// Result of processing an uploaded image.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImageMetadata {
    pub width: u32,
    pub height: u32,
    pub file: String,
    pub sizes: std::collections::HashMap<String, GeneratedSize>,
}

/// Get the default WordPress image sizes.
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
    ]
}

/// Process an uploaded image file: generate thumbnails and return metadata.
///
/// This is the equivalent of WordPress's `wp_generate_attachment_metadata()`.
///
/// # Arguments
/// * `file_path` - Path to the original uploaded image
/// * `upload_dir` - Directory to save generated thumbnails
/// * `sizes` - Image sizes to generate (use `default_image_sizes()` for defaults)
pub fn process_image(
    file_path: &Path,
    upload_dir: &Path,
    sizes: &[ImageSize],
) -> Result<ImageMetadata, String> {
    let img = image::open(file_path).map_err(|e| format!("Failed to open image: {e}"))?;

    let original_width = img.width();
    let original_height = img.height();

    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image");
    let file_stem = file_path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("image");
    let extension = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg");

    let mut generated_sizes = std::collections::HashMap::new();

    for size in sizes {
        // Skip if the original is smaller than the target
        if original_width <= size.width && original_height <= size.height {
            continue;
        }
        if size.width == 0 && size.height == 0 {
            continue;
        }

        let (new_width, new_height) = if size.crop {
            // Crop to exact dimensions
            (size.width, size.height)
        } else {
            // Scale proportionally
            calculate_proportional_size(original_width, original_height, size.width, size.height)
        };

        if new_width == 0 || new_height == 0 {
            continue;
        }

        let resized = if size.crop {
            img.resize_to_fill(new_width, new_height, FilterType::Lanczos3)
        } else {
            img.resize(new_width, new_height, FilterType::Lanczos3)
        };

        let size_filename = format!("{file_stem}-{new_width}x{new_height}.{extension}");
        let size_path = upload_dir.join(&size_filename);

        match resized.save(&size_path) {
            Ok(_) => {
                debug!(
                    size = size.name,
                    width = new_width,
                    height = new_height,
                    "generated image size"
                );
                generated_sizes.insert(
                    size.name.clone(),
                    GeneratedSize {
                        file: size_filename,
                        width: new_width,
                        height: new_height,
                        mime_type: mime_from_extension(extension),
                    },
                );
            }
            Err(e) => {
                error!(
                    size = size.name,
                    error = %e,
                    "failed to save image size"
                );
            }
        }
    }

    Ok(ImageMetadata {
        width: original_width,
        height: original_height,
        file: file_name.to_string(),
        sizes: generated_sizes,
    })
}

/// Calculate proportional dimensions while respecting max width/height.
fn calculate_proportional_size(orig_w: u32, orig_h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    let ratio = orig_w as f64 / orig_h as f64;

    let (mut new_w, mut new_h) = if max_w > 0 && max_h > 0 {
        // Both constrained
        let w = max_w;
        let h = (w as f64 / ratio) as u32;
        if h > max_h {
            let h = max_h;
            let w = (h as f64 * ratio) as u32;
            (w, h)
        } else {
            (w, h)
        }
    } else if max_w > 0 {
        // Width only
        let w = max_w;
        let h = (w as f64 / ratio) as u32;
        (w, h)
    } else {
        // Height only
        let h = max_h;
        let w = (h as f64 * ratio) as u32;
        (w, h)
    };

    // Don't upscale
    if new_w > orig_w {
        new_w = orig_w;
        new_h = orig_h;
    }

    (new_w, new_h)
}

fn mime_from_extension(ext: &str) -> String {
    match ext.to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg".to_string(),
        "png" => "image/png".to_string(),
        "gif" => "image/gif".to_string(),
        "webp" => "image/webp".to_string(),
        "bmp" => "image/bmp".to_string(),
        "svg" => "image/svg+xml".to_string(),
        _ => format!("image/{ext}"),
    }
}

/// Serialize image metadata to WordPress `_wp_attachment_metadata` format.
///
/// WordPress stores this as serialized PHP, but we use JSON internally
/// and convert if needed for WP compatibility.
pub fn serialize_metadata(metadata: &ImageMetadata) -> String {
    serde_json::to_string(metadata).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_proportional_size() {
        // Landscape image, constrained to 300x300
        let (w, h) = calculate_proportional_size(1200, 800, 300, 300);
        assert_eq!(w, 300);
        assert_eq!(h, 200);

        // Portrait image, constrained to 300x300
        let (w, h) = calculate_proportional_size(800, 1200, 300, 300);
        assert_eq!(w, 200);
        assert_eq!(h, 300);

        // Width only constraint
        let (w, h) = calculate_proportional_size(1200, 800, 768, 0);
        assert_eq!(w, 768);
        assert_eq!(h, 512);
    }

    #[test]
    fn test_default_image_sizes() {
        let sizes = default_image_sizes();
        assert_eq!(sizes.len(), 4);
        assert_eq!(sizes[0].name, "thumbnail");
        assert_eq!(sizes[0].width, 150);
        assert!(sizes[0].crop);
        assert_eq!(sizes[1].name, "medium");
        assert!(!sizes[1].crop);
    }

    #[test]
    fn test_mime_from_extension() {
        assert_eq!(mime_from_extension("jpg"), "image/jpeg");
        assert_eq!(mime_from_extension("PNG"), "image/png");
        assert_eq!(mime_from_extension("webp"), "image/webp");
    }
}
