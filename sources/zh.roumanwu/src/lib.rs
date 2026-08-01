#![no_std]

use aidoku::imports::canvas::{Canvas, ImageRef, Rect};
use aidoku::prelude::*;
use aidoku::{
    AidokuError, BaseUrlProvider, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult,
    DynamicSettings, FilterValue, Home, HomeComponent, HomeComponentValue, HomeLayout, Link,
    LinkValue, Listing, ListingProvider, Manga, MangaPageResult, MangaStatus, Page, PageContent,
    Result, Source, Viewer,
    alloc::{String, Vec, format, string::ToString, vec},
    imports::{defaults::defaults_get, html::Html, net::Request},
};

mod utils;
use utils::*;

const BASE_URL: &str = "https://rouman5.com";

fn get_base_url() -> String {
    match defaults_get::<String>("base_url") {
        Some(url) if !url.trim().is_empty() => url,
        _ => String::from(BASE_URL),
    }
}

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

// ---------- HTTP helpers ----------

fn html_get_string(url: &str) -> Result<String> {
    Request::get(url)?
        .header("Use -Agent", USER_AGENT)
        .header("Accept-Language", "zh-TW,zh;q=0.9,en;q=0.8")
        .string()
}

// ---------- Misc helpers ----------

// Decide whether the listing/search page has another page of results.
fn has_next_page_from_html(html: &str, current_page_0idx: i32) -> bool {
    let needle = format!("page={}", current_page_0idx + 1);
    if html.contains(&needle) {
        return true;
    }
    html.contains("下一頁") || html.contains("Next")
}

// Extract text from a div with a specific CSS class
fn extract_div_text<'a>(block: &'a str, class: &str) -> Option<String> {
    let class_attr = format!("class=\"{}\"", class);
    let class_pos = block.find(&class_attr)?;
    let after_class = &block[class_pos + class_attr.len()..];
    let close_tag = after_class.find(">")?;
    let after_close = &after_class[close_tag + 1..];
    let end_tag = after_close.find("</div>")?;
    Some(after_close[..end_tag].trim().to_string())
}

// Extract the first cover image URL from a block
fn extract_first_cover(block: &str) -> Option<String> {
    let bg_marker = "background-image:url(&quot;";
    let bg_start = block.find(bg_marker)? + bg_marker.len();
    let bg_end = block[bg_start..].find("&quot;")?;
    Some(block[bg_start..bg_start + bg_end].to_string())
}

// ---------- Image Unscrambling ----------

fn unscramble_image_url(url: &str) -> bool {
    url.contains("sr:1")
}

fn unscramble_image(url: &str, image_data: &[u8]) -> Option<ImageRef> {
    let parts: Vec<&str> = url.split('/').collect();
    let last_part = parts.last()?;
    let base64_part = last_part.split('.').collect::<Vec<&str>>();
    let base64_str = &base64_part[..base64_part.len().saturating_sub(1)].join(".");

    let decoded = base64_decode(base64_str);
    let hash = md5_hash(&decoded);
    let last_byte = hash[15];
    let num_slices: i32 = (last_byte as i32 % 10) + 5;

    let src_image = ImageRef::new(image_data);
    let width = src_image.width();
    let height = src_image.height();
    if width <= 0.0 || height <= 0.0 {
        return None;
    }

    let mut canvas = Canvas::new(width, height);
    let slice_height = (height as i32 / num_slices) as f32;
    let height_offset = (height as i32 % num_slices) as f32;

    for l in 0..num_slices {
        let (src_y, dst_y, h) = if l == 0 {
            (
                height - slice_height - height_offset,
                0.0,
                slice_height + height_offset,
            )
        } else {
            (
                height - slice_height * (l as f32 + 1.0) - height_offset,
                slice_height * l as f32 + height_offset,
                slice_height,
            )
        };

        let src_rect = Rect {
            x: 0.0,
            y: src_y,
            width,
            height: h,
        };
        let dst_rect = Rect {
            x: 0.0,
            y: dst_y,
            width,
            height: h,
        };
        canvas.copy_image(&src_image, src_rect, dst_rect);
    }

    Some(canvas.get_image())
}

// ---------- Home parsing ----------

#[derive(Clone, Copy)]
enum HomeSectionKind {
    BigScroller,
    Scroller,
    MangaList { ranking: bool, page_size: i32 },
}

