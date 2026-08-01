//! Home page parsing.
//!
//! The rouman5.com home page is rendered server-side but uses Tailwind class
//! hashes that don't survive into stable CSS selectors. We locate each section
//! by its title string, slice the surrounding HTML, then extract the manga
//! cards inline.

use aidoku::alloc::string::ToString;
use aidoku::alloc::{String, Vec, format, vec};
use aidoku::{
    ContentRating, HomeComponent, HomeComponentValue, HomeLayout, Link, LinkValue, Manga, Result,
    Viewer,
};

use crate::source_url::get_base_url;

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

// Extract text from a div with a specific CSS class.
fn extract_div_text<'a>(block: &'a str, class: &str) -> Option<String> {
    let class_attr = format!("class=\"{}\"", class);
    let class_pos = block.find(&class_attr)?;
    let after_class = &block[class_pos + class_attr.len()..];
    let close_tag = after_class.find(">")?;
    let after_close = &after_class[close_tag + 1..];
    let end_tag = after_close.find("</div>")?;
    Some(after_close[..end_tag].trim().to_string())
}

// Extract the first cover image URL from a card block (background-image style).
fn extract_first_cover(block: &str) -> Option<String> {
    let bg_marker = "background-image:url(&quot;";
    let bg_start = block.find(bg_marker)? + bg_marker.len();
    let bg_end = block[bg_start..].find("&quot;")?;
    Some(block[bg_start..bg_start + bg_end].to_string())
}

pub(crate) fn parse_home_layout(html: &str) -> Result<HomeLayout> {
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
