//! Home page parsing.
//!
//! The rouman5.com home page is rendered server-side but uses Tailwind class
//! hashes that don't survive into stable CSS selectors. We locate each section
//! by its title string, slice the surrounding HTML, then hand the section
//! slice to [`crate::listing::extract_manga_cards`] for DOM-based card
//! extraction — the same parser the listing and search pages use.

use aidoku::alloc::{String, Vec, format, vec};
use aidoku::{HomeComponent, HomeComponentValue, HomeLayout, Link, LinkValue, Result};

use crate::listing::extract_manga_cards;

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

        // Shared DOM parser produces Manga entries; the BigScroller arm
        // needs them directly, while Scroller/MangaList wrap each one in a
        // Link so the UI shows titles + cover thumbnails.
        let mangas: Vec<aidoku::Manga> = extract_manga_cards(range)?;
        let links: Vec<Link> = mangas
            .iter()
            .map(|m| Link {
                title: m.title.clone(),
                subtitle: None,
                image_url: m.cover.clone(),
                value: Some(LinkValue::Manga(m.clone())),
            })
            .collect();

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
