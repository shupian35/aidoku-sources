//! Listing / search page parsing.
//!
//! Listing and search share an identical card layout, so a single parser
//! feeds both the `ListingProvider::get_manga_list` and
//! `Source::get_search_manga_list` impls. Pagination is inferred from the
//! presence of a `page=N+1` link or a "下一頁" / "Next" marker.

use aidoku::alloc::string::ToString;
use aidoku::alloc::{String, Vec, format};
use aidoku::imports::html::Html;
use aidoku::{AidokuError, ContentRating, Manga, MangaPageResult, Result, Viewer};

use crate::source_url::get_base_url;
use crate::utils::extract_url_from_style;

// Decide whether the listing/search page has another page of results.
fn has_next_page_from_html(html: &str, current_page_0idx: i32) -> bool {
    let needle = format!("page={}", current_page_0idx + 1);
    if html.contains(&needle) {
        return true;
    }
    html.contains("下一頁") || html.contains("Next")
}

pub(crate) fn parse_manga_listing(html: &str, current_page_0idx: i32) -> Result<MangaPageResult> {
    let doc = Html::parse(html)?;
    let anchors = doc
        .select("a[href^=\"/books/\"]")
        .ok_or_else(|| AidokuError::message("no anchors found"))?;

    let mut entries: Vec<Manga> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    for a in anchors {
        let href = match a.attr("href") {
            Some(h) => h,
            None => continue,
        };
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
        let title_el = a
            .select_first("div.truncate.text-foreground, div.line-clamp-2")
            .or_else(|| a.select_first("div[class*=\"text-foreground\"]"));
        let title = match title_el.and_then(|e| e.text()) {
            Some(t) => t.trim().to_string(),
            None => continue,
        };
        if title.is_empty() {
            continue;
        }
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

    let has_next_page = has_next_page_from_html(html, current_page_0idx);
    Ok(MangaPageResult {
        entries,
        has_next_page,
    })
}
