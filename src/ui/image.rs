//! Image rendering for terminal UIs
//!
//! Uses ratatui-image to render images using the best available protocol:
//! - Sixel (iTerm2, mlterm, foot, xterm)
//! - Kitty graphics protocol
//! - iTerm2 inline images
//! - Unicode half-blocks (fallback)
//!
//! SVG images are rendered using resvg.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use image::{DynamicImage, RgbaImage};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui_image::StatefulImage;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;

/// Cache for loaded and encoded images
pub struct ImageCache {
    /// The picker for creating protocol instances
    picker: Picker,
    /// Map from image path to loaded image data
    images: HashMap<PathBuf, DynamicImage>,
    /// Map from (path, height) to protocol state for cropped renders
    protocols: HashMap<(PathBuf, u16), StatefulProtocol>,
    /// Base path for resolving relative image paths
    base_path: Option<PathBuf>,
}

impl ImageCache {
    /// Create a new image cache, detecting terminal capabilities
    pub fn new() -> Self {
        // Try to detect terminal graphics capabilities
        // Use from_query_stdio for full detection, but it requires
        // being called after entering alternate screen
        let mut picker = match Picker::from_query_stdio() {
            Ok(p) => {
                tracing::debug!("Terminal graphics protocol detected: {:?}", p.protocol_type());
                p
            }
            Err(e) => {
                tracing::debug!("Failed to detect terminal graphics: {:?}", e);
                // Fallback: use halfblocks with estimated font size
                Picker::from_fontsize((8, 16))
            }
        };

        // If we're in Ghostty and got halfblocks, try forcing Kitty protocol
        // Ghostty supports the Kitty graphics protocol
        if picker.protocol_type() == ProtocolType::Halfblocks && is_ghostty() {
            picker.set_protocol_type(ProtocolType::Kitty);
        }

        Self { picker, images: HashMap::new(), protocols: HashMap::new(), base_path: None }
    }

    /// Create a new image cache with halfblocks only (safe fallback)
    pub fn new_halfblocks() -> Self {
        Self {
            picker: Picker::from_fontsize((8, 16)),
            images: HashMap::new(),
            protocols: HashMap::new(),
            base_path: None,
        }
    }

    /// Set the base path for resolving relative image paths
    pub fn set_base_path(&mut self, path: PathBuf) {
        // If base path changed, clear cache
        if self.base_path.as_ref() != Some(&path) {
            self.images.clear();
            self.protocols.clear();
            self.base_path = Some(path);
        }
    }

    /// Get the current base path
    pub fn base_path(&self) -> Option<&Path> {
        self.base_path.as_deref()
    }

    /// Get the font size (pixels per cell)
    pub fn font_size(&self) -> (u16, u16) {
        self.picker.font_size()
    }

    /// Calculate the recommended row height for an image given available width in columns
    /// Returns the number of terminal rows needed to display the image at proper aspect ratio
    pub fn recommended_rows(&mut self, src: &str, available_cols: u16) -> Option<usize> {
        // Load the image to get dimensions
        let path = self.load_image(src)?;
        let img = self.images.get(&path)?;

        let (font_width, font_height) = self.picker.font_size();

        // Calculate pixel dimensions of the available area
        let available_width_px = available_cols as u32 * font_width as u32;

        // Calculate height needed to maintain aspect ratio
        let img_aspect = img.height() as f32 / img.width() as f32;
        let needed_height_px = (available_width_px as f32 * img_aspect) as u32;

        // Convert to rows, with min/max bounds
        let rows = (needed_height_px / font_height as u32) as usize;

        // Clamp to reasonable bounds (min 8 rows, max 20 rows)
        // Most diagrams don't need more than 20 rows to be readable
        Some(rows.clamp(8, 20))
    }

    /// Resolve an image source path to an absolute path
    fn resolve_path(&self, src: &str) -> Option<PathBuf> {
        let path = Path::new(src);

        // If it's already absolute, use it directly
        if path.is_absolute() {
            if path.exists() {
                return Some(path.to_path_buf());
            }
            return None;
        }

        // Try to resolve relative to base path
        if let Some(base) = &self.base_path {
            let resolved = base.join(src);
            if resolved.exists() {
                return Some(resolved);
            }

            // Also try without leading "./" if present
            let src_cleaned = src.trim_start_matches("./");
            let resolved = base.join(src_cleaned);
            if resolved.exists() {
                return Some(resolved);
            }
        }

        None
    }

