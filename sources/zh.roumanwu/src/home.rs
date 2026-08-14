//! Home page parsing.
//!
//! The rouman5.com home page is rendered server-side as a vertical stack of
//! sections. Each section starts with a `text-2xl` title div (matching one
//! of the known titles in `HOME_SECTIONS`) and is followed — possibly after
//! an ad-slot div — by a `grid` div holding the section's manga anchors.
//!
//! Each section's title element lives inside the section's wrapper div; the
//! manga grid is the last child div of that wrapper. We locate the title
//! element with HTML selectors, walk up to the wrapper, then walk the wrapper
//! children in reverse to find the grid.

use aidoku::alloc::string::ToString;
use aidoku::alloc::{String, Vec};
use aidoku::imports::html::{Document, Element, Html};
use aidoku::{HomeComponent, HomeComponentValue, HomeLayout, Link, Result};

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
    let doc = Html::parse(html)?;
    let mut components: Vec<HomeComponent> = Vec::new();

    for (titles, subtitles, kind) in HOME_SECTIONS {
        let title_el = match find_title_element(&doc, titles) {
            Some(e) => e,
            None => continue,
        };
        let used_title = title_el
            .text()
            .map(|t| t.trim().to_string())
            .unwrap_or_default();

        let grid_html = match find_manga_grid_html(&title_el) {
            Some(s) => s,
            None => continue,
        };
        let mangas = match extract_manga_cards(&grid_html) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let links: Vec<Link> = mangas.iter().map(|m| Link::from(m.clone())).collect();

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
            title: Some(used_title),
            subtitle,
            value,
        });
    }
    Ok(HomeLayout { components })
}

// ---------- Helpers ----------

/// Find the first `div.text-2xl` whose trimmed text matches one of the
/// supplied title aliases. SwiftSoup's `:matches(REGEX)` would do this in a
/// single query, but anchoring on the class is more robust to future site
/// edits that add new sections with new titles.
fn find_title_element(doc: &Document, aliases: &[&str]) -> Option<Element> {
    let list = doc.select("div.text-2xl")?;
    for el in list {
        if let Some(t) = el.text() {
            let trimmed = t.trim();
            if aliases.iter().any(|a| *a == trimmed) {
                return Some(el);
            }
        }
    }
    None
}

/// Walk up from the title element to find the section wrapper (a div that
/// contains a `/books/...` anchor), then walk that wrapper's children in
/// reverse to locate the grid that holds the section's manga cards. Return
/// that grid's outer HTML so the shared card parser can consume it.
fn find_manga_grid_html(title_el: &Element) -> Option<String> {
    let wrapper = find_section_wrapper(title_el)?;
    let mut children = wrapper.children();
    while let Some(child) = children.next_back() {
        if child.select_first("a[href^=\"/books/\"]").is_some() {
            return child.outer_html();
        }
    }
    None
}

/// Walk up from `title_el` until we find a parent that contains a
/// `/books/...` anchor (i.e. the section wrapper that holds both the title
/// and the manga grid).
fn find_section_wrapper(title_el: &Element) -> Option<Element> {
    let mut current = title_el.parent()?;
    while current.select_first("a[href^=\"/books/\"]").is_none() {
        current = current.parent()?;
    }
    Some(current)
}
