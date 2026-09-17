//! Chapter page parsing.
//!
//! Chapter HTML on rouman5.com embeds the page list in one of two shapes:
//!
//! 1. **Current format** — a single inline `<script>` whose body uses
//!    `$R[N]=[…]` reference slots and exposes the page list as
//!    `imagePaths:["…","…",…]`. Entries are JSON-escaped strings in
//!    source order.
//! 2. **Legacy format** — Next.js RSC streaming chunks of the form
//!    `<script>self.__next_f.push([1,"{\"imageUrl\":\"…\",\"ind\":N}"])</script>`
//!    that once concatenated expose `"imageUrl"` / `"ind"` pairs.
//!
//! We try the current format first because that's what the live site
//! ships, and fall back to the RSC scanner so any future rollback or
//! third-party mirror still works.
//!
//! The chapter detail page *also* renders a `<div … text-right mr-4>1/N頁</div>`
//! widget above the image grid, but the value it carries is regularly
//! stale (e.g. the chapter 0 widget says 73 while the page list actually
//! carries 128 URLs). Earlier revisions read that widget and used its N to
//! truncate the page list, which dropped real pages while still surfacing
//! unrelated artwork. We now ignore the widget entirely and derive the
//! page list purely from the script payload.

use aidoku::alloc::string::ToString;
use aidoku::alloc::{String, Vec, format};
use aidoku::imports::html::{Document, Html};
use aidoku::{HashMap, Page, PageContent, PageContext, Result};

