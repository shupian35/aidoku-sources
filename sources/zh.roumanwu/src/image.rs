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

pub(crate) fn unscramble_image(url: &str, image_data: &[u8]) -> Option<ImageRef> {
    let parts: Vec<&str> = url.split('/').collect();
    let last_part = parts.last()?;
    let base64_part = last_part.split('.').collect::<Vec<&str>>();
    let base64_str = &base64_part[..base64_part.len().saturating_sub(1)].join(".");

    let decoded = base64_decode(base64_str);
    let hash = md5_hash(&decoded);
    let last_byte = hash[15];
    let num_slices: i32 = (last_byte as i32 % 10) + 5;

    let src_image = ImageRef::new(image_data);
    let width = src_image.width();
    let height = src_image.height();
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
        canvas.copy_image(&src_image, src_rect, dst_rect);
    }

    Some(canvas.get_image())
}
