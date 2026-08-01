use aidoku::alloc::{String, Vec};
use aidoku::{
    ContentRating, DeepLinkHandler, DeepLinkResult, Home, HomeComponentValue, Link, LinkValue,
    Listing, ListingProvider, Manga, MangaPageResult, MangaStatus, Page, PageContent, Source,
    Viewer,
};
use aidoku_test::aidoku_test;

use super::Roumanwu;

fn new_source() -> Roumanwu {
    <Roumanwu as Source>::new()
}

#[aidoku_test]
fn debug_home_parse() {
    let s = new_source();
    let layout = s.get_home().expect("get home should succeed");
    let _ = aidoku::prelude::println!("debug_home_parse: got {} sections", layout.components.len());
}

#[aidoku_test]
fn debug_home_response() {
    use aidoku::imports::net::Request;
    use aidoku::prelude::println;
    let raw = Request::get("https://rouman5.com/home")
        .expect("send")
        .string()
        .expect("string");
    let _ = println!("=== HOME RAW len={} ===", raw.len());
    let marker = String::from("<div class=\"text-2xl text-gray-900 dark:text-gray-100\">");
    let mut i = 0;
    while let Some(rel) = raw[i..].find(&marker) {
        let abs = i + rel;
        let after = &raw[abs + marker.len()..];
        let end = after.find("</div>").unwrap_or(40);
        let title = &after[..end.min(40)];
        let _ = println!("  section at {}: {:?}", abs, title);
        i = abs + 1;
    }
    let _ = println!("=== END ===");
}

#[aidoku_test]
fn home_link_manga_resolves_via_get_manga_update() {
    let s = new_source();
    let layout = s.get_home().expect("get home should succeed");
    let mut checked = 0;
    for comp in &layout.components {
        let HomeComponentValue::MangaList { entries, .. } = &comp.value else {
            continue;
        };
        let Some(first) = entries.first() else {
            continue;
        };
        let Link {
            value: Some(LinkValue::Manga(manga)),
            ..
        } = first
        else {
            panic!("section {:?} first link is not a Manga", comp.title);
        };
        assert!(!manga.key.is_empty(), "manga key should not be empty");
        let updated: Manga = s
            .get_manga_update(manga.clone(), true, true)
            .expect("get manga update should succeed");
        assert!(!updated.title.is_empty(), "title should be filled in");
        assert!(updated.chapters.is_some(), "chapters should be present");
        let chs = updated.chapters.as_deref().unwrap();
        assert!(
            !chs.is_empty(),
            "chapter list should not be empty (manga={})",
            updated.key
        );
        checked += 1;
    }
    assert!(checked >= 3, "checked at least 3 sections, got {checked}");
}

#[aidoku_test]
fn get_manga_update_for_known_manga() {
    let s = new_source();
    let manga = Manga {
        key: String::from("cm4sx1zpa000avnl0ziqnbfy5"),
        ..Default::default()
    };
    let updated = s
        .get_manga_update(manga, true, false)
        .expect("get manga update should succeed");
    assert!(!updated.title.is_empty(), "title should be present");
    assert!(updated.cover.is_some(), "cover should be present");
    assert_eq!(updated.viewer, Viewer::Webtoon);
    assert_eq!(updated.content_rating, ContentRating::NSFW);
    assert!(
        updated.description.is_some(),
        "description should be present"
    );
    assert_eq!(updated.status, MangaStatus::Ongoing, "should be ongoing");
}

#[aidoku_test]
fn get_manga_update_returns_chapter_list() {
    let s = new_source();
    let manga = Manga {
        key: String::from("cm4sx1zpa000avnl0ziqnbfy5"),
        ..Default::default()
    };
    let updated = s
        .get_manga_update(manga, false, true)
        .expect("get manga update should succeed");
    let chs = updated.chapters.as_deref().expect("chapters");
    assert!(
        chs.len() >= 10,
        "should have many chapters, got {}",
        chs.len()
    );
    for c in chs.iter().take(3) {
        assert!(!c.key.is_empty(), "chapter key should be set");
        assert!(c.url.is_some(), "chapter url should be set");
        assert!(c.chapter_number.is_some(), "chapter number should be set");
    }
}

#[aidoku_test]
fn get_page_list_returns_many_pages() {
    let s = new_source();
    let manga = Manga {
        key: String::from("cm4sx1zpa000avnl0ziqnbfy5"),
        ..Default::default()
    };
    let updated = s
        .get_manga_update(manga.clone(), false, true)
        .expect("get manga update");
    let chs = updated.chapters.as_deref().expect("chapters");
    let first = chs.iter().find(|c| c.key == "0").expect("chapter 0");
    let pages: Vec<Page> = s
        .get_page_list(manga, first.clone())
        .expect("get page list");
    assert!(!pages.is_empty(), "pages should be non-empty");
    assert!(
        pages.len() >= 50,
        "should have many pages, got {}",
        pages.len()
    );
    for (i, p) in pages.iter().enumerate().take(3) {
        match &p.content {
            PageContent::Url(u, _) => assert!(u.contains("r5.rmcdn"), "page {i} url = {u}"),
            PageContent::Image(_) => {} // Image was unscrambled successfully
            _ => panic!("page {i} is not a Url or Image"),
        }
    }
}

