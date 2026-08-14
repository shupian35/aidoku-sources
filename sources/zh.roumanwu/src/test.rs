use aidoku::alloc::{String, Vec, format, vec};
use aidoku::{
    ContentRating, DeepLinkHandler, DeepLinkResult, Home, HomeComponentValue, Link, LinkValue,
    Listing, ListingProvider, Manga, MangaPageResult, MangaStatus, Page, PageContent, Source,
    Viewer,
};
use aidoku_test::aidoku_test;

use super::Roumanwu;
use crate::chapter::{build_pages, page_count, resolve_chapter_url, truncate_to_page_count};
use crate::image::{scramble_slices, unscramble_image_url};
use crate::listing::extract_manga_cards;
use crate::utils::{json_top_level_object_field, json_top_level_string};

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
fn resolve_chapter_url_passes_absolute_through() {
    let url = resolve_chapter_url("https://other.example/books/x/1", "https://rouman5.com");
    assert_eq!(url, "https://other.example/books/x/1");
}

#[aidoku_test]
fn resolve_chapter_url_prepends_base_for_relative_path() {
    let url = resolve_chapter_url("/books/x/1", "https://rouman5.com");
    assert_eq!(url, "https://rouman5.com/books/x/1");
}

#[aidoku_test]
fn truncate_to_page_count_caps_at_page_count() {
    let urls: Vec<String> = (0..20).map(|i| format!("https://x/{i}")).collect();
    let out = truncate_to_page_count(urls, 5);
    assert_eq!(out.len(), 5);
}

#[aidoku_test]
fn truncate_to_page_count_is_noop_when_zero_or_too_small() {
    let urls: Vec<String> = (0..3).map(|i| format!("https://x/{i}")).collect();
    assert_eq!(truncate_to_page_count(urls.clone(), 0).len(), 3);
    assert_eq!(truncate_to_page_count(urls, 99).len(), 3);
}

#[aidoku_test]
fn build_pages_marks_scrambled_urls_with_context() {
    let tagged = vec![
        (String::from("https://x/a"), false),
        (String::from("https://x/b-sr:1"), true),
    ];
    let pages = build_pages(tagged);
    assert_eq!(pages.len(), 2);
    match &pages[0].content {
        PageContent::Url(url, ctx) => {
            assert_eq!(url, "https://x/a");
            assert!(ctx.is_none(), "non-scrambled URL must have no context");
        }
        other => panic!("expected Url, got {other:?}"),
    }
    match &pages[1].content {
        PageContent::Url(url, ctx) => {
            assert_eq!(url, "https://x/b-sr:1");
            let ctx = ctx.as_ref().expect("scrambled URL must carry context");
            assert_eq!(ctx.get("scramble").map(String::as_str), Some("1"));
        }
        other => panic!("expected Url, got {other:?}"),
    }
}

#[aidoku_test]
fn build_pages_handles_empty_input() {
    let pages = build_pages(Vec::new());
    assert!(pages.is_empty());
}

#[aidoku_test]
fn scramble_slices_derives_count_from_url() {
    // Pure function over the URL — no network. The base64 final segment is
    // an S3 key (`s3://rouman/images/...`); its MD5's last byte (0x35 = 53)
    // yields 53 % 10 + 5 = 8 slices.
    let scrambled = "https://r5.rmcdn10.xyz/m/bWYO6lLCSUgUlH3ZcUEfkwofo1hVCAx9RUc_bw0PnRU/wm:0/sr:1/czM6Ly9yb3VtYW4vaW1hZ2VzL2NtNHN4MXpwYTAwMGF2bmwwemlxbmJmeTUvZnJlZXgvNDQ1NDUvMjY5NjI4MS5qcGc.jpg";
    assert_eq!(scramble_slices(scrambled), Some(8));
    assert!(
        unscramble_image_url(scrambled),
        "sr:1 URL must be marked scrambled"
    );

    let plain = "https://r5.rmcdn11.xyz/m/uUzbUTZLfXg1oylH22QyIByCcolLMdPtncHSkLuSMMs/wm:2/sr:0/czM6Ly9yb3VtYW4vaW1hZ2VzL2NtNHN4MXpwYTAwMGF2bmwwemlxbmJmeTUvZnJlZXgvNDQ1NDUvMjY5NjI4Mi5qcGc.jpg";
    assert_eq!(scramble_slices(plain).is_some(), true);
    assert!(
        !unscramble_image_url(plain),
        "sr:0 URL must not be marked scrambled"
    );
}

