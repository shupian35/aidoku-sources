//! Manga detail and chapter-list parsing.
//!
//! The detail page embeds a JSON-LD blob (`<script type="application/ld+json">`)
//! with the title, cover, author, description, and genre. Chapter list is then
//! scraped from the inline chapter anchors below the JSON-LD block.

use aidoku::alloc::string::ToString;
use aidoku::alloc::{String, Vec, format, vec};
use aidoku::imports::html::Html;
use aidoku::{Chapter, ContentRating, Manga, MangaStatus, Result, Viewer};

use crate::source_url::get_base_url;
use crate::utils::{json_top_level_object_field, json_top_level_string, slice_between};

pub(crate) fn parse_manga_detail(html: &str, key: &str) -> Result<Manga> {
    let json_ld_raw =
        slice_between(html, "<script type=\"application/ld+json\">", "</script>").unwrap_or("");
    let json_ld = json_ld_raw
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&#039;", "'");

    let title = json_top_level_string(&json_ld, "name").unwrap_or_default();
    let cover = json_top_level_string(&json_ld, "image");
    let description = json_top_level_string(&json_ld, "description");
    let author_str: Option<String> = json_top_level_string(&json_ld, "author")
        .or_else(|| json_top_level_object_field(&json_ld, "author", "name"));

    let mut tags: Vec<String> = Vec::new();
    if let Some(arr_start) = json_ld.find("\"genre\":[") {
        let arr_rest = &json_ld[arr_start + 9..];
        if let Some(arr_end) = arr_rest.find(']') {
            let arr_body = &arr_rest[..arr_end];
            for piece in arr_body.split(',') {
                let p = piece.trim();
                if p.starts_with('"') && p.ends_with('"') && p.len() >= 2 {
                    tags.push(p[1..p.len() - 1].to_string());
                }
            }
        }
    }

    let mut status = MangaStatus::Unknown;
    if let Some(idx) = html.find("狀態:") {
        let window = &html[idx..html.len().min(idx + 1200)];
        if let Some(open) = window.find("<span class=\"text-foreground\">") {
            let mut content_start = open + 30;
            while content_start < window.len() && !window.is_char_boundary(content_start) {
                content_start += 1;
            }
            let after = &window[content_start..];
            if let Some(close_rel) = after.find("</span>") {
                let val = &after[..close_rel].trim();
                if val.contains("連載中") {
                    status = MangaStatus::Ongoing;
                } else if val.contains("完結") {
                    status = MangaStatus::Completed;
                } else if val.contains("休刊") || val.contains("停刊") {
                    status = MangaStatus::Hiatus;
                }
            }
        }
    }

    // Chapters live inside the site's chapter grid; selecting the grid by
    // its exact class excludes the "開始閱讀" button and related-manga links
    // that appear outside it, so no extra filtering is needed.
    let mut chapters: Vec<Chapter> = {
        let doc = Html::parse(html)?;
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
    };
    // Newest first: the site grid is oldest-first, readers expect the
    // latest chapter at the top.
    chapters.reverse();

    let url = Some(format!("{}/books/{}", get_base_url(), key));
    Ok(Manga {
        key: key.to_string(),
        title,
        cover,
        artists: None,
        authors: match author_str {
            Some(a) if !a.is_empty() => Some(vec![a]),
            _ => None,
        },
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
