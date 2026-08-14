//! Manga detail and chapter-list parsing.
//!
//! The detail page renders title, cover, author, status, and genre as a
//! vertical stack of labelled rows. Every label (`作者:`, `狀態:`, `地區:`,
//! `標籤:`) is followed by a `<span class="text-foreground">` whose text
//! is the field value, so a single DOM walk over all `span.text-foreground`
//! elements collects every labelled field by inspecting each span's parent
//! `own_text()` for the leading label.
//!
//! JSON-LD is still fetched as a fallback for `description` (only some pages
//! render the `簡介:` paragraph) and is located via the HTML parser rather
//! than by `slice_between` so the script-tag boundary is robust.

use aidoku::alloc::string::ToString;
use aidoku::alloc::{String, Vec, format, vec};
use aidoku::imports::html::{Document, Element, Html};
use aidoku::{Chapter, ContentRating, Manga, MangaStatus, Result, Viewer};

use crate::source_url::get_base_url;

pub(crate) fn parse_manga_detail(html: &str, key: &str) -> Result<Manga> {
    let doc = Html::parse(html)?;

    let title = doc
        .select_first("div.text-xl.text-foreground")
        .and_then(|d| d.text())
        .map(|t| t.trim().to_string())
        .unwrap_or_default();

    let cover = doc
        .select_first("img.rounded")
        .and_then(|img| img.attr("src"))
        .map(|s| absolutize(&s));

    // Pull every labelled row in one DOM walk. The label is the parent div's
    // own_text() (the text before the <span>); the value is the span's text.
    let mut author: Option<String> = None;
    let mut status = MangaStatus::Unknown;
    let mut genre_text: Option<String> = None;
    if let Some(spans) = doc.select("span.text-foreground") {
        for span in spans {
            let label = span.parent().and_then(|p| p.own_text()).unwrap_or_default();
            let value = span.text().unwrap_or_default();
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            // own_text() includes the trailing space between the label and the
            // span, so starts_with is enough — no need to scan for the colon.
            if label.starts_with("作者") {
                author = Some(decode_entities(value));
            } else if label.starts_with("狀態") {
                status = manga_status_from_text(value);
            } else if label.starts_with("標籤") {
                genre_text = Some(value.to_string());
            }
        }
    }

    let tags: Vec<String> = genre_text
        .map(|s| {
            s.split(|c: char| c == ',' || c == '，' || c == '、')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();

    // Description: prefer the rendered `簡介:` paragraph (HTML body); fall
    // back to JSON-LD's `description` field when the page omits the paragraph.
    let description = doc
        .select_first("p")
        .filter(|p| p.own_text().map(|t| t.starts_with("簡介")).unwrap_or(false))
        .and_then(|p| p.text())
        .map(|t| t.trim_start_matches("簡介:").trim().to_string())
        .filter(|t| !t.is_empty())
        .or_else(|| json_ld_description(&doc));

    // Chapters live inside the site's chapter grid; selecting the grid by
    // its exact class excludes the "開始閱讀" button and related-manga links
    // that appear outside it, so no extra filtering is needed.
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
        tags: if tags.is_empty() { None } else { Some(tags) },
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
    // pull the script body via `Element::html()`. For a `<script>` element
    // this returns the verbatim body (no HTML-entity escaping applied).
    let raw = script.html()?;
    let json = raw
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&#039;", "'");
    json_top_level_string(&json, "description").filter(|s| !s.is_empty())
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

/// Build the chapter list by anchoring on the grid container the site renders
/// the chapter list inside. Picking the grid div by its class — rather than
/// just `a[href^="/books/{key}/"]` over the whole page — drops the
/// `開始閱讀` button above the grid and related-manga links under it.
fn parse_chapters(doc: &Document, key: &str) -> Vec<Chapter> {
    let anchor_sel = format!("a[href^=\"/books/{}/\"]", key);
    doc.select_first(
        "div[class=\"grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-2 px-2 py-4\"]",
    )
    .and_then(|grid| grid.select(anchor_sel.as_str()))
    .map(|anchors| {
        anchors
            .map(|a| {
                let href = a.attr("href").unwrap_or_default();
                let index: i32 = href
                    .rsplit('/')
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                let title = a
                    .text()
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty());
                Chapter {
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
                }
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
fn unescape_json_string(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
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
                        let hex = &raw[i + 2..i + 6];
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
    out
}

/// Convenience re-export so callers/tests can pin the Element API we use.
#[allow(dead_code)]
pub(crate) fn _element_marker(_e: Element) {}