#[aidoku_test]
fn get_page_list_filters_corrupted_urls() {
    // Regression: Next.js chunks the RSC payload by byte size and can cut a
    // URL's JSON string mid-way across chunks, leaving empty/truncated
    // imageUrl values. Those must be dropped, not surfaced as pages — a bad
    // page URL makes the host abort the whole chapter with "load failed".
    let s = new_source();
    let manga = Manga {
        key: String::from("3d449abf-d024-4c6b-b0c3-f1fd8e6b6f04"),
        ..Default::default()
    };
    let updated = s
        .get_manga_update(manga.clone(), false, true)
        .expect("get manga update should succeed");
    let chs = updated.chapters.as_deref().expect("chapters");
    let first = chs.iter().find(|c| c.key == "0").expect("chapter 0");
    let pages: Vec<Page> = s
        .get_page_list(manga, first.clone())
        .expect("get page list");
    assert!(
        pages.len() >= 50,
        "should have many pages, got {}",
        pages.len()
    );
    for (i, p) in pages.iter().enumerate() {
        match &p.content {
            PageContent::Url(url, _) => {
                assert!(
                    url.starts_with("http://") || url.starts_with("https://"),
                    "page {i} has invalid url: {url:?}"
                );
            }
            other => panic!("page {i} is not a Url: {other:?}"),
        }
    }
}

#[aidoku_test]
fn chapter_url_is_absolute() {
    // Regression: the chapter detail page renders an "open in browser"
    // button. Aidoku dispatches that using the chapter's `url` field, which
    // must be an absolute URL. The previous code stored only the relative
    // path (`/books/<key>/<idx>`), so the button silently did nothing.
    // The pure-function unit tests above pin the `resolve_chapter_url`
    // contract; this e2e confirms `detail::parse_manga_detail` actually
    // calls it with an absolute URL.
    let s = new_source();
    let manga = Manga {
        key: String::from("cm4sx1zpa000avnl0ziqnbfy5"),
        ..Default::default()
    };
    let updated = s
        .get_manga_update(manga, false, true)
        .expect("get manga update should succeed");
    let chs = updated.chapters.as_deref().expect("chapters");
    let c = chs.first().expect("at least one chapter");
    let url = c.url.as_deref().expect("chapter url set");
    assert!(
        url.starts_with("http://") || url.starts_with("https://"),
        "chapter url must be absolute, got {url}"
    );
    assert!(
        url.contains("/books/"),
        "chapter url must point at the chapter, got {url}"
    );
}

#[aidoku_test]
fn page_count_resolves_json_ld_value() {
    let html = r#"<script type="application/ld+json">{"numberOfPages":42}</script>"#;
    assert_eq!(page_count(html, ""), Some(42));
}

#[aidoku_test]
fn page_count_resolves_html_comment_split() {
    // <!-- -->N<!-- -->/<!-- -->(junk)<!-- -->頁  ← site splits digits across HTML comments
    let html = "<!-- -->7<!-- -->/<!-- -->more<!-- -->頁 trailing";
    assert_eq!(page_count(html, ""), Some(7));
}

#[aidoku_test]
fn page_count_resolves_rsc_payload_marker() {
    // Rouman5 RSC payload: "/",  N  ,"頁"  — the digits are caught between the two strings.
    let payload = r#"random"/","12","頁"trailing"#;
    assert_eq!(page_count("", payload), Some(12));
}

#[aidoku_test]
fn page_count_returns_none_when_all_heuristics_fail() {
    assert_eq!(page_count("", ""), None);
    assert_eq!(
        page_count("<html>nothing useful</html>", "no markers"),
        None
    );
}

#[aidoku_test]
fn page_count_prefers_json_ld_over_other_heuristics() {
    // All three heuristics could match; JSON-LD wins because it is first in the chain.
    let html = r#"
        <script type="application/ld+json">{"numberOfPages":1}</script>
        <!-- -->999<!-- -->/<!-- -->more<!-- -->頁
    "#;
    let payload = r#""/","9","頁""#;
    assert_eq!(page_count(html, payload), Some(1));
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
fn chapter_list_includes_numberless_chapters() {
    // Regression: chapters whose title has no 第N話 number (e.g. "最終話",
    // "後記") used to be dropped by a chapter_number == 0.0 filter. They must
    // be kept (with their original title) and sort after numbered chapters.
    let s = new_source();
    let manga = Manga {
        key: String::from("cm9uutbj9000gs63l0po5ihd0"),
        ..Default::default()
    };
    let updated = s
        .get_manga_update(manga, false, true)
        .expect("get manga update should succeed");
    let chs = updated.chapters.as_deref().expect("chapters");
    let titles: Vec<String> = chs
        .iter()
        .map(|c| c.title.clone().unwrap_or_default())
        .collect();
    assert!(
        titles.iter().any(|t| t.contains("最終話")),
        "最終話 chapter should be present, got {titles:?}"
    );
    assert!(
        titles.iter().any(|t| t == "後記"),
        "後記 chapter should be present, got {titles:?}"
    );
    for c in chs.iter() {
        let n = c.chapter_number.unwrap_or(0.0);
        assert!(
            n >= 1.0,
            "chapter_number must be >= 1.0, got {n} for title={:?}",
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

#[aidoku_test]
fn extract_manga_cards_returns_empty_for_no_anchors() {
    let cards = extract_manga_cards("<html>no manga links here</html>").expect("parse");
    assert!(cards.is_empty());
}

#[aidoku_test]
fn extract_manga_cards_returns_manga_for_valid_anchor() {
    let html = r#"<a href="/books/abc123"><div class="truncate text-sm md:text-base text-foreground">Title A</div><div style="background-image:url(&quot;https://x/c.jpg&quot;)"></div></a>"#;
    let cards = extract_manga_cards(html).expect("parse");
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].key, "abc123");
    assert_eq!(cards[0].title, "Title A");
    assert_eq!(cards[0].cover.as_deref(), Some("https://x/c.jpg"));
}

#[aidoku_test]
fn extract_manga_cards_skips_chapter_anchors() {
    // /books/{id}/{N} has 3 slashes — chapter anchors, not manga list entries.
    let html = r#"<a href="/books/abc/1"><div class="truncate text-foreground">chap</div></a><a href="/books/xyz"><div class="truncate text-foreground">manga</div></a>"#;
    let cards = extract_manga_cards(html).expect("parse");
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].key, "xyz");
}

