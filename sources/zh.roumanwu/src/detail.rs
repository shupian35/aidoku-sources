//! Manga detail and chapter-list parsing.
//!
//! The detail page renders title, cover, author, status, region, and update
//! date as rows inside `<dl class="site-book-data">` (each `<dt>LABEL</dt>`
//! followed by `<dd>VALUE</dd>`). The chapter list lives in
//! `<section aria-label="章節目錄"><div class="site-chapters">…</div></section>`
//! with one `<a class="site-chapter-link" href="/books/{key}/{N}">` per
//! entry. The synopsis is `<div class="site-book-synopsis text-foreground">
//! <p>…</p></div>`.
//!
//! JSON-LD is still fetched as a fallback for `description` and is located
//! via the HTML parser rather than by `slice_between` so the script-tag
//! boundary is robust.

use aidoku::alloc::string::ToString;
use aidoku::alloc::{String, Vec, format, vec};
use aidoku::imports::html::{Document, Element, Html};
use aidoku::{Chapter, ContentRating, Manga, MangaStatus, Result, Viewer};

use crate::source_url::get_base_url;

pub(crate) fn parse_manga_detail(html: &str, key: &str) -> Result<Manga> {
    let doc = Html::parse(html)?;

    let title = doc
        .select_first(".site-book-info h1, h1")
        .and_then(|d| d.text())
        .map(|t| t.trim().to_string())
        .unwrap_or_default();

    let cover = doc
        .select_first("img.site-detail-cover[src]")
        .and_then(|img| img.attr("src"))
        .map(|s| absolutize(&s));

    // Walk the `<dl class="site-book-data">` rows. Each row is
    // `<dt>LABEL</dt><dd>VALUE</dd>`; we only act on `作者` and `狀態`
    // (other labels like `地區` / `更新` aren't surfaced by the app).
    let mut author: Option<String> = None;
    let mut status = MangaStatus::Unknown;
    if let Some(dl) = doc.select_first("dl.site-book-data") {
        let mut children = dl.children();
        while let Some(child) = children.next() {
            let label = child.text().unwrap_or_default();
            let Some(dd) = children.next() else { break };
            let value = dd.text().unwrap_or_default();
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            if label.trim_start().starts_with("作者") {
                author = Some(decode_entities(value));
            } else if label.trim_start().starts_with("狀態") {
                status = manga_status_from_text(value);
            }
        }
    }

    // Description: prefer the rendered synopsis paragraph; fall back to
    // JSON-LD's `description` field when the page omits it.
    let description = doc
        .select_first("div.site-book-synopsis p")
        .and_then(|p| p.text())
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .or_else(|| json_ld_description(&doc));

    let mut chapters: Vec<Chapter> = parse_chapters(&doc, key);
    // Newest first: the site grid is oldest-first, readers expect the
    // latest chapter at the top.
    chapters.reverse();

    let url = Some(format!("{}/books/{}", get_base_url(), key));
    Ok(Manga {
        key: key.to_string(),
        title,
        cover,
        artists: None,
        authors: author.filter(|a| !a.is_empty()).map(|a| vec![a]),
        description,
        url,
        tags: None,
        status,
        content_rating: ContentRating::NSFW,
        viewer: Viewer::Webtoon,
        update_strategy: Default::default(),
        next_update_time: None,
        chapters: if chapters.is_empty() {
            None
        } else {
            Some(chapters)
        },
    })
}

// ---------- Helpers ----------

/// Read a `<script type="application/ld+json">` block from the parsed doc and
/// return its `description` field. The script content is JSON, not HTML, so
/// the inner value extraction is still done with the existing JSON helper.
fn json_ld_description(doc: &Document) -> Option<String> {
    let script = doc.select_first("script[type=\"application/ld+json\"]")?;
    // The aidoku-rs test runner doesn't implement `Element::data()`, so we
    // pull the script body via `Element::html()`. The new site HTML-encodes
    // the JSON-LD contents (`描述` → `&#25551;&#36848;`), so we have to
    // decode the numeric entities before handing the string to the JSON
    // extractor. Hex entities (`&#xHHHH;`) are handled too in case the
    // site ever switches.
    let raw = script.html()?;
    let json = decode_script_entities(&raw);
    json_top_level_string(&json, "description").filter(|s| !s.is_empty())
}

/// Decode the HTML entities the site may emit inside a `<script>` body:
/// `&quot;`, `&amp;`, `&#039;`, and numeric entities `&#NNNN;` / `&#xHHHH;`.
///
/// The site double-encodes some characters as `&amp;#NNNN;` (the JSON-LD's
/// CJK chars ship as `&amp;#25551;&amp;#36848;`, for example). A single
/// pass collapses `&amp;` → `&`, leaving `&#25551;&#36848;` which the same
/// pass can then collapse to `描述`. We loop until the string stops
/// shrinking so arbitrarily-deep encodings are handled.
fn decode_script_entities(s: &str) -> String {
    let mut out = String::from(s);
    for _ in 0..4 {
        let next = decode_entities_pass(&out);
        if next == out {
            break;
        }
        out = next;
    }
    out
}

