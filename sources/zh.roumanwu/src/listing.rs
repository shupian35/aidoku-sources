//! Listing / search page parsing.
//!
//! The site's listing, search, and home-page sections all render manga cards
//! with the same DOM (`<a class="site-comic" href="/books/{key}">…</a>`).
//! `extract_manga_cards` parses that shared shape and is reused by both
//! `parse_manga_listing` (here) and `parse_home_layout` in `home.rs`.
//! Pagination is inferred from the presence of a `page=N+1` link or a
//! "下一頁" marker; that marker only matters for listing pages.

use aidoku::alloc::string::ToString;
use aidoku::alloc::{String, Vec, format, vec};
use aidoku::imports::html::Html;
use aidoku::{ContentRating, Manga, MangaPageResult, Result, Viewer};

use crate::source_url::get_base_url;

/// Extract every manga card from a chunk of HTML.
///
/// Skips anchors whose `href` doesn't look like a manga path (chapter
/// anchors carry three `/` segments), dedupes by `key`, and ignores entries
/// with no resolvable title. Both listing pages and home-page sections call
/// this with their respective HTML slices.
///
/// The new site renders each card as
/// `<a class="site-comic" href="/books/{key}"><div class="site-comic-cover">
/// <img src="…"></div><h3 title="…">…</h3><div class="site-comic-meta">
/// <span class="site-comic-chapter">…</span><span class="shrink-0">…</span>
/// </div><div class="site-comic-meta"><span>REGION</span><span>◉ N</span>
/// </div></a>`. Title comes from the `<h3>` (preferring its `title` attribute
/// so we don't pick up whitespace), cover from the `img[src]`, latest
/// chapter text from `span.site-comic-chapter`, and tags from the second
/// `<div class="site-comic-meta">` (region + views).
pub(crate) fn extract_manga_cards(html: &str) -> Result<Vec<Manga>> {
    let doc = Html::parse(html)?;
    let anchors = match doc.select("a[href^=\"/books/\"]") {
        Some(a) => a,
        None => return Ok(Vec::new()),
    };

    let mut entries: Vec<Manga> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    for a in anchors {
        let href = match a.attr("href") {
            Some(h) => h,
            None => continue,
        };
        // /books/{id} has 2 slashes; /books/{id}/{N} has 3 — keep only manga entries.
        if href.matches('/').count() != 2 {
            continue;
        }
        let key = match href.rsplit('/').next() {
            Some(k) if !k.is_empty() => k.to_string(),
            _ => continue,
        };
        if seen.contains(&key) {
            continue;
        }
        let title = a
            .select_first("h3[title]")
            .and_then(|e| e.attr("title"))
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .or_else(|| {
                a.select_first("h3")
                    .and_then(|e| e.text())
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
            });
        let Some(title) = title else { continue };
        let cover = a
            .select_first("div.site-comic-cover > img[src]")
            .and_then(|img| img.attr("src"));
        // Latest chapter label (e.g. "第8話 8"). Search pages carry a
        // category here ("Sanku", "Room308") instead; we still surface it
        // as the description so the app's listing cards aren't empty.
        let latest = a
            .select_first("span.site-comic-chapter")
            .and_then(|e| e.text())
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());

        // Stats: pull every <span> inside the last `div.site-comic-meta`
        // block. The site renders region + views there; some search results
        // leave the views span empty, so we accept the row as long as the
        // region span is populated.
        let tags: Option<Vec<String>> = a
            .select("div.site-comic-meta")
            .and_then(|list| list.into_iter().last())
            .and_then(|row| row.select("span"))
            .map(|spans| {
                spans
                    .into_iter()
                    .filter_map(|s| s.text())
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<String>>()
            })
            .and_then(|spans| {
                let region = spans.first().cloned();
                let views = spans.get(1).cloned();
                match (region, views) {
                    (Some(r), Some(v)) if !r.is_empty() && !v.is_empty() => {
                        Some(vec![format!("地区 {}", r), format!("浏览 {}", v)])
                    }
                    (Some(r), None) if !r.is_empty() => Some(vec![format!("地区 {}", r)]),
                    _ => None,
                }
            });

        seen.push(key.clone());
        entries.push(Manga {
            key,
            title,
            cover,
            description: latest,
            tags,
            url: Some(format!("{}{}", get_base_url(), href)),
            viewer: Viewer::Webtoon,
            content_rating: ContentRating::NSFW,
            ..Default::default()
        });
    }

    Ok(entries)
}

// Decide whether the listing/search page has another page of results.
//
// The site renders pagination as a series of `<a href="…?page=N">N</a>`
// anchors followed by a `<a …>下一頁</a>` link, both inside the same
// pagination widget. We detect "has next" by scanning anchor `href`s for
// the next page number, falling back to the textual `下一頁` / `Next` label
// on pages that don't number their pagination (search results do this).
pub(crate) fn has_next_page_from_html(html: &str, current_page_0idx: i32) -> bool {
    let doc = match Html::parse(html) {
        Ok(d) => d,
        Err(_) => return false,
    };
    let needle = format!("page={}", current_page_0idx + 1);
    let Some(anchors) = doc.select("a") else {
        return false;
    };
    for a in anchors {
        if let Some(href) = a.attr("href") {
            if href.contains(&needle) {
                return true;
            }
        }
        if let Some(text) = a.text() {
            if text.contains("下一頁") || text.contains("Next") {
                return true;
            }
        }
    }
    false
}

pub(crate) fn parse_manga_listing(html: &str, current_page_0idx: i32) -> Result<MangaPageResult> {
    let entries = extract_manga_cards(html)?;
    let has_next_page = has_next_page_from_html(html, current_page_0idx);
    Ok(MangaPageResult {
        entries,
        has_next_page,
    })
}
