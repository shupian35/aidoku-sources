//! Chapter page parsing.
//!
//! Chapter HTML on rouman5.com embeds the page list inside Next.js streaming
//! payload chunks (`<script>self.__next_f.push([1,"..."])`). Each chunk is
//! JSON-string-escaped; once unescaped and concatenated we look for
//! `"imageUrl":"..."` pairs and their `"ind":N` indexes, sort by `ind`, and
//! dedupe.
use aidoku::alloc::string::ToString;
use aidoku::alloc::{String, Vec, format};
use aidoku::{HashMap, Page, PageContent, PageContext, Result};

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

    // Determine page count via the prioritized heuristic chain.
    // None means no heuristic matched; caller treats that as 0 to
    // preserve prior behaviour.
    let page_count: i32 = page_count(html, &payload).unwrap_or(0);
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
/// Resolve a chapter path into an absolute URL.
///
/// `path` may already be absolute (preferred — Aidoku's chapter detail
/// page stores absolute URLs so the "open in browser" button works)
/// or relative (legacy). Only prepend `base` for relative paths so we
/// never produce `https://xhttps://x/...`.
pub(crate) fn resolve_chapter_url(path: &str, base: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else {
        format!("{}{}", base, path)
    }
}

/// Truncate a parsed page URL list to `page_count`.
///
/// The RSC payload sometimes embeds imageUrl entries for related-manga
/// cards alongside real chapter pages; `page_count` (from JSON-LD or a
/// HTML comment heuristic) trims those off.
pub(crate) fn truncate_to_page_count(urls: Vec<String>, page_count: i32) -> Vec<String> {
    if page_count > 0 && urls.len() > page_count as usize {
        urls[..page_count as usize].to_vec()
    } else {
        urls
    }
}

/// Build Aidoku `Page` records from a list of `(url, is_scrambled)` pairs.
///
/// Caller decides which URLs need unscrambling (rouman5 CDN marks them
/// `sr:1`). This module knows nothing about that detection — it just
/// surfaces the tag via `PageContext` so `PageImageProcessor` can
/// unscramble lazily as each image loads.
pub(crate) fn build_pages(urls: Vec<(String, bool)>) -> Vec<Page> {
    urls.into_iter()
        .map(|(url, scramble)| {
            if scramble {
                let mut ctx: PageContext = HashMap::new();
                ctx.insert("scramble".into(), "1".into());
                Page {
                    content: PageContent::url_context(url, ctx),
                    thumbnail: None,
                    has_description: false,
                    description: None,
                }
            } else {
                Page {
                    content: PageContent::url(url),
                    thumbnail: None,
                    has_description: false,
                    description: None,
                }
            }
        })
        .collect()
}

// ---------- Page-count heuristics ----------

/// Site-specific way of extracting the page count from a chapter HTML.
///
/// Each variant owns one encoding the CDN ships (JSON-LD schema, HTML
/// comment split, React Server Components payload). The chain walks them in
/// priority order and returns the first match.
enum PageCountHeuristic {
    JsonLd,
    HtmlComment,
    RscPayload,
}

impl PageCountHeuristic {
    fn run(&self, html: &str, payload: &str) -> Option<i32> {
        match self {
            Self::JsonLd => json_ld_count(html),
            Self::HtmlComment => html_comment_count(html),
            Self::RscPayload => rsc_payload_count(payload),
        }
    }
}

const PAGE_COUNT_CHAIN: &[PageCountHeuristic] = &[
    PageCountHeuristic::JsonLd,
    PageCountHeuristic::HtmlComment,
    PageCountHeuristic::RscPayload,
];

/// Walk the page-count heuristic chain and return the first match.
///
/// Returns `None` when no heuristic extracts a count. Callers decide how to
/// interpret "unknown" — `parse_chapter_pages` falls back to `0`.
pub(crate) fn page_count(html: &str, payload: &str) -> Option<i32> {
    PAGE_COUNT_CHAIN.iter().find_map(|h| h.run(html, payload))
}

fn json_ld_count(html: &str) -> Option<i32> {
    let json_ld_raw = slice_between(html, "<script type=\"application/ld+json\">", "</script>")
        .unwrap_or("")
        .replace("&quot;", "\"")
        .replace("&amp;", "&");
    let needle = "numberOfPages";
    let i = json_ld_raw.find(needle)?;
    let after = &json_ld_raw[i + needle.len()..];
    let mut s = 0;
    while s < after.len() && (after.as_bytes()[s] == b':' || after.as_bytes()[s] == b' ') {
        s += 1;
    }
    let digits: String = after[s..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

fn html_comment_count(html: &str) -> Option<i32> {
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
                return Some(n);
            }
        }
        i = abs + 1;
    }
    None
}

fn rsc_payload_count(payload: &str) -> Option<i32> {
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
                return Some(n);
            }
        }
        i = abs + 1;
    }
    None
}