    /// Load an image, returning the path key
    fn load_image(&mut self, src: &str) -> Option<PathBuf> {
        let path = self.resolve_path(src)?;

        // Check if already cached
        if self.images.contains_key(&path) {
            return Some(path);
        }

        // Load the image (handle SVG specially)
        let img = if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("svg")) {
            match load_svg(&path) {
                Ok(img) => img,
                Err(e) => {
                    tracing::warn!("Failed to load SVG {}: {}", path.display(), e);
                    return None;
                }
            }
        } else {
            match image::open(&path) {
                Ok(img) => img,
                Err(e) => {
                    tracing::warn!("Failed to load image {}: {}", path.display(), e);
                    return None;
                }
            }
        };

        self.images.insert(path.clone(), img);
        Some(path)
    }

    /// Render an image to the frame with proper clipping
    ///
    /// `full_height` is the height the image should be when fully visible.
    /// `area.height` is the actual visible height - we crop to fit.
    pub fn render_cropped(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        src: &str,
        full_height: u16,
    ) -> bool {
        // Try to load/get cached image
        let Some(path) = self.load_image(src) else {
            return false;
        };

        // If fully visible, use full-size cached protocol
        if area.height >= full_height {
            let cache_key = (path.clone(), full_height);
            if !self.protocols.contains_key(&cache_key) {
                let Some(img) = self.images.get(&path) else {
                    return false;
                };
                let protocol = self.picker.new_resize_protocol(img.clone());
                self.protocols.insert(cache_key.clone(), protocol);
            }
            let Some(protocol) = self.protocols.get_mut(&cache_key) else {
                return false;
            };
            let widget = StatefulImage::default();
            frame.render_stateful_widget(widget, area, protocol);
            return true;
        }

        // Partially visible - need to crop. Quantize to reduce cache entries.
        let quantized = (area.height / 3) * 3 + 3; // Round up to nearest 3
        let cache_key = (path.clone(), quantized);

        if !self.protocols.contains_key(&cache_key) {
            let Some(img) = self.images.get(&path) else {
                return false;
            };

            // Crop top portion of image proportionally
            let visible_ratio = quantized as f32 / full_height as f32;
            let crop_height = ((img.height() as f32) * visible_ratio).ceil() as u32;
            let crop_height = crop_height.min(img.height()).max(1);
            let cropped = img.crop_imm(0, 0, img.width(), crop_height);

            let protocol = self.picker.new_resize_protocol(cropped);
            self.protocols.insert(cache_key.clone(), protocol);
        }

        let Some(protocol) = self.protocols.get_mut(&cache_key) else {
            return false;
        };

        let widget = StatefulImage::default();
        frame.render_stateful_widget(widget, area, protocol);

        true
    }

    /// Render an image to the frame (convenience method for full-size rendering)
    pub fn render(&mut self, frame: &mut Frame, area: Rect, src: &str) -> bool {
        self.render_cropped(frame, area, src, area.height)
    }

    /// Render an image showing the bottom portion (when image scrolls off top)
    ///
    /// `full_height` is the height the image should be when fully visible.
    /// `area.height` is the visible portion - we show the bottom of the image.
    pub fn render_cropped_bottom(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        src: &str,
        full_height: u16,
    ) -> bool {
        // Try to load/get cached image
        let Some(path) = self.load_image(src) else {
            return false;
        };

        // If mostly visible, just use the full image
        if area.height >= full_height {
            return self.render_cropped(frame, area, src, full_height);
        }

        // Crop from bottom of source image. Quantize to reduce cache entries.
        // Use negative values in cache key to distinguish from top crops
        let quantized = (area.height / 3) * 3 + 3;
        let cache_key = (path.clone(), quantized.wrapping_neg());

        if !self.protocols.contains_key(&cache_key) {
            let Some(img) = self.images.get(&path) else {
                return false;
            };

            // Crop bottom portion of image proportionally
            let visible_ratio = quantized as f32 / full_height as f32;
            let crop_height = ((img.height() as f32) * visible_ratio).ceil() as u32;
            let crop_height = crop_height.min(img.height()).max(1);
            let crop_y = img.height().saturating_sub(crop_height);
            let cropped = img.crop_imm(0, crop_y, img.width(), crop_height);

            let protocol = self.picker.new_resize_protocol(cropped);
            self.protocols.insert(cache_key.clone(), protocol);
        }

        let Some(protocol) = self.protocols.get_mut(&cache_key) else {
            return false;
        };

        let widget = StatefulImage::default();
        frame.render_stateful_widget(widget, area, protocol);

        true
    }

    /// Check if an image can be rendered (exists and is loadable)
    pub fn can_render(&self, src: &str) -> bool {
        self.resolve_path(src).is_some()
    }

    /// Clear the image cache
    pub fn clear(&mut self) {
        self.images.clear();
        self.protocols.clear();
    }

    /// Get the number of cached images
    pub fn len(&self) -> usize {
        self.images.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }
}

