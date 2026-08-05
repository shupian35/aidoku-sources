//! Listing / search page parsing.
//!
//! The site's listing, search, and home-page sections all render manga cards
//! with the same DOM. `extract_manga_cards` parses that shared shape and is
//! reused by both `parse_manga_listing` (here) and `parse_home_layout` in
//! `home.rs`. Pagination is inferred from the presence of a `page=N+1` link
//! or a "下一頁" / "Next" marker; that marker only matters for listing pages.

use aidoku::alloc::string::ToString;
use aidoku::alloc::{String, Vec, format};
use aidoku::imports::html::Html;
use aidoku::{ContentRating, Manga, MangaPageResult, Result, Viewer};

use crate::source_url::get_base_url;
use crate::utils::extract_url_from_style;

/// Extract every manga card from a chunk of HTML.
///
/// Skips anchors whose `href` doesn't look like a manga path (chapter
/// anchors carry three `/` segments), dedupes by `key`, and ignores entries
/// with no resolvable title. Both listing pages and home-page sections call
/// this with their respective HTML slices.
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
            .select_first("div.truncate.text-foreground, div.line-clamp-2")
            .or_else(|| a.select_first("div[class*=\"text-foreground\"]"))
            .and_then(|e| e.text())
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        let Some(title) = title else { continue };
        let cover = a
            .select_first("div[style*=\"background-image\"]")
            .and_then(|d| d.attr("style"))
            .and_then(|s| extract_url_from_style(&s));

        seen.push(key.clone());
        entries.push(Manga {
            key,
            title,
            cover,
            url: Some(format!("{}{}", get_base_url(), href)),
            viewer: Viewer::Webtoon,
            content_rating: ContentRating::NSFW,
            ..Default::default()
        });
    }

    Ok(entries)
}

// Decide whether the listing/search page has another page of results.
fn has_next_page_from_html(html: &str, current_page_0idx: i32) -> bool {
    let needle = format!("page={}", current_page_0idx + 1);
    if html.contains(&needle) {
        return true;
    }
    html.contains("下一頁") || html.contains("Next")
}

pub(crate) fn parse_manga_listing(html: &str, current_page_0idx: i32) -> Result<MangaPageResult> {
    let entries = extract_manga_cards(html)?;
    let has_next_page = has_next_page_from_html(html, current_page_0idx);
    Ok(MangaPageResult {
        entries,
        has_next_page,
    })
}
