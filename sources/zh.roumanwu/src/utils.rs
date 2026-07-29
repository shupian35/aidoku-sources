//! Generic utility helpers used by the roumanwu source.
//!
//! These functions are deliberately source-agnostic. Anything that requires
//! knowledge about rouman5.com specifically (HTTP fetching, page-index
//! conventions, DOM selectors, etc.) belongs in lib.rs instead.

use aidoku::alloc::{format, string::ToString, String, Vec};

// ---------- Encoding / hashing ----------

pub(crate) fn urlencode(s: &str) -> String {
	let mut out = String::with_capacity(s.len());
	for &b in s.as_bytes() {
		let safe = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~');
		if safe {
			out.push(b as char);
		} else {
			const HEX: &[u8; 16] = b"0123456789ABCDEF";
			out.push('%');
			out.push(HEX[(b >> 4) as usize] as char);
			out.push(HEX[(b & 0x0F) as usize] as char);
		}
	}
	out
}


pub(crate) fn site_page(page: i32) -> i32 {
	if page < 1 {
		0
	} else {
		page - 1
	}
}


pub(crate) fn extract_url_from_style(style: &str) -> Option<String> {
	let s = style.replace("&quot;", "\"");
	let start = s.find("url(\"")? + 5;
	let rest = &s[start..];
	let end = rest.find(0x22 as char)?;
	Some(rest[..end].to_string())
}

pub(crate) fn slice_between<'a>(html: &'a str, start: &str, end: &str) -> Option<&'a str> {
	let s = html.find(start)? + start.len();
	let e = html[s..].find(end)? + s;
	Some(&html[s..e])
}


// ---------- MD5 ----------

pub(crate) fn md5_hash(data: &[u8]) -> [u8; 16] {
    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xEFCDAB89;
    let mut h2: u32 = 0x98BADCFE;
    let mut h3: u32 = 0x10325476;
    
    // Pre-processing: adding padding bits
    let original_len = data.len();
    let bit_len = (original_len as u64) * 8;
    
    // Calculate padded length
    let mut padded_len = original_len + 1; // +1 for 0x80 byte
    while padded_len % 64 != 56 {
        padded_len += 1;
    }
    padded_len += 8; // +8 for length
    
    // Create padded message
    let mut msg = Vec::with_capacity(padded_len);
    msg.extend_from_slice(data);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());
    
    // Process each 512-bit block
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 16];
        for i in 0..16 {
            w[i] = u32::from_le_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        
        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        
        // Round 1
        for i in 0..16 {
            let f = (b & c) | ((!b) & d);
            let g = i;
            let temp = d;
            d = c;
            c = b;
            let k = [0xD76AA478, 0xE8C7B756, 0x242070DB, 0xC1BDCEEE,
                     0xF57C0FAF, 0x4787C62A, 0xA8304613, 0xFD469501,
                     0x698098D8, 0x8B44F7AF, 0xFFFF5BB1, 0x895CD7BE,
                     0x6B901122, 0xFD987193, 0xA679438E, 0x49B40821];
            let s = [7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22];
            b = b.wrapping_add(
                a.wrapping_add(f).wrapping_add(k[i]).wrapping_add(w[g])
                    .rotate_left(s[i])
            );
            a = temp;
        }
        
        // Round 2
        for i in 0..16 {
            let f = (d & b) | ((!d) & c);
            let g = (5 * i + 1) % 16;
            let temp = d;
            d = c;
            c = b;
            let k = [0xF61E2562, 0xC040B340, 0x265E5A51, 0xE9B6C7AA,
                     0xD62F105D, 0x02441453, 0xD8A1E681, 0xE7D3FBC8,
                     0x21E1CDE6, 0xC33707D6, 0xF4D50D87, 0x455A14ED,
                     0xA9E3E905, 0xFCEFA3F8, 0x676F02D9, 0x8D2A4C8A];
            let s = [5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20];
            b = b.wrapping_add(
                a.wrapping_add(f).wrapping_add(k[i]).wrapping_add(w[g])
                    .rotate_left(s[i])
            );
            a = temp;
        }
        
        // Round 3
        for i in 0..16 {
            let f = b ^ c ^ d;
            let g = (3 * i + 5) % 16;
            let temp = d;
            d = c;
            c = b;
            let k = [0xFFFA3942, 0x8771F681, 0x6D9D6122, 0xFDE5380C,
                     0xA4BEEA44, 0x4BDECFA9, 0xF6BB4B60, 0xBEBFBC70,
                     0x289B7EC6, 0xEAA127FA, 0xD4EF3085, 0x04881D05,
                     0xD9D4D039, 0xE6DB99E5, 0x1FA27CF8, 0xC4AC5665];
            let s = [4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23];
            b = b.wrapping_add(
                a.wrapping_add(f).wrapping_add(k[i]).wrapping_add(w[g])
                    .rotate_left(s[i])
            );
            a = temp;
        }
        
        // Round 4
        for i in 0..16 {
            let f = c ^ (b | (!d));
            let g = (7 * i) % 16;
            let temp = d;
            d = c;
            c = b;
            let k = [0xF4292244, 0x432AFF97, 0xAB9423A7, 0xFC93A039,
                     0x655B59C3, 0x8F0CCC92, 0xFFEFF47D, 0x85845DD1,
                     0x6FA87E4F, 0xFE2CE6E0, 0xA3014314, 0x4E0811A1,
                     0xF7537E82, 0xBD3AF235, 0x2AD7D2BB, 0xEB86D391];
            let s = [6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21];
            b = b.wrapping_add(
                a.wrapping_add(f).wrapping_add(k[i]).wrapping_add(w[g])
                    .rotate_left(s[i])
            );
            a = temp;
        }
        
        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
    }
    
    let mut result = [0u8; 16];
    result[0..4].copy_from_slice(&h0.to_le_bytes());
    result[4..8].copy_from_slice(&h1.to_le_bytes());
    result[8..12].copy_from_slice(&h2.to_le_bytes());
    result[12..16].copy_from_slice(&h3.to_le_bytes());
    result
}