type SectionSpec = (
    &'static [&'static str],
    &'static [&'static str],
    HomeSectionKind,
);
const HOME_SECTIONS: &[SectionSpec] = &[
    (
        &["正熱門"],
        &["當下超高人氣作品"],
        HomeSectionKind::BigScroller,
    ),
    (
        &["今日最佳"],
        &["今日爆款"],
        HomeSectionKind::MangaList {
            ranking: true,
            page_size: 3,
        },
    ),
    (
        &["最近更新"],
        &["每日多次更新"],
        HomeSectionKind::MangaList {
            ranking: true,
            page_size: 3,
        },
    ),
    (
        &["本週熱門"],
        &["本週最熱漫畫"],
        HomeSectionKind::MangaList {
            ranking: true,
            page_size: 3,
        },
    ),
    (&["已完結"], &["完結精選"], HomeSectionKind::Scroller),
];

fn parse_home_layout(html: &str) -> Result<HomeLayout> {
    let mut components: Vec<HomeComponent> = Vec::new();
    for (titles, subtitles, kind) in HOME_SECTIONS {
        let mut found: Option<(usize, &str)> = None;
        for title in titles.iter() {
            let tag = format!(
                "<div class=\"text-2xl text-gray-900 dark:text-gray-100\">{}</div>",
                title
            );
            if let Some(i) = html.find(&tag) {
                found = Some((i, title));
                break;
            }
        }
        let (t_idx, used_title) = match found {
            Some(x) => x,
            None => continue,
        };
        let end_idx = HOME_SECTIONS
            .iter()
            .flat_map(|(other_titles, _, _)| {
                if other_titles == titles {
                    return vec![];
                }
                other_titles
                    .iter()
                    .filter_map(|t| {
                        let tag = format!(
                            "<div class=\"text-2xl text-gray-900 dark:text-gray-100\">{}</div>",
                            t
                        );
                        html.find(&tag)
                            .and_then(|i| if i > t_idx { Some(i) } else { None })
                    })
                    .collect::<Vec<_>>()
            })
            .min()
            .unwrap_or(html.len());
        let range = &html[t_idx..end_idx];
        let mut links: Vec<Link> = Vec::new();
        let mut mangas: Vec<Manga> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        let mut search = 0;
        let anchor = "<a href=\"/books/";
        while let Some(rel) = range[search..].find(anchor) {
            let abs = search + rel;
            if let Some(close_rel) = range[abs..].find("</a>") {
                let block = &range[abs..abs + close_rel + 4];
                let href_start: usize = 9;
                if block.len() < href_start + 2 {
                    search = abs + close_rel + 4;
                    continue;
                }
                let href_end = match block[href_start..].find(0x22 as char) {
                    Some(i) => href_start + i,
                    None => {
                        search = abs + close_rel + 4;
                        continue;
                    }
                };
                let href = block[href_start..href_end].to_string();
                if href.matches('/').count() != 2 {
                    search = abs + close_rel + 4;
                    continue;
                }

                let class1 = "truncate text-sm md:text-base text-foreground";
                let class2 = "line-clamp-2 h-10 text-sm";
                let a = extract_div_text(block, class1);
                let b = extract_div_text(block, class2);
                let card_title = a.or(b).unwrap_or_default().trim().to_string();

                if card_title.is_empty() {
                    search = abs + close_rel + 4;
                    continue;
                }
                let cover = extract_first_cover(block);
                let key = href.rsplit('/').next().unwrap_or("").to_string();
                if key.is_empty() || seen.contains(&key) {
                    search = abs + close_rel + 4;
                    continue;
                }
                seen.push(key.clone());
                let manga = Manga {
                    key,
                    title: card_title.clone(),
                    cover: cover.clone(),
                    url: Some(format!("{}{}", get_base_url(), href)),
                    viewer: Viewer::Webtoon,
                    content_rating: ContentRating::NSFW,
                    ..Default::default()
                };
                mangas.push(manga.clone());
                links.push(Link {
                    title: card_title,
                    subtitle: None,
                    image_url: cover,
                    value: Some(LinkValue::Manga(manga)),
                });
                search = abs + close_rel + 4;
            } else {
                break;
            }
        }
        let subtitle = subtitles.first().map(|s| String::from(*s));
        let value = match *kind {
            HomeSectionKind::BigScroller => HomeComponentValue::BigScroller {
                entries: mangas,
                auto_scroll_interval: None,
            },
            HomeSectionKind::Scroller => HomeComponentValue::Scroller {
                entries: links,
                listing: None,
            },
            HomeSectionKind::MangaList { ranking, page_size } => HomeComponentValue::MangaList {
                ranking,
                page_size: Some(page_size),
                entries: links,
                listing: None,
            },
        };
        components.push(HomeComponent {
            title: Some(String::from(used_title)),
            subtitle,
            value,
        });
    }
    Ok(HomeLayout { components })
}