/// One pass of HTML-entity decoding. Unwraps `&quot;`, `&amp;`, `&#039;`,
/// and numeric entities `&#NNNN;` / `&#xHHHH;`. Bytes outside an entity
/// are emitted by copying whole chars (not bytes) so multi-byte UTF-8
/// sequences such as `描述` survive intact.
fn decode_entities_pass(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'&' {
            let mut semi = None;
            for j in (i + 1)..bytes.len().min(i + 12) {
                match bytes[j] {
                    b';' => {
                        semi = Some(j);
                        break;
                    }
                    b'&' | b'>' | b'<' | b'"' | b'\'' | b' ' | b'\t' | b'\n' | b'\r' => break,
                    _ if bytes[j] >= 128 => break,
                    _ => {}
                }
            }
            if let Some(semi) = semi {
                let entity = &s[i + 1..semi];
                if entity == "quot" {
                    out.push('"');
                    i = semi + 1;
                    continue;
                } else if entity == "amp" {
                    out.push('&');
                    i = semi + 1;
                    continue;
                } else if entity == "#039" {
                    out.push('\'');
                    i = semi + 1;
                    continue;
                } else if let Some(num) = entity.strip_prefix('#') {
                    let radix = if let Some(hex) = num.strip_prefix('x') {
                        u32::from_str_radix(hex, 16).ok()
                    } else if let Some(hex) = num.strip_prefix('X') {
                        u32::from_str_radix(hex, 16).ok()
                    } else {
                        num.parse::<u32>().ok()
                    };
                    if let Some(code) = radix {
                        if let Some(c) = char::from_u32(code) {
                            out.push(c);
                            i = semi + 1;
                            continue;
                        }
                    }
                }
            }
        }
        // Not an entity: copy the next full char (handles UTF-8).
        let c = s[i..].chars().next().expect("non-empty at i");
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// Map a `狀態:` value (e.g. `連載中`, `完結`, `休刊`) to `MangaStatus`.
pub(crate) fn manga_status_from_text(value: &str) -> MangaStatus {
    if value.contains("連載中") {
        MangaStatus::Ongoing
    } else if value.contains("完結") {
        MangaStatus::Completed
    } else if value.contains("休刊") || value.contains("停刊") {
        MangaStatus::Hiatus
    } else {
        MangaStatus::Unknown
    }
}

/// Build the chapter list by anchoring on the `<div class="site-chapters">`
/// container. Scoping to that container drops the `開始閱讀` CTA above the
/// grid and the related-manga links under it. Each anchor's `<span title>`
/// is used as the chapter title — anchors wrap the title in a `<span>` and
/// follow it with a `<small>↗</small>` (or `NEW` badge on the freshest
/// chapter), so the full `a.text()` would concatenate that suffix onto the
/// title.
fn parse_chapters(doc: &Document, key: &str) -> Vec<Chapter> {
    let anchor_sel = format!("a[href^=\"/books/{}/\"]", key);
    doc.select_first("div.site-chapters")
        .and_then(|grid| grid.select(anchor_sel.as_str()))
        .map(|anchors| {
            anchors
                .filter_map(|a| {
                    let href = a.attr("href").unwrap_or_default();
                    let index: i32 = href
                        .rsplit('/')
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    // Skip any CTA that happened to be rendered inside the
                    // grid (the current site keeps it outside, but be safe
                    // against future moves).
                    let anchor_text = a.text().unwrap_or_default();
                    if anchor_text.contains("開始閱讀") {
                        return None;
                    }
                    let title = a
                        .select_first("span[title]")
                        .and_then(|s| s.attr("title"))
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty())
                        .or_else(|| {
                            let trimmed = anchor_text.trim();
                            if trimmed.is_empty() {
                                None
                            } else {
                                Some(trimmed.to_string())
                            }
                        });
                    Some(Chapter {
                        key: index.to_string(),
                        title,
                        chapter_number: Some((index + 1) as f32),
                        volume_number: None,
                        date_uploaded: None,
                        scanlators: None,
                        url: Some(format!("{}/books/{}/{}", get_base_url(), key, index)),
                        language: Some("zh".to_string()),
                        thumbnail: None,
                        locked: false,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Resolve a possibly-relative URL against the source base URL.
fn absolutize(url: &str) -> String {
    if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else if let Some(rest) = url.strip_prefix('/') {
        format!("{}/{}", get_base_url().trim_end_matches('/'), rest)
    } else {
        format!("{}/{}", get_base_url().trim_end_matches('/'), url)
    }
}

/// Decode the HTML entities (`&amp;`, `&quot;`) the site emits inside field
/// values. SwiftSoup's `.text()` does not decode them automatically.
pub(crate) fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

/// Extract a top-level string field from a JSON object: `"key":"value"`.
/// Returns `None` when the key is missing or the value is not a string.
///
/// The JSON-LD block is short, well-formed, and untrusted (we control
/// nothing about its content), so a small hand-rolled extractor avoids
/// pulling in a full JSON parser just for one field per page.
pub(crate) fn json_top_level_string(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":\"", key);
    let start = json.find(&needle)? + needle.len();
    let rest = &json[start..];
    let bytes = rest.as_bytes();
    let mut end = 0;
    while end < bytes.len() {
        if bytes[end] == b'"' && (end == 0 || bytes[end - 1] != b'\\') {
            break;
        }
        end += 1;
    }
    if end == 0 || end >= bytes.len() {
        return None;
    }
    Some(unescape_json_string(&rest[..end]))
}

/// Unescape a JSON-encoded string (handles the subset the JSON-LD uses).
/// Iterates by `char` rather than byte so the CJK characters the new
/// site emits literally (rather than via `\uXXXX` escapes) survive intact.
fn unescape_json_string(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.char_indices();
    while let Some((_, c)) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some((_, next)) = chars.next() else { break };
        match next {
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            '/' => out.push('/'),
            'u' => {
                let mut hex = String::with_capacity(4);
                for _ in 0..4 {
                    if let Some((_, c)) = chars.next() {
                        hex.push(c);
                    }
                }
                if let Ok(code) = u32::from_str_radix(&hex, 16) {
                    if let Some(c) = char::from_u32(code) {
                        out.push(c);
                    }
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// Convenience re-export so callers/tests can pin the Element API we use.
#[allow(dead_code)]
pub(crate) fn _element_marker(_e: Element) {}
