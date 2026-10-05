//! Minimal, allocation-free MD5.
//!
//! The de-scramble strip count on this site is derived from an MD5 digest, and
//! an Aidoku source is `#![no_std]` wasm with no crypto dependency available,
//! so the handful of lines it takes to compute one live here.

/// Per-round left-rotation amounts.
const S: [u32; 64] = [
	7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, //
	5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, //
	4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, //
	6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

/// Round constants: `floor(2^32 * abs(sin(i + 1)))`.
const K: [u32; 64] = [
	0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, //
	0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501, //
	0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, //
	0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821, //
	0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, //
	0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8, //
	0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, //
	0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, //
	0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, //
	0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, //
	0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, //
	0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, //
	0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, //
	0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1, //
	0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, //
	0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

const HEX: [u8; 16] = *b"0123456789abcdef";

/// Computes the raw 16-byte MD5 digest of `data`.
pub(crate) fn digest(data: &[u8]) -> [u8; 16] {
	let mut state: [u32; 4] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476];

	// Process every full 64-byte block, then the tail with its padding.
	let full_blocks = data.len() / 64;
	for chunk in data.chunks_exact(64).take(full_blocks) {
		compress(&mut state, chunk);
	}

	let tail = &data[full_blocks * 64..];
	let mut block = [0u8; 64];
	block[..tail.len()].copy_from_slice(tail);
	block[tail.len()] = 0x80;
	if tail.len() + 1 > 56 {
		// No room for the length, so this block is flushed and padded again.
		compress(&mut state, &block);
		block = [0u8; 64];
	}
	let bit_len = (data.len() as u64).wrapping_mul(8);
	block[56..64].copy_from_slice(&bit_len.to_le_bytes());
	compress(&mut state, &block);

	let mut out = [0u8; 16];
	for (i, word) in state.iter().enumerate() {
		out[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
	}
	out
}

/// Lowercase hex encoding of [`digest`].
pub(crate) fn hex(data: &[u8]) -> [u8; 32] {
	let raw = digest(data);
	let mut out = [0u8; 32];
	for (i, byte) in raw.iter().enumerate() {
		out[i * 2] = HEX[(byte >> 4) as usize];
		out[i * 2 + 1] = HEX[(byte & 0x0f) as usize];
	}
	out
}

fn compress(state: &mut [u32; 4], chunk: &[u8]) {
	let mut m = [0u32; 16];
	for (i, word) in m.iter_mut().enumerate() {
		let bytes = &chunk[i * 4..i * 4 + 4];
		*word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
	}

	let (mut a, mut b, mut c, mut d) = (state[0], state[1], state[2], state[3]);

	for i in 0..64 {
		let (f, g) = match i / 16 {
			0 => ((b & c) | (!b & d), i),
			1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
			2 => (b ^ c ^ d, (3 * i + 5) % 16),
			_ => (c ^ (b | !d), (7 * i) % 16),
		};
		let tmp = d;
		d = c;
		c = b;
		let sum = a.wrapping_add(f).wrapping_add(K[i]).wrapping_add(m[g]);
		b = b.wrapping_add(sum.rotate_left(S[i]));
		a = tmp;
	}

	state[0] = state[0].wrapping_add(a);
	state[1] = state[1].wrapping_add(b);
	state[2] = state[2].wrapping_add(c);
	state[3] = state[3].wrapping_add(d);
}