// ---------- Listing / search parsing ----------

fn parse_manga_listing(html: &str, current_page_0idx: i32) -> Result<MangaPageResult> {
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

// ---------- Detail / chapter parsing ----------

// "第N話 ..." -> (N, "...") where 第 = U+7B2C, 話 = U+8A71
fn extract_chapter_number_and_title(s: &str) -> (f32, String) {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut number: Option<f32> = None;
    let mut title_start: usize = s.len();
    let mut title_trimmed: bool = false;

    while i < bytes.len() {
        if i + 2 < bytes.len() && bytes[i] == 0xE7 && bytes[i + 1] == 0xAC && bytes[i + 2] == 0xAC {
            let mut j = i + 3;
            let num_start = j;
            while j < bytes.len() && (bytes[j] as char).is_ascii_digit() {
                j += 1;
            }
            if j > num_start {
                let num_str: String = s[num_start..j].chars().collect();
                if let Ok(n) = num_str.parse::<f32>() {
                    number = Some(n);
                    if j + 2 < bytes.len()
                        && bytes[j] == 0xE8
                        && bytes[j + 1] == 0xA9
                        && bytes[j + 2] == 0xB1
                    {
                        j += 3;
                    }
                    title_start = j;
                    title_trimmed = true;
                }
            }
        }
        i += 1;
    }

    let mut title: String = s[title_start..].chars().take(200).collect();
    if title_trimmed {
        while let Some(first) = title.chars().next() {
            if matches!(first, ' ' | '-' | ':' | '：' | '~' | '_') {
                title = title[first.len_utf8()..].to_string();
            } else {
                break;
            }
        }
    }
    let title = title.trim().to_string();
    (number.unwrap_or(0.0), title)
}

fn parse_manga_detail(html: &str, key: &str) -> Result<Manga> {
    let json_ld_raw =
        slice_between(html, "<script type=\"application/ld+json\">", "</script>").unwrap_or("");
    let json_ld = json_ld_raw
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&#039;", "'");

    let title = json_string(&json_ld, "name").unwrap_or_default();
    let cover = json_string(&json_ld, "image");
    let description = json_string(&json_ld, "description");
    let author_str: Option<String> = json_string(&json_ld, "author").or_else(|| {
        if let Some(i) = json_ld.find("\"author\":{") {
            let rest = &json_ld[i + 10..];
            json_string(rest, "name")
        } else {
            None
        }
    });

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

        let block_start = href_start + end + 1;
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
        let (chapter_number, clean_title) = extract_chapter_number_and_title(raw_title);

        chapters.push(Chapter {
            key: index.to_string(),
            title: if clean_title.is_empty() {
                None
            } else {
                Some(clean_title)
            },
            chapter_number: Some(chapter_number),
            volume_number: None,
            date_uploaded: None,
            scanlators: None,
            url: Some(format!("/books/{}/{}", key, index)),
            language: Some("zh".to_string()),
            thumbnail: None,
            locked: false,
        });

        search_from = a_close + 4;
    }
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

// ---------- Chapter pages parsing ----------

fn parse_chapter_pages(html: &str) -> Result<(i32, Vec<String>)> {
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

    // Determine page count
    let mut page_count: i32 = {
        let json_ld_raw = slice_between(html, "<script type=\"application/ld+json\">", "</script>")
            .unwrap_or("")
            .replace("&quot;", "\"")
            .replace("&amp;", "&");
        let needle = "numberOfPages";
        let v = if let Some(i) = json_ld_raw.find(needle) {
            let after = &json_ld_raw[i + needle.len()..];
            let mut s = 0;
            while s < after.len() && (after.as_bytes()[s] == b':' || after.as_bytes()[s] == b' ') {
                s += 1;
            }
            let digits: String = after[s..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            digits.parse().unwrap_or(0)
        } else {
            0
        };
        v
    };
    if page_count == 0 {
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
                    page_count = n;
                    break;
                }
            }
            i = abs + 1;
        }
    }
    if page_count == 0 {
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
                    page_count = n;
                    break;
                }
            }
            i = abs + 1;
        }
    }

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

// ---------- Source impl ----------

pub struct Roumanwu;

impl Source for Roumanwu {
    fn new() -> Self {
        Self
    }

