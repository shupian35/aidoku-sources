//! Unit tests. The de-scramble strip count is the part of this source most
//! likely to break silently, so it is pinned here against values confirmed by
//! measuring real page images: a correct count puts a discontinuity at every
//! band boundary of the scrambled file and none in the reassembled one.

use aidoku::alloc::{String, format, vec::Vec};
use aidoku_test::aidoku_test;

use crate::md5;
use crate::scramble::strip_count;

fn hex_of(data: &[u8]) -> String {
	let bytes = md5::hex(data);
	bytes.iter().map(|byte| *byte as char).collect()
}

#[aidoku_test]
fn md5_matches_known_vectors() {
	assert_eq!(hex_of(b""), "d41d8cd98f00b204e9800998ecf8427e");
	assert_eq!(hex_of(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
	assert_eq!(
		hex_of(b"The quick brown fox jumps over the lazy dog"),
		"9e107d9d372bb6826bd81d3542a419d6"
	);
}

#[aidoku_test]
fn md5_handles_multi_block_input() {
	// 56, 64 and 119 bytes straddle the padding boundary, where a block is
	// flushed early and the length has to go into a second one.
	assert_eq!(
		hex_of(b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
		"3b0c8ac703f828b04c6c197006d17218"
	);
	assert_eq!(
		hex_of(b"1234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890"),
		"49cb3608e2b33fad6b65df8cb8f49668"
	);
	assert_eq!(hex_of(&[b'a'; 119]), "8a7bd0732ed6a28ce75f6dabc90e1613");
}

#[aidoku_test]
fn strip_count_is_zero_below_the_scramble_threshold() {
	// The scramble id doubles as the lower bound; nothing below it is banded.
	assert_eq!(strip_count(1, "220980", "00001"), 0);
	assert_eq!(strip_count(220_979, "220980", "00001"), 0);
}

#[aidoku_test]
fn strip_count_uses_the_legacy_fixed_count() {
	// Albums in the legacy window `[scramble_id, 268850)` always use ten bands,
	// whatever the hash would have said. 268850 itself is the first hashed one.
	assert_eq!(strip_count(220_980, "220980", "00001"), 10);
	assert_eq!(strip_count(240_000, "220980", "00001"), 10);
	assert_eq!(strip_count(268_849, "220980", "00001"), 10);
	assert_eq!(strip_count(268_850, "220980", "00001"), 6);
}

#[aidoku_test]
fn strip_count_matches_measured_pages() {
	// Each of these was confirmed against a live page image: the band count is
	// the one that removes every seam from the reassembled result.
	assert_eq!(strip_count(422_866, "220980", "00001"), 10);
	assert_eq!(strip_count(900_000, "220980", "00001"), 4);
	assert_eq!(strip_count(1_114_751, "220980", "00001"), 8);
	assert_eq!(strip_count(1_114_751, "220980", "00007"), 10);
}

#[aidoku_test]
fn strip_count_always_yields_an_even_positive_number() {
	for album in [
		300_000u64, 400_000, 422_866, 500_000, 900_000, 1_114_751, 1_500_000,
	] {
		for page in 1..=24u32 {
			let count = strip_count(album, "220980", &format!("{:05}", page));
			assert!(count >= 2, "album {album} page {page} -> {count}");
			assert_eq!(count % 2, 0, "album {album} page {page} -> {count}");
		}
	}
}

#[aidoku_test]
fn strip_count_varies_with_album_and_page() {
	// The seed is `album + file stem`, so changing either must be able to move
	// the result; a constant would mean the seed is being ignored.
	let mut seen = Vec::new();
	for page in 1..=40u32 {
		seen.push(strip_count(1_114_751, "220980", &format!("{:05}", page)));
	}
	let first = seen[0];
	assert!(
		seen.iter().any(|count| *count != first),
		"strip count never changed across 40 pages: {seen:?}"
	);
}

#[aidoku_test]
fn missing_scramble_id_falls_back_to_the_default_threshold() {
	// An unparsable scramble id must not disable de-scrambling entirely.
	assert_eq!(
		strip_count(1_114_751, "", "00001"),
		strip_count(1_114_751, "220980", "00001")
	);
	assert_eq!(
		strip_count(1_114_751, "0", "00001"),
		strip_count(1_114_751, "220980", "00001")
	);
	assert_eq!(
		strip_count(1_114_751, "abc", "00001"),
		strip_count(1_114_751, "220980", "00001")
	);
}