#[aidoku_test]
fn get_page_list_returns_lazy_urls_with_scramble_context() {
    // Regression: get_page_list used to download + unscramble every sr:1
    // image before returning, blocking chapter open for tens of seconds.
    // Now every page is returned as a URL; scrambled URLs carry a
    // {"scramble":"1"} PageContext so process_page_image can decode them
    // lazily as the app loads each image.
    let s = new_source();
    let manga = Manga {
        key: String::from("cm4sx1zpa000avnl0ziqnbfy5"),
        ..Default::default()
    };
    let updated = s
        .get_manga_update(manga.clone(), false, true)
        .expect("get manga update");
    let chs = updated.chapters.as_deref().expect("chapters");
    let first = chs.first().expect("at least one chapter");
    let pages: Vec<Page> = s
        .get_page_list(manga, first.clone())
        .expect("get page list");
    assert!(
        pages.len() >= 50,
        "should have many pages, got {}",
        pages.len()
    );

    let mut url_count = 0;
    let mut scramble_tagged = 0;
    let mut untagged_rmcdn = 0;
    for p in &pages {
        match &p.content {
            PageContent::Url(url, ctx) => {
                url_count += 1;
                assert!(url.contains("r5.rmcdn"), "unexpected url: {url}");
                let tagged = ctx
                    .as_ref()
                    .and_then(|c| c.get("scramble"))
                    .is_some_and(|v| v == "1");
                if url.contains("sr:1") {
                    assert!(tagged, "sr:1 URL must carry scramble context: {url}");
                    scramble_tagged += 1;
                } else {
                    assert!(
                        !tagged,
                        "non-sr:1 URL must not carry scramble context: {url}"
                    );
                    untagged_rmcdn += 1;
                }
            }
            other => panic!("page content must be a URL, got {other:?}"),
        }
    }
    // Rouman5 mixes sr:0 and sr:1 across a chapter; assert both flavors
    // are actually present so the test exercises both branches.
    assert!(
        scramble_tagged >= 10,
        "expected many sr:1 pages, got {scramble_tagged}"
    );
    assert!(
        untagged_rmcdn >= 10,
        "expected many sr:0 pages, got {untagged_rmcdn}"
    );
    assert_eq!(
        url_count,
        pages.len(),
        "every page should be a URL, no pre-decoded images"
    );
}

fn listing_provider_default_listing() {
    let s = new_source();
    let listing = Listing {
        id: String::from("default"),
        name: String::from("Default"),
        kind: Default::default(),
    };
    let res: MangaPageResult = s.get_manga_list(listing, 1).expect("get manga list");
    assert!(!res.entries.is_empty(), "page 1 should not be empty");
    assert!(res.has_next_page, "should have next page");
    for m in res.entries.iter().take(3) {
        assert!(!m.key.is_empty());
        assert!(!m.title.is_empty());
        assert_eq!(m.viewer, Viewer::Webtoon);
    }
}

#[aidoku_test]
fn search_finds_known_manga() {
    let s = new_source();
    let res = s
        .get_search_manga_list(Some(String::from("娣卞堡")), 1, Vec::new())
        .expect("search should succeed");
    assert!(!res.entries.is_empty(), "search should return results");
}

#[aidoku_test]
fn chapter_list_first_entry_is_real_chapter_not_cta() {
    // Regression: the detail page links to `/books/<key>/0` for the
    // "開始閱讀" (start reading) CTA above the chapter grid. The parser used
    // to pick it up as a phantom "chapter 0", which then surfaced as the
    // first entry in the chapter list. The real chapter list starts with
    // "第1話" inside the chapter grid container.
    let s = new_source();
    let manga = Manga {
        key: String::from("cm4sx1zpa000avnl0ziqnbfy5"),
        ..Default::default()
    };
    let updated = s
        .get_manga_update(manga, false, true)
        .expect("get manga update should succeed");
    let chs = updated.chapters.as_deref().expect("chapters");
    assert!(
        chs.len() >= 10,
        "should have many chapters, got {}",
        chs.len()
    );
    for c in chs.iter() {
        let title = c.title.as_deref().unwrap_or("");
        assert_ne!(
            title, "放入書架",
            "phantom bookmark CTA should not appear as a chapter"
        );
        assert_ne!(
            title, "開始閱讀",
            "phantom start-reading CTA should not appear as a chapter"
        );
        let n = c.chapter_number.unwrap_or(0.0);
        assert!(
            n >= 1.0,
            "no chapter should have chapter_number 0.0 (would sort to the \
             top of the chapter list); got {} for key={} title={:?}",
            n,
            c.key,
            c.title
        );
    }
}

#[aidoku_test]
fn deep_link_dispatch() {
    let s = new_source();
    let r = s
        .handle_deep_link(String::from(
            "https://rouman5.com/books/cm4sx1zpa000avnl0ziqnbfy5",
        ))
        .expect("deep link");
    match r {
        Some(DeepLinkResult::Manga { key }) => {
            assert_eq!(key.as_str(), "cm4sx1zpa000avnl0ziqnbfy5")
        }
        other => panic!("expected Manga, got {other:?}"),
    }
    let r = s
        .handle_deep_link(String::from(
            "https://rouman5.com/books/cm4sx1zpa000avnl0ziqnbfy5/0",
        ))
        .expect("deep link");
    match r {
        Some(DeepLinkResult::Chapter { manga_key, key }) => {
            assert_eq!(manga_key.as_str(), "cm4sx1zpa000avnl0ziqnbfy5");
            assert_eq!(key.as_str(), "0");
        }
        other => panic!("expected Chapter, got {other:?}"),
    }
    let r = s
        .handle_deep_link(String::from("https://example.com/foo"))
        .expect("deep link");
    assert!(r.is_none(), "unknown URL should return None, got {r:?}");
}