    fn get_search_manga_list(
        &self,
        query: Option<String>,
        page: i32,
        _filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        let sp = site_page(page);
        let url = match query.as_deref() {
            Some(q) if !q.trim().is_empty() => {
                format!(
                    "{}/search?term={}&page={}",
                    get_base_url(),
                    urlencode(q),
                    sp
                )
            }
            _ => format!("{}/books?page={}", get_base_url(), sp),
        };
        let html = html_get_string(&url)?;
        parse_manga_listing(&html, sp)
    }

    fn get_manga_update(
        &self,
        manga: Manga,
        _needs_details: bool,
        _needs_chapters: bool,
    ) -> Result<Manga> {
        let key = manga.key.clone();
        if key.is_empty() {
            return Err(AidokuError::message("missing manga key"));
        }
        let url = format!("{}/books/{}", get_base_url(), key);
        let html = html_get_string(&url)?;
        parse_manga_detail(&html, &key)
    }

    fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
        let path = chapter
            .url
            .clone()
            .unwrap_or_else(|| format!("/books/{}/{}", manga.key, chapter.key));
        let full = format!("{}{}", get_base_url(), path);
        let html = html_get_string(&full)?;
        let (page_count, mut urls) = parse_chapter_pages(&html)?;

        if page_count > 0 && urls.len() > page_count as usize {
            urls.truncate(page_count as usize);
        }

        Ok(urls
            .into_iter()
            .map(|u| {
                if unscramble_image_url(&u) {
                    let image_data = Request::get(&u)
                        .ok()
                        .and_then(|r| r.data().ok())
                        .unwrap_or_default();
                    if let Some(image) = unscramble_image(&u, &image_data) {
                        Page {
                            content: PageContent::image(image),
                            thumbnail: None,
                            has_description: false,
                            description: None,
                        }
                    } else {
                        Page {
                            content: PageContent::url(u),
                            thumbnail: None,
                            has_description: false,
                            description: None,
                        }
                    }
                } else {
                    Page {
                        content: PageContent::url(u),
                        thumbnail: None,
                        has_description: false,
                        description: None,
                    }
                }
            })
            .collect())
    }
}

impl ListingProvider for Roumanwu {
    fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
        let sp = site_page(page);
        let filter = match listing.id.as_str() {
            "ongoing" => "true",
            "completed" => "false",
            "all" | "default" => "",
            _ => "",
        };
        let url = format!("{}/books?continued={}&page={}", get_base_url(), filter, sp);
        let html = html_get_string(&url)?;
        parse_manga_listing(&html, sp)
    }
}

impl Home for Roumanwu {
    fn get_home(&self) -> Result<HomeLayout> {
        let html = html_get_string(&format!("{}/home", get_base_url()))?;
        parse_home_layout(&html)
    }
}

impl DeepLinkHandler for Roumanwu {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        let url = url.trim();
        let marker = "/books/";
        let Some(idx) = url.find(marker) else {
            return Ok(None);
        };
        let after = &url[idx + marker.len()..];
        let seg: String = after
            .chars()
            .take_while(|c| *c != '/' && *c != '?' && *c != '#')
            .collect();
        if seg.is_empty() {
            return Ok(None);
        }
        let rest_after_seg = &after[seg.len()..];
        if rest_after_seg.starts_with('/') {
            let chap_str: String = rest_after_seg[1..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(chapter) = chap_str.parse::<i32>() {
                return Ok(Some(DeepLinkResult::Chapter {
                    manga_key: seg.into(),
                    key: chapter.to_string().into(),
                }));
            }
        }
        Ok(Some(DeepLinkResult::Manga { key: seg.into() }))
    }
}

impl DynamicSettings for Roumanwu {
    fn get_dynamic_settings(&self) -> Result<Vec<aidoku::Setting>> {
        let current_url = get_base_url();
        Ok(vec![
            aidoku::TextSetting {
                key: "base_url".into(),
                title: "源地址".into(),
                placeholder: Some(BASE_URL.into()),
                default: Some(current_url.into()),
                ..Default::default()
            }
            .into(),
            aidoku::LinkSetting {
                key: "address_link".into(),
                title: "地址发布：https://rdz3.xyz/dizhi".into(),
                url: "https://rdz3.xyz/dizhi".into(),
                external: Some(true),
                ..Default::default()
            }
            .into(),
        ])
    }
}

impl BaseUrlProvider for Roumanwu {
    fn get_base_url(&self) -> Result<String> {
        Ok(get_base_url())
    }
}

#[cfg(test)]
mod test;

register_source!(
    Roumanwu,
    ListingProvider,
    Home,
    DeepLinkHandler,
    DynamicSettings,
    BaseUrlProvider
);
