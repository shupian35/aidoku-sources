//! Chapter page parsing.
//!
//! Chapter HTML on rouman5.com embeds the page list in its Next.js RSC
//! streaming payload — a series of `<script>self.__next_f.push([1,"…"])`
//! `</script>` chunks. Once unescaped and concatenated, the payload
//! contains `"imageUrl":"…"` / `"ind":N` pairs that list every page.
//!
//! The chapter detail page *also* renders a `<div … text-right mr-4>1/N頁</div>`
//! widget above the image grid, but the value it carries is regularly
//! stale (e.g. the chapter 0 widget says 73 while the RSC actually carries
//! 128 unique imageUrl/ind pairs). Earlier revisions read that widget and
//! used its N to truncate the page list, which dropped real pages while
//! still surfacing unrelated artwork. We now ignore the widget entirely
//! and derive the page list purely from the RSC payload.

use aidoku::alloc::string::ToString;
use aidoku::alloc::{String, Vec, format};
use aidoku::imports::html::Html;
use aidoku::{HashMap, Page, PageContent, PageContext, Result};

pub(crate) fn parse_chapter_pages(html: &str) -> Result<Vec<String>> {
    // The RSC payload lives in <script>self.__next_f.push([1,"…"])</script>
    // chunks. Iterate every <script>, grab its body, and accumulate the
    // chunks whose body starts with the Next.js marker. Strings inside the
    // chunks are JSON-escaped ("\"", "\\", "\n", …), so we unescape them as
    // we concatenate so the downstream imageUrl/ind scan sees clean JSON.
    let doc = Html::parse(html)?;
    let mut payload = String::new();
    if let Some(scripts) = doc.select("script") {
        for script in scripts {
            // SwiftSoup stores <script> bodies as a DataNode sibling, not the
            // element's text. `Element::data()` returns None for them in the
            // aidoku binding, but `Element::html()` returns the inner HTML —
            // which for a <script> is the script body verbatim.
            let data = match script.html() {
                Some(d) => d,
                None => continue,
            };
            if !data.starts_with("self.__next_f.push") {
                continue;
            }
            let inner = match extract_rsc_chunk(&data) {
                Some(s) => s,
                None => continue,
            };
            unescape_json_string_into(&inner, &mut payload);
        }
    }

    // Extract (imageUrl, ind) pairs from the concatenated payload and dedupe
    // by URL. The deduped list is the chapter's real page count.
    let entries = extract_image_url_ind_pairs(&payload);
    Ok(dedup_preserving_order(entries))
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

/// Strip the `self.__next_f.push([1,"…"])` wrapper from a single RSC chunk
/// (the data we already filtered to start with the marker), returning the
/// inner JSON-escaped string.
fn extract_rsc_chunk(data: &str) -> Option<&str> {
    const OPEN: &str = "self.__next_f.push([1,\"";
    const CLOSE: &str = "\"])";
    let start = data.find(OPEN)? + OPEN.len();
    // Some chunks wrap the call as [1,"…"]; others reuse a numeric chunk id
    // (e.g. [3,"…"]). Strip from the first opening quote after the marker so
    // chunk variants share one extractor.
    let rest = &data[start..];
    let end_rel = rest.rfind(CLOSE)?;
    Some(&rest[..end_rel])
}

/// Append the unescaped JSON string contents of `inner` to `out`. Handles
/// `\"`, `\\`, `\n`, `\r`, `\t`, `\/`, and `\uXXXX`. Anything else is
/// preserved verbatim so unexpected escapes don't truncate the payload.
fn unescape_json_string_into(inner: &str, out: &mut String) {
    let bytes = inner.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'"' => out.push('"'),
                b'\\' => out.push('\\'),
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'/' => out.push('/'),
                b'u' => {
                    if i + 5 < bytes.len() {
                        let hex = &inner[i + 2..i + 6];
                        if let Ok(code) = u32::from_str_radix(hex, 16) {
                            if let Some(c) = char::from_u32(code) {
                                out.push(c);
                            }
                        }
                        i += 4;
                    }
                }
                _ => out.push(bytes[i + 1] as char),
            }
            i += 2;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
}

/// Scan the unescaped RSC payload for `"imageUrl":"…"` / `"ind":N` pairs.
/// Returns pages in source order (the `ind` ordering); the caller is
/// responsible for de-duping the entries.
fn extract_image_url_ind_pairs(payload: &str) -> Vec<(i32, String)> {
    let bytes = payload.as_bytes();
    let needle = b"\"imageUrl\":\"";
    let needle_ind = b"\"ind\":";
    let mut entries: Vec<(i32, String)> = Vec::new();
    let mut i = 0;
    while i + needle.len() < bytes.len() {
        if &bytes[i..i + needle.len()] != needle {
            i += 1;
            continue;
        }
        let url_start = i + needle.len();
        let mut url_end = url_start;
        while url_end < bytes.len() && bytes[url_end] != b'"' {
            url_end += 1;
        }
        if url_end >= bytes.len() {
            break;
        }
        let url: String = payload[url_start..url_end].chars().collect();
        // Next.js byte-chunks the RSC payload and can split a URL's JSON
        // string mid-way (e.g. `"imageUrl":"https"` in one chunk and
        // `://…jpg"` in the next), leaving empty/truncated values that would
        // fail to load and abort the whole chapter.
        if !url.starts_with("http://") && !url.starts_with("https://") {
            i = url_end + 1;
            continue;
        }
        // The matching `"ind":N` lives within the next few hundred bytes.
        let scan_end = core::cmp::min(bytes.len(), url_end + 400);
        let mut j = url_end;
        let mut ind_val: Option<i32> = None;
        while j + needle_ind.len() < scan_end {
            if &bytes[j..j + needle_ind.len()] == needle_ind {
                let mut k = j + needle_ind.len();
                let n_start = k;
                while k < scan_end && (bytes[k] as char).is_ascii_digit() {
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
    }
    entries.sort_by_key(|(n, _)| *n);
    entries
}

/// Sort by `ind`, dedupe identical URLs (Next.js occasionally re-emits the
/// same page across chunks), and surface every unique page.
///
/// The page count widget is ignored here on purpose: rouman5 frequently
/// ships a stale or partial count (e.g. `1/73` for a chapter that actually
/// spans well past 73), and clamping to the widget dropped real pages
/// while still surfacing unrelated artwork.
fn dedup_preserving_order(entries: Vec<(i32, String)>) -> Vec<String> {
    let mut seen: Vec<String> = Vec::with_capacity(entries.len());
    for (_, url) in entries {
        if !seen.contains(&url) {
            seen.push(url);
        }
    }
    seen
}