pub(crate) fn parse_chapter_pages(html: &str) -> Result<Vec<String>> {
    let doc = Html::parse(html)?;

    if let Some(urls) = parse_image_paths_payload(&doc) {
        return Ok(urls);
    }

    // Legacy: assemble the RSC streaming payload from every
    // <script>self.__next_f.push([1,"…"])</script> chunk and extract
    // (imageUrl, ind) pairs.
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

/// Scan the parsed chapter document for the current `imagePaths:[…]` shape
/// and, if found, return the deduped list of page URLs in source order.
/// Returns `None` when no script body matches — the caller falls back to the
/// legacy RSC parser.
fn parse_image_paths_payload(doc: &Document) -> Option<Vec<String>> {
    let scripts = doc.select("script")?;
    for script in scripts {
        let body = script.html()?;
        let array_start = match find_image_paths_array(&body) {
            Some(s) => s,
            None => continue,
        };
        let raw = &body[array_start..];
        let entries = extract_json_string_array(raw);
        // If the array contained zero entries it isn't the page list —
        // some other payload might happen to mention `imagePaths`. Require
        // at least one real http(s) URL to commit to the new format.
        if entries
            .iter()
            .any(|u| u.starts_with("http://") || u.starts_with("https://"))
        {
            let mut seen: Vec<String> = Vec::with_capacity(entries.len());
            for url in entries {
                if !(url.starts_with("http://") || url.starts_with("https://")) {
                    // Drop truncated/chunk-boundary fragments.
                    continue;
                }
                if !seen.contains(&url) {
                    seen.push(url);
                }
            }
            return Some(seen);
        }
    }
    None
}

/// Find the offset of the JSON array literal that follows `imagePaths:` in
/// `body`. Handles both shapes the site ships:
///
/// - `imagePaths:["…","…",…]`
/// - `imagePaths:$R[27]=["…","…",…]`
///
/// Returns the byte offset of the opening `[` of the array, or `None` if
/// the marker isn't present in this script body.
fn find_image_paths_array(body: &str) -> Option<usize> {
    // Walk every occurrence of the marker so a payload that embeds another
    // `imagePaths` (e.g. a future schema variant) still parses correctly.
    let needle = "imagePaths:";
    let mut search_start = 0;
    while let Some(rel) = body[search_start..].find(needle) {
        let marker_abs = search_start + rel + needle.len();
        // Skip past an optional `$R[N]=` LHS so we land on the array.
        let array_start = match skip_optional_array_lhs(body, marker_abs) {
            Some(idx) => idx,
            None => {
                search_start = marker_abs;
                continue;
            }
        };
        return Some(array_start);
    }
    None
}

/// If `body` at `offset` looks like `$R[N]=` (a Next.js server reference
/// slot prefix), `[N]=`, or directly begins with `[`, advance past the
/// optional LHS and return the index of the array's opening `[`. Returns
/// `None` if the byte sequence doesn't fit any of those shapes.
fn skip_optional_array_lhs(body: &str, offset: usize) -> Option<usize> {
    let bytes = body.as_bytes();
    let mut i = offset;
    if i >= bytes.len() {
        return None;
    }
    // Bare `[` — the LHS is empty. The array starts here.
    if bytes[i] == b'[' {
        return Some(i);
    }
    // Optional `$R[N]=` (Next.js server reference slot) or `[N]=` prefix.
    if bytes[i] == b'$' {
        if !(i + 2 <= bytes.len() && &bytes[i..i + 2] == b"$R") {
            return None;
        }
        i += 2;
        if i >= bytes.len() || bytes[i] != b'[' {
            return None;
        }
        i += 1;
        while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b']' {
            return None;
        }
        i += 1;
    } else if bytes[i] == b'[' {
        i += 1;
        while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b']' {
            return None;
        }
        i += 1;
    } else {
        return None;
    }
    if i < bytes.len() && bytes[i] == b'=' {
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'[' {
        Some(i)
    } else {
        None
    }
}

/// Walk a JSON array literal starting at `[` and yield each entry's
/// unescaped string contents. Respects nesting depth (so an inner `[` in
/// a string doesn't re-enter the array parser) and the JSON string rules
/// (`\"`, `\\`, control escapes, `\uXXXX`). Multi-byte UTF-8 sequences
/// inside strings are copied whole (chunks of CJK characters, etc.) so
/// they survive intact.
fn extract_json_string_array(raw: &str) -> Vec<String> {
    let mut entries: Vec<String> = Vec::new();
    let bytes = raw.as_bytes();
    if bytes.first().copied() != Some(b'[') {
        return entries;
    }
    let mut i = 1; // past the opening `[`
    while i < bytes.len() {
        // Skip whitespace + commas between entries.
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\r' | b',') {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] == b']' {
            break;
        }
        if bytes[i] != b'"' {
            // Unexpected token — bail rather than corrupt state.
            break;
        }
        i += 1; // past the opening `"`
        let mut entry = String::new();
        loop {
            if i >= bytes.len() {
                return entries;
            }
            let c = bytes[i];
            if c == b'"' {
                entries.push(entry);
                i += 1;
                break;
            }
            if c == b'\\' && i + 1 < bytes.len() {
                match bytes[i + 1] {
                    b'"' => entry.push('"'),
                    b'\\' => entry.push('\\'),
                    b'n' => entry.push('\n'),
                    b'r' => entry.push('\r'),
                    b't' => entry.push('\t'),
                    b'/' => entry.push('/'),
                    b'u' => {
                        if i + 6 <= bytes.len() {
                            let hex = &raw[i + 2..i + 6];
                            if let Ok(code) = u32::from_str_radix(hex, 16) {
                                if let Some(ch) = char::from_u32(code) {
                                    entry.push(ch);
                                }
                            }
                            i += 6;
                            continue;
                        }
                    }
                    other => entry.push(other as char),
                }
                i += 2;
                continue;
            }
            // Plain character: copy whole chars (multi-byte UTF-8) until
            // the next `"` or `\`. Using `char_indices` keeps UTF-8 intact
            // — pushing each byte as `char` would corrupt any non-ASCII
            // run (the live chapter URLs include CJK segments inside the
            // base64-decoded path).
            let rest = &raw[i..];
            for (off, ch) in rest.char_indices() {
                let b = rest.as_bytes()[off];
                if b == b'"' || b == b'\\' {
                    break;
                }
                entry.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    entries
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