#[aidoku_test]
fn extract_manga_cards_dedupes_repeated_keys() {
    let html = r#"
        <a href="/books/dup"><div class="truncate text-foreground">first</div></a>
        <a href="/books/dup"><div class="truncate text-foreground">second</div></a>
    "#;
    let cards = extract_manga_cards(html).expect("parse");
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].title, "first", "first occurrence wins");
}

#[aidoku_test]
fn extract_manga_cards_skips_empty_titles() {
    let html = r#"<a href="/books/a"><div class="truncate text-foreground">   </div></a><a href="/books/b"><div class="truncate text-foreground">real</div></a>"#;
    let cards = extract_manga_cards(html).expect("parse");
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].key, "b");
}

#[aidoku_test]
fn extract_manga_cards_parses_latest_chapter() {
    // The "至: <!-- -->第N話-..." line under the title becomes the manga's
    // description, so home/listing cards can show what the manga updated to.
    let html = r#"<a href="/books/abc123"><div class="truncate text-sm md:text-base text-foreground">Title A</div><div class="text-muted-foreground text-xs">至: <!-- -->第5話-測試標題</div><div style="background-image:url(&quot;https://x/c.jpg&quot;)"></div></a>"#;
    let cards = extract_manga_cards(html).expect("parse");
    assert_eq!(cards.len(), 1);
    assert!(
        cards[0]
            .description
            .as_deref()
            .unwrap_or("")
            .contains("第5話"),
        "description should contain the latest chapter, got {:?}",
        cards[0].description
    );
}

#[aidoku_test]
fn extract_manga_cards_parses_stats_tags() {
    // The stats row (views / favorites / last-updated) becomes tags so cards
    // can surface them. Search pages only carry the date, so fewer than three
    // stats leaves tags unset.
    let html = r#"<a href="/books/abc123"><div class="truncate text-sm md:text-base text-foreground">Title A</div><div class="text-muted-foreground text-xs">至: <!-- -->第5話-測試標題</div><div class="text-xs text-muted-foreground"><div>862.4K</div></div><div class="text-xs text-muted-foreground"><div>5.1K</div></div><div class="text-xs text-muted-foreground"><div>8/12/2026</div></div><div style="background-image:url(&quot;https://x/c.jpg&quot;)"></div></a>"#;
    let cards = extract_manga_cards(html).expect("parse");
    assert_eq!(cards.len(), 1);
    let tags = cards[0].tags.as_deref().expect("tags");
    assert_eq!(tags.len(), 3);
    assert_eq!(tags[0], "浏览 862.4K");
    assert_eq!(tags[1], "收藏 5.1K");
    assert_eq!(tags[2], "更新 8/12/2026");
}

#[aidoku_test]
fn json_top_level_string_returns_value() {
    let json = r#"{"name":"Hajime no Ippo","other":42}"#;
    assert_eq!(
        json_top_level_string(json, "name").as_deref(),
        Some("Hajime no Ippo")
    );
}

#[aidoku_test]
fn json_top_level_object_field_returns_nested_value() {
    let json = r#"{"author":{"name":"George Morikawa","@type":"Person"}}"#;
    assert_eq!(
        json_top_level_object_field(json, "author", "name").as_deref(),
        Some("George Morikawa")
    );
}

#[aidoku_test]
fn json_top_level_object_field_returns_none_when_parent_is_string() {
    let json = r#"{"author":"anonymous"}"#;
    assert_eq!(json_top_level_object_field(json, "author", "name"), None);
}
