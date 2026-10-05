//! De-scrambling of chapter page images.
//!
//! Page images are published as WebP with their rows cut into `n` horizontal
//! bands that are then stored in reverse order. The band count is not stored
//! anywhere; it is derived from the album id, the scramble id and the page's
//! file stem. The web reader recomputes it and re-stacks the bands on a canvas
//! before display — this module does the same thing through Aidoku's canvas
//! imports so the pages arrive already assembled.

use aidoku::{
	HashMap,
	alloc::{String, format},
	imports::canvas::{Canvas, ImageRef, Rect},
};

use crate::md5;

/// Page context keys consumed by [`unscramble`].
pub(crate) const CTX_CHAPTER: &str = "chapter";
pub(crate) const CTX_SCRAMBLE: &str = "scramble";
pub(crate) const CTX_NAME: &str = "name";

/// The band count the site would use for a given page.
///
/// `chapter_id` is the **chapter** id, not the album id: the reader takes the
/// id from the `/reader/<id>` route and feeds it both to the image path and to
/// this formula. The two only coincide for a single-chapter album, so using
/// the album id silently re-serves the first chapter of a multi-chapter one.
///
/// Mirrors the reader's own helper: the seed is `chapter id + file stem`, and
/// the band count is read out of character 31 of the *hex* digest — an ASCII
/// code, not a decoded byte.
pub(crate) fn strip_count(chapter_id: u64, scramble_id: &str, file_stem: &str) -> usize {
	// `scramble_id` doubles as the lower bound for scrambling; the reader
	// falls back to 220980 when it is missing or not numeric.
	let floor = match parse_u64(scramble_id) {
		Some(value) if value != 0 => value,
		_ => 220980,
	};
	if chapter_id < floor {
		return 0;
	}
	if chapter_id < 268_850 {
		return 10;
	}

	let mut seed = String::new();
	seed.push_str(&format!("{}", chapter_id));
	seed.push_str(file_stem);
	let hex = md5::hex(seed.as_bytes());
	let code = hex[31] as char as u32;

	let base = if chapter_id > 421_926 { 8 } else { 10 };
	((code % base) * 2 + 2) as usize
}

/// Assembles `image` by stacking its bands in the correct order.
///
/// Returns the image untouched when the page is not scrambled, when the
/// context is missing, or when the geometry rules out any banding.
pub(crate) fn unscramble(image: ImageRef, context: &HashMap<String, String>) -> ImageRef {
	let (Some(chapter), Some(scramble), Some(name)) = (
		context.get(CTX_CHAPTER).and_then(|v| parse_u64(v)),
		context.get(CTX_SCRAMBLE).map(String::as_str),
		context.get(CTX_NAME).map(String::as_str),
	) else {
		return image;
	};

	let width = image.width();
	let height = image.height();
	if width <= 0.0 || height <= 0.0 {
		return image;
	}

	let stem = name.rsplit_once('.').map(|(base, _)| base).unwrap_or(name);
	let bands = strip_count(chapter, scramble, stem);
	let total_h = height as u32;
	if bands <= 1 || total_h < bands as u32 * 2 {
		return image;
	}

	// Equal-height bands, with the leftover rows folded into the final one.
	let band_h = total_h / bands as u32;
	let remainder = total_h % bands as u32;

	let mut canvas = Canvas::new(width, height);
	let mut dest_y = 0.0f32;
	for index in (0..bands as u32).rev() {
		let end = band_h * (index + 1)
			+ if index == bands as u32 - 1 {
				remainder
			} else {
				0
			};
		let start = band_h * index;
		let band = (end - start) as f32;
		if band > 0.0 {
			canvas.copy_image(
				&image,
				Rect::new(0.0, start as f32, width, band),
				Rect::new(0.0, dest_y, width, band),
			);
			dest_y += band;
		}
	}

	canvas.get_image()
}

fn parse_u64(value: &str) -> Option<u64> {
	let trimmed = value.trim();
	if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
		return None;
	}
	trimmed.parse::<u64>().ok()
}
