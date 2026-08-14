//! Manga detail and chapter-list parsing.
//!
//! The detail page embeds a JSON-LD blob (`<script type="application/ld+json">`)
//! with the title, cover, author, description, and genre. Chapter list is then
//! scraped from the inline chapter anchors below the JSON-LD block.

use aidoku::alloc::string::ToString;
use aidoku::alloc::{String, Vec, format, vec};
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

    let mut chapters: Vec<Chapter> = Vec::new();
    let needle = format!("href=\"/books/{}/", key);
    let mut search_from = 0;
    while let Some(rel) = html[search_from..].find(&needle) {
        let abs = search_from + rel;
        let href_start = abs + needle.len();
        let rest = &html[href_start..];
        let mut end = 0;
        while end < rest.len() && rest.as_bytes()[end] != b'"' {
            end += 1;
        }
        if end == 0 {
            break;
        }
        let index_str = &rest[..end];
        let index: i32 = match index_str.parse() {
            Ok(n) => n,
            Err(_) => {
                search_from = href_start + end + 1;
                continue;
            }
        };

        let block_start = href_start + end + 2;
        let a_close = match html[block_start..].find("</a>") {
            Some(o) => block_start + o,
            None => break,
        };
        let block = &html[block_start..a_close];
        let div_open = block.find('>').map(|i| i + 1).unwrap_or(0);
        let div_close = block.rfind("</div>").unwrap_or(block.len());
        let raw_title: String = if div_open < div_close {
            block[div_open..div_close].chars().take(200).collect()
        } else {
            String::new()
        };
        let raw_title = raw_title.trim();
        let chapter_number = (index + 1) as f32;

        chapters.push(Chapter {
            key: index.to_string(),
            title: if raw_title.is_empty() {
                None
            } else {
                Some(raw_title.to_string())
            },
            chapter_number: Some(chapter_number),
            volume_number: None,
            date_uploaded: None,
            scanlators: None,
            url: Some(format!("{}/books/{}/{}", get_base_url(), key, index)),
            language: Some("zh".to_string()),
            thumbnail: None,
            locked: false,
        });

        search_from = a_close + 4;
    }

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