// ---------- Base64 ----------

pub(crate) fn base64_decode(input: &str) -> Vec<u8> {
    let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = Vec::new();
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    
    for c in input.chars() {
        if c == '=' {
            break;
        }
        if let Some(val) = alphabet.find(c) {
            buf = (buf << 6) | val as u32;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                output.push((buf >> bits) as u8);
            }
        }
    }
    
    output
}


// ---------- JSON helpers ----------

pub(crate) fn json_string(haystack: &str, key: &str) -> Option<String> {
	let needle = format!("\"{}\":\"", key);
	let i = haystack.find(&needle)? + needle.len();
	let rest = &haystack[i..];
	// Find the closing quote, handling escaped quotes
	let mut j = 0;
	let chars: Vec<char> = rest.chars().collect();
	while j < chars.len() {
		if chars[j] == '"' && (j == 0 || chars[j - 1] != '\\') {
			break;
		}
		j += 1;
	}
	if j == 0 || j >= chars.len() {
		return None;
	}
	let raw: String = chars[..j].iter().collect();
	// Process escape sequences
	let mut out = String::with_capacity(raw.len());
	let mut chars_iter = raw.chars().peekable();
	while let Some(c) = chars_iter.next() {
		if c == '\\' {
			if let Some(&n) = chars_iter.peek() {
				match n {
					'"' => { out.push('"'); chars_iter.next(); }
					'\\' => { out.push('\\'); chars_iter.next(); }
					'n' => { out.push('\n'); chars_iter.next(); }
					'r' => { out.push('\r'); chars_iter.next(); }
					't' => { out.push('\t'); chars_iter.next(); }
					'/' => { out.push('/'); chars_iter.next(); }
					'u' => {
						chars_iter.next(); // skip 'u'
						let mut hex = String::with_capacity(4);
						for _ in 0..4 {
							if let Some(h) = chars_iter.next() {
								hex.push(h);
							}
						}
						if let Ok(code) = u32::from_str_radix(&hex, 16) {
							if let Some(uch) = char::from_u32(code) {
								out.push(uch);
							}
						}
					}
					_ => {
						out.push(c);
						out.push(n);
						chars_iter.next();
					}
				}
			} else {
				out.push(c);
			}
		} else {
			out.push(c);
		}
	}
	Some(out)
}


