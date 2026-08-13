//! Image unscrambling for CDN URLs that ship a shuffled page.
//!
//! Some chapter pages on rouman5.com come back with `sr:1` in the path,
//! meaning the JPEG rows have been reordered. We re-stitch them by hashing
//! the URL and counting slices based on the MD5's last byte.

use aidoku::alloc::Vec;
use aidoku::imports::canvas::{Canvas, ImageRef, Rect};

use crate::utils::{base64_decode, md5_hash};

pub(crate) fn unscramble_image_url(url: &str) -> bool {
    url.contains("sr:1")
}

/// Number of row slices a `sr:1` URL was shuffled into.
///
/// The slice count is a pure function of the URL: the CDN encodes an S3 key
/// as base64 in the final path segment, MD5-hashes it, and derives the count
/// from the hash's last byte (`last_byte % 10 + 5`, i.e. 5..=14).
pub(crate) fn scramble_slices(url: &str) -> Option<i32> {
    let parts: Vec<&str> = url.split('/').collect();
    let last_part = parts.last()?;
    let base64_parts: Vec<&str> = last_part.split('.').collect();
    let base64_str = &base64_parts[..base64_parts.len().saturating_sub(1)].join(".");
    let decoded = base64_decode(base64_str);
    let hash = md5_hash(&decoded);
    Some((hash[15] as i32 % 10) + 5)
}

/// Re-stitch a shuffled `sr:1` page into a correctly-ordered image.
///
/// Takes the already-decoded [`ImageRef`] directly so we avoid the
/// encode-then-decode round-trip that `image.data()` → `ImageRef::new()`
/// would force: `get_image_data` re-encodes the bitmap only for `new_image`
/// to decode it straight back. The app hands us a decoded `ImageRef` in
/// `ImageResponse::image`, so we copy rows straight from it.
pub(crate) fn unscramble_image(url: &str, src: &ImageRef) -> Option<ImageRef> {
    let num_slices = scramble_slices(url)?;
    let width = src.width();
    let height = src.height();
    if width <= 0.0 || height <= 0.0 {
        return None;
    }

    let mut canvas = Canvas::new(width, height);
    let slice_height = (height as i32 / num_slices) as f32;
    let height_offset = (height as i32 % num_slices) as f32;

    for l in 0..num_slices {
        let (src_y, dst_y, h) = if l == 0 {
            (
                height - slice_height - height_offset,
                0.0,
                slice_height + height_offset,
            )
        } else {
            (
                height - slice_height * (l as f32 + 1.0) - height_offset,
                slice_height * l as f32 + height_offset,
                slice_height,
            )
        };

        let src_rect = Rect {
            x: 0.0,
            y: src_y,
            width,
            height: h,
        };
        let dst_rect = Rect {
            x: 0.0,
            y: dst_y,
            width,
            height: h,
        };
        canvas.copy_image(src, src_rect, dst_rect);
    }

    Some(canvas.get_image())
}