impl Default for ImageCache {
    fn default() -> Self {
        // Use halfblocks as default to avoid stdio issues
        Self::new_halfblocks()
    }
}

/// Check if the terminal likely supports graphics protocols
pub fn terminal_supports_graphics() -> bool {
    // Check common environment variables
    let term = std::env::var("TERM").unwrap_or_default();
    let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();

    // Known terminals with graphics support
    term.contains("kitty")
        || term.contains("xterm")
        || term_program.contains("iTerm")
        || term_program.contains("WezTerm")
        || term_program.contains("Ghostty")
        || std::env::var("KITTY_WINDOW_ID").is_ok()
        || std::env::var("ITERM_SESSION_ID").is_ok()
}

/// Detect if running in Ghostty terminal
pub fn is_ghostty() -> bool {
    std::env::var("TERM_PROGRAM").map(|v| v.contains("ghostty")).unwrap_or(false)
        || std::env::var("GHOSTTY_RESOURCES_DIR").is_ok()
}

/// Load an SVG file and render it to a raster image
fn load_svg(path: &Path) -> Result<DynamicImage, String> {
    // Read the SVG file
    let svg_data = std::fs::read(path).map_err(|e| format!("Failed to read SVG file: {}", e))?;

    // Create a font database and load system fonts for text rendering
    let mut fontdb = resvg::usvg::fontdb::Database::new();
    fontdb.load_system_fonts();

    // Set up options with the font database
    let options =
        resvg::usvg::Options { fontdb: std::sync::Arc::new(fontdb), ..Default::default() };

    // Parse the SVG with font support
    let tree = resvg::usvg::Tree::from_data(&svg_data, &options)
        .map_err(|e| format!("Failed to parse SVG: {}", e))?;

    // Get the size - render at 2x for good quality without being too large
    // Larger images = slower protocol creation = laggy scrolling
    // Terminal area is typically ~1000x400 pixels, so 1200x900 is plenty
    let size = tree.size();
    let scale = 2.0;
    let width = (size.width() * scale).min(1200.0) as u32;
    let height = (size.height() * scale).min(900.0) as u32;

    if width == 0 || height == 0 {
        return Err("SVG has zero dimensions".to_string());
    }

    // Create a pixmap to render into
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| "Failed to create pixmap".to_string())?;

    // Fill with white background (SVGs often assume white background)
    pixmap.fill(resvg::tiny_skia::Color::WHITE);

    // Render the SVG
    let transform = resvg::tiny_skia::Transform::from_scale(
        width as f32 / size.width(),
        height as f32 / size.height(),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Convert to image::RgbaImage
    let rgba_image = RgbaImage::from_raw(width, height, pixmap.take())
        .ok_or_else(|| "Failed to create RGBA image".to_string())?;

    Ok(DynamicImage::ImageRgba8(rgba_image))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_cache_default() {
        let cache = ImageCache::default();
        assert!(cache.images.is_empty());
        assert!(cache.protocols.is_empty());
        assert!(cache.base_path.is_none());
    }

    #[test]
    fn image_cache_len_and_empty() {
        let cache = ImageCache::default();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn set_base_path_clears_cache() {
        let mut cache = ImageCache::default();
        cache.base_path = Some(PathBuf::from("/old/path"));
        cache.set_base_path(PathBuf::from("/new/path"));
        assert!(cache.images.is_empty());
        assert!(cache.protocols.is_empty());
        assert_eq!(cache.base_path(), Some(Path::new("/new/path")));
    }

    #[test]
    fn resolve_nonexistent_path() {
        let cache = ImageCache::default();
        assert!(cache.resolve_path("/nonexistent/image.png").is_none());
    }

    #[test]
    fn can_render_nonexistent() {
        let cache = ImageCache::default();
        assert!(!cache.can_render("/nonexistent/image.png"));
    }
}
