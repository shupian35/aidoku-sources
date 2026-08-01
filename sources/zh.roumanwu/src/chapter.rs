//! Chapter page parsing.
//!
//! Chapter HTML on rouman5.com embeds the page list inside Next.js streaming
//! payload chunks (`<script>self.__next_f.push([1,"..."])`). Each chunk is
//! JSON-string-escaped; once unescaped and concatenated we look for
//! `"imageUrl":"..."` pairs and their `"ind":N` indexes, sort by `ind`, and
//! dedupe.

use aidoku::Result;
use aidoku::alloc::{String, Vec};

use crate::utils::slice_between;

pub(crate) fn parse_chapter_pages(html: &str) -> Result<(i32, Vec<String>)> {
    let mut payload = String::with_capacity(html.len());
    let marker = "<script>self.__next_f.push([1,\"";
    let mut cursor = 0;
    while let Some(rel) = html[cursor..].find(marker) {
        let abs = cursor + rel;
        let after = &html[abs + marker.len()..];
        let end_marker = "])</script>";
        if let Some(close_rel) = after.find(end_marker) {
            let chunk_bytes = after[..close_rel].as_bytes();
            let mut unescaped = String::with_capacity(chunk_bytes.len());
            let mut k = 0;
            while k < chunk_bytes.len() {
                if chunk_bytes[k] == b'\\' && k + 1 < chunk_bytes.len() {
                    let n = chunk_bytes[k + 1];
                    match n {
                        b'"' => unescaped.push('"'),
                        b'\\' => unescaped.push('\\'),
                        b'n' => unescaped.push('\n'),
                        b'r' => unescaped.push('\r'),
                        _ => {
                            unescaped.push(chunk_bytes[k] as char);
                            unescaped.push(n as char);
                        }
                    }
                    k += 2;
                } else {
                    unescaped.push(chunk_bytes[k] as char);
                    k += 1;
                }
            }
            payload.push_str(&unescaped);
            cursor = abs + marker.len() + close_rel + end_marker.len();
        } else {
            break;
        }
    }

    // Determine page count
    let mut page_count: i32 = {
        let json_ld_raw = slice_between(html, "<script type=\"application/ld+json\">", "</script>")
            .unwrap_or("")
            .replace("&quot;", "\"")
            .replace("&amp;", "&");
        let needle = "numberOfPages";
        let v = if let Some(i) = json_ld_raw.find(needle) {
            let after = &json_ld_raw[i + needle.len()..];
            let mut s = 0;
            while s < after.len() && (after.as_bytes()[s] == b':' || after.as_bytes()[s] == b' ') {
                s += 1;
            }
            let digits: String = after[s..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            digits.parse().unwrap_or(0)
        } else {
            0
        };
        v
    };
    if page_count == 0 {
        let mut i = 0;
        while let Some(rel) = html[i..].find("<!-- -->/<!-- -->") {
            let abs = i + rel;
            let after = &html[abs + 18..];
            let head: String = after.chars().take(40).collect();
            let after_digits: String = head.chars().skip_while(|c| c.is_ascii_digit()).collect();
            if let Some(d_end) = after_digits.find("<!-- -->頁") {
                let digits: String = after_digits[..d_end]
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(n) = digits.parse::<i32>() {
                    page_count = n;
                    break;
                }
            }
            i = abs + 1;
        }
    }
    if page_count == 0 {
        let mut i = 0;
        while let Some(rel) = payload[i..].find("\"/\",") {
            let abs = i + rel;
            let after = &payload[abs + 4..];
            let head: String = after.chars().take(60).collect();
            if let Some(d_end) = head.find("\",\"頁\"") {
                let digits: String = head[..d_end]
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                if let Ok(n) = digits.parse::<i32>() {
                    page_count = n;
                    break;
                }
            }
            i = abs + 1;
        }
    }

    // Extract (imageUrl, ind) pairs from the concatenated payload
    let mut entries: Vec<(i32, String)> = Vec::new();
    let bytes = payload.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 12 < bytes.len() && &bytes[i..i + 12] == b"\"imageUrl\":\"" {
            let url_start = i + 12;
            let mut url_end = url_start;
            while url_end < bytes.len() && bytes[url_end] != b'"' {
                url_end += 1;
            }
            if url_end >= bytes.len() {
                break;
            }
            let url: String = payload[url_start..url_end].chars().collect();
            let mut j = url_end;
            let mut ind_val: Option<i32> = None;
            let ind_needle = b"\"ind\":";
            let scan_end = core::cmp::min(bytes.len(), url_end + 400);
            while j < scan_end {
                if j + ind_needle.len() < bytes.len()
                    && &bytes[j..j + ind_needle.len()] == ind_needle
                {
                    let mut k = j + ind_needle.len();
                    let n_start = k;
                    while k < bytes.len() && (bytes[k] as char).is_ascii_digit() {
                        k += 1;
                    }
                    if k > n_start {
                        let num: String = payload[n_start..k].chars().collect();
                        if let Ok(n) = num.parse::<i32>() {
                            ind_val = Some(n);
                        }
                    }
                    break;
                }
                j += 1;
            }
            if let Some(n) = ind_val {
                entries.push((n, url));
            }
            i = url_end + 1;
        } else {
            i += 1;
        }
    }

    entries.sort_by_key(|(n, _)| *n);
    let mut pages: Vec<String> = Vec::with_capacity(entries.len());
    for (_, url) in entries {
        if !pages.contains(&url) {
            pages.push(url);
        }
    }

    Ok((page_count, pages))
}
