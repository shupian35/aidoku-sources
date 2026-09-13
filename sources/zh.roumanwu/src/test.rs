use aidoku::alloc::{String, Vec, vec};
use aidoku::{
    ContentRating, DeepLinkHandler, DeepLinkResult, Home, HomeComponentValue, Link, LinkValue,
    Manga, MangaStatus, Page, PageContent, Source, Viewer,
};
use aidoku_test::aidoku_test;

use super::Roumanwu;
use crate::chapter::{build_pages, parse_chapter_pages, resolve_chapter_url};
use crate::detail::{
    decode_entities, json_top_level_string, manga_status_from_text, parse_manga_detail,
};
use crate::image::{scramble_slices, unscramble_image_url};
use crate::listing::{extract_manga_cards, has_next_page_from_html};
use crate::source_url::{BASE_URL, get_base_url};

fn new_source() -> Roumanwu {
    <Roumanwu as Source>::new()
}

#[aidoku_test]
fn custom_base_url_setting_takes_priority() {
    // Regression: the app-generated Base URL picker (the `url` defaults
    // key) always carries a value — the app registers the first preset as
    // its default — so a URL typed into the "自定义网址" text setting (the
    // `base_url` defaults key) must be consulted first. With the old order
    // the picker's value shadowed it and a custom URL could never take
    // effect, leaving users stuck with the preset mirrors.
    use aidoku::imports::defaults::{DefaultValue, defaults_set};

    // Custom text wins over the picker selection.
    defaults_set(
        "base_url",
        DefaultValue::String(String::from("https://roum99.example")),
    );
    defaults_set(
        "url",
        DefaultValue::String(String::from("https://roum28.xyz")),
    );
    assert_eq!(get_base_url(), "https://roum99.example");

    // Empty custom text falls through to the picker selection.
    defaults_set("base_url", DefaultValue::String(String::from("")));
    assert_eq!(get_base_url(), "https://roum28.xyz");

    // Nothing set falls back to the constant.
    defaults_set("url", DefaultValue::String(String::from("")));
    assert_eq!(get_base_url(), BASE_URL);

    // Clean up so sibling tests see no overrides.
    defaults_set("base_url", DefaultValue::Null);
    defaults_set("url", DefaultValue::Null);
}

#[aidoku_test]
fn debug_home_parse() {
    let s = new_source();
    let layout = s.get_home().expect("get home should succeed");
    let _ = aidoku::prelude::println!("debug_home_parse: got {} sections", layout.components.len());
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
    let scrambled = "https://v1.kelv47.xyz/m/bWYO6lLCSUgUlH3ZcUEfkwofo1hVCAx9RUc_bw0PnRU/wm:0/sr:1/czM6Ly9yb3VtYW4vaW1hZ2VzL2NtNHN4MXpwYTAwMGF2bmwwemlxbmJmeTUvZnJlZXgvNDQ1NDUvMjY5NjI4MS5qcGc.jpg";
    assert_eq!(scramble_slices(scrambled), Some(8));
    assert!(
        unscramble_image_url(scrambled),
        "sr:1 URL must be marked scrambled"
    );

    let plain = "https://v2.kelv47.xyz/m/uUzbUTZLfXg1oylH22QyIByCcolLMdPtncHSkLuSMMs/wm:2/sr:0/czM6Ly9yb3VtYW4vaW1hZ2VzL2NtNHN4MXpwYTAwMGF2bmwwemlxbmJmeTUvZnJlZXgvNDQ1NDUvMjY5NjI4Mi5qcGc.jpg";
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
    //
    // We pick a manga whose chapter 0 currently ships >50 pages so the test
    // catches both regressions: dropping corrupted URLs must not zero out the
    // chapter, and every surfaced URL must be a real http(s) URL. The
    // widget advertises 73 pages but the RSC actually has 128, so we want
    // 100+ to catch the stale-widget regression safely above that floor.
    let s = new_source();
    let manga = Manga {
        key: String::from("cm4sx1zpa000avnl0ziqnbfy5"),
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
        pages.len() >= 100,
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
fn parse_chapter_pages_reassembles_from_rsc_scripts() {
    // Build a chapter HTML with a couple of RSC script tags. Each script
    // contains the Next.js push pattern; together their unescaped payload
    // should yield every imageUrl / ind pair.
    let html = r#"
        <html><body>
            <script>self.__next_f.push([1,"{\"imageUrl\":\"https://v1.kelv47.xyz/p0.jpg\",\"ind\":0}"])</script>
            <script>self.__next_f.push([1,"{\"imageUrl\":\"https://v2.kelv47.xyz/p1.jpg\",\"ind\":1}"])</script>
            <script>other()</script>
        </body></html>
    "#;
    let urls = parse_chapter_pages(html).expect("parse");
    assert_eq!(
        urls,
        vec![
            String::from("https://v1.kelv47.xyz/p0.jpg"),
            String::from("https://v2.kelv47.xyz/p1.jpg"),
        ]
    );
}

#[aidoku_test]
fn parse_chapter_pages_drops_corrupted_urls() {
    // A truncated imageUrl (the "https" got cut at a chunk boundary) must
    // be filtered; otherwise the chapter would fail to load that page and
    // the host aborts the rest of the chapter.
    let html = r#"
        <html><body>
            <script>self.__next_f.push([1,"{\"imageUrl\":\"https\",\"ind\":0}"])</script>
            <script>self.__next_f.push([1,"{\"imageUrl\":\"https://v2.kelv47.xyz/p1.jpg\",\"ind\":1}"])</script>
        </body></html>
    "#;
    let urls = parse_chapter_pages(html).expect("parse");
    assert_eq!(urls.len(), 1);
    assert_eq!(urls[0], "https://v2.kelv47.xyz/p1.jpg");
}

#[aidoku_test]
fn parse_chapter_pages_ignores_stale_widget_count() {
    // Regression: the chapter detail page's "1/N頁" widget frequently
    // ships a stale page count (e.g. the chapter actually has 128 pages but
    // the widget says 73). The page list is the deduped RSC payload, so
    // every URL must surface even when the widget is significantly shorter.
    let html = r#"
        <html><body>
            <div class="text-muted-foreground text-right mr-4">1<!-- -->/<!-- -->2<!-- -->頁</div>
            <script>self.__next_f.push([1,"{\"imageUrl\":\"https://v1.kelv47.xyz/p0.jpg\",\"ind\":0}"])</script>
            <script>self.__next_f.push([1,"{\"imageUrl\":\"https://v2.kelv47.xyz/p1.jpg\",\"ind\":1}"])</script>
            <script>self.__next_f.push([1,"{\"imageUrl\":\"https://v3.kelv47.xyz/p2.jpg\",\"ind\":2}"])</script>
            <script>self.__next_f.push([1,"{\"imageUrl\":\"https://v4.kelv47.xyz/p3.jpg\",\"ind\":3}"])</script>
        </body></html>
    "#;
    let urls = parse_chapter_pages(html).expect("parse");
    // The widget says 2 but the RSC actually has 4 pages; we must surface
    // all 4 since the page count is now derived from the RSC payload.
    assert_eq!(urls.len(), 4);
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
    // The widget on the chapter detail page advertises 73 pages, but the
    // RSC payload actually has 128 unique URLs for this chapter. Surface
    // them all — clamping to the widget used to drop ~55 pages.
    assert!(
        pages.len() >= 100,
        "should have many pages, got {}",
        pages.len()
    );
    for (i, p) in pages.iter().enumerate().take(3) {
        match &p.content {
            PageContent::Url(u, _) => assert!(u.contains("kelv47.xyz"), "page {i} url = {u}"),
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
    // The widget advertises 73 but the RSC has 128 pages; we must surface
    // all of them, not clamp to the widget.
    assert!(
        pages.len() >= 80,
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
                assert!(url.contains("kelv47.xyz"), "unexpected url: {url}");
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

#[aidoku_test]
fn search_finds_known_manga() {
    // Search for a term the live site reliably indexes. Pick a high-traffic
    // tag so the test isn't dependent on any one title still being present.
    let s = new_source();
    let res = s
        .get_search_manga_list(Some(String::from("人妻")), 1, Vec::new())
        .expect("search should succeed");
    assert!(
        !res.entries.is_empty(),
        "search should return results, got {}",
        res.entries.len()
    );
}

#[aidoku_test]
fn chapter_list_includes_numberless_chapters() {
    // Chapters are listed newest-first: "最終話" / "後記" first, "第1話"
    // last, each keeping its original title. The "開始閱讀" button above
    // the grid is dropped.
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
    assert!(
        !titles.iter().any(|t| t == "開始閱讀" || t == "开始阅读"),
        "start-reading CTA must not appear, got {titles:?}"
    );
    // Newest first: 後記 before 第1話.
    let first = titles.iter().position(|t| t.contains("第1話"));
    let last = titles.iter().position(|t| t == "後記");
    assert!(
        matches!((first, last), (Some(f), Some(l)) if l < f),
        "後記 should appear before 第1話, got {titles:?}"
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
fn chapter_list_matches_site_grid() {
    // The grid renders 第1話…第N話 (newest first once reversed). The
    // 開始閱讀 button above the grid must not leak into the chapter list.
    // We don't pin a specific N — the live manga keeps accruing chapters,
    // so we just enforce order invariants and the CTA exclusion.
    let s = new_source();
    let manga = Manga {
        key: String::from("cmjuau8r3000hs6i94s7qug06"),
        ..Default::default()
    };
    let updated = s
        .get_manga_update(manga, false, true)
        .expect("get manga update should succeed");
    let chs = updated.chapters.as_deref().expect("chapters");
    assert!(
        chs.len() >= 39,
        "should be at least 39 chapters, got {}",
        chs.len()
    );
    let first = chs
        .first()
        .and_then(|c| c.title.clone())
        .unwrap_or_default();
    let last = chs.last().and_then(|c| c.title.clone()).unwrap_or_default();
    assert!(first.contains("第"), "first should be 第N話, got {first:?}");
    assert!(last.contains("第1話"), "last should be 第1話, got {last:?}");
    // First chapter must have a strictly higher index than the last.
    let first_num = chs
        .first()
        .and_then(|c| c.chapter_number)
        .unwrap_or_default();
    let last_num = chs
        .last()
        .and_then(|c| c.chapter_number)
        .unwrap_or_default();
    assert!(
        first_num > last_num,
        "first chapter_number ({first_num}) should be > last ({last_num})"
    );
    for c in chs {
        assert_ne!(c.title.as_deref().unwrap_or(""), "開始閱讀");
    }
}

#[aidoku_test]
fn chapter_list_keeps_announcements() {
    // The site's chapter grid also contains non-chapter entries (休刊公告,
    // 登場人物MBTI, 第4季預告, 後記). Keep them all so the list matches the
    // site; only the 開始閱讀 button above the grid is dropped.
    let s = new_source();
    let manga = Manga {
        key: String::from("e1a23182-bb48-4215-b301-5ebfe9edc9b4"),
        ..Default::default()
    };
    let updated = s
        .get_manga_update(manga, false, true)
        .expect("get manga update should succeed");
    let chs = updated.chapters.as_deref().expect("chapters");
    assert!(
        chs.len() >= 150,
        "should be at least 150 entries, got {}",
        chs.len()
    );
    let titles: Vec<String> = chs
        .iter()
        .map(|c| c.title.clone().unwrap_or_default())
        .collect();
    for needle in ["休刊一周公告", "登場人物MBTI", "第4季預告", "後記"] {
        assert!(
            titles.iter().any(|t| t == needle),
            "{needle} should be present"
        );
    }
    assert!(
        !titles.iter().any(|t| t == "開始閱讀"),
        "start-reading CTA must not appear"
    );
    assert_eq!(titles.first().unwrap(), "後記");
    assert!(
        titles.last().unwrap().contains("第1話"),
        "last should be 第1話 (newest first), got {:?}",
        titles.last()
    );
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
    let html = r#"<a class="site-comic" href="/books/abc123"><div class="site-comic-cover"><img src="https://kelv47.xyz/c.jpg" width="300" height="450"></div><h3 title="Title A">Title A</h3></a>"#;
    let cards = extract_manga_cards(html).expect("parse");
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].key, "abc123");
    assert_eq!(cards[0].title, "Title A");
    assert_eq!(cards[0].cover.as_deref(), Some("https://kelv47.xyz/c.jpg"));
}

#[aidoku_test]
fn extract_manga_cards_skips_chapter_anchors() {
    // /books/{id}/{N} has 3 slashes — chapter anchors, not manga list entries.
    let html = r#"<a class="site-comic" href="/books/abc/1"><h3 title="chap">chap</h3></a><a class="site-comic" href="/books/xyz"><h3 title="manga">manga</h3></a>"#;
    let cards = extract_manga_cards(html).expect("parse");
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].key, "xyz");
}

#[aidoku_test]
fn extract_manga_cards_dedupes_repeated_keys() {
    let html = r#"
        <a class="site-comic" href="/books/dup"><h3 title="first">first</h3></a>
        <a class="site-comic" href="/books/dup"><h3 title="second">second</h3></a>
    "#;
    let cards = extract_manga_cards(html).expect("parse");
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].title, "first", "first occurrence wins");
}

#[aidoku_test]
fn extract_manga_cards_skips_empty_titles() {
    let html = r#"<a class="site-comic" href="/books/a"><h3 title="   ">   </h3></a><a class="site-comic" href="/books/b"><h3 title="real">real</h3></a>"#;
    let cards = extract_manga_cards(html).expect("parse");
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].key, "b");
}

#[aidoku_test]
fn extract_manga_cards_parses_latest_chapter() {
    // The `<span class="site-comic-chapter">第N話-…</span>` inside the first
    // `site-comic-meta` block becomes the manga's description so home/listing
    // cards can show what the manga updated to.
    let html = r#"<a class="site-comic" href="/books/abc123"><div class="site-comic-cover"><img src="https://kelv47.xyz/c.jpg"></div><h3 title="Title A">Title A</h3><div class="site-comic-meta"><span class="site-comic-chapter" title="第5話-測試標題">第5話-測試標題</span><span class="shrink-0">連載中</span></div></a>"#;
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
    // The last `site-comic-meta` row carries region + views; the parser
    // emits them as `地区 X` and `浏览 Y` so cards can surface them.
    let html = r#"<a class="site-comic" href="/books/abc123"><div class="site-comic-cover"><img src="https://kelv47.xyz/c.jpg"></div><h3 title="Title A">Title A</h3><div class="site-comic-meta"><span class="site-comic-chapter" title="第5話-測試標題">第5話-測試標題</span><span class="shrink-0">連載中</span></div><div class="site-comic-meta"><span>韓國</span><span>◉ 701K</span></div></a>"#;
    let cards = extract_manga_cards(html).expect("parse");
    assert_eq!(cards.len(), 1);
    let tags = cards[0].tags.as_deref().expect("tags");
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0], "地区 韓國");
    assert_eq!(tags[1], "浏览 ◉ 701K");
}

#[aidoku_test]
fn extract_manga_cards_emits_region_only_when_views_missing() {
    // Search pages sometimes leave the views span empty — emit a single
    // region tag instead of dropping tags entirely.
    let html = r#"<a class="site-comic" href="/books/abc"><h3 title="T">T</h3><div class="site-comic-meta"><span>韓國</span><span></span></div></a>"#;
    let cards = extract_manga_cards(html).expect("parse");
    let tags = cards[0].tags.as_deref().expect("tags");
    assert_eq!(tags, &["地区 韓國"]);
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
fn json_top_level_string_returns_none_for_missing_key() {
    let json = r#"{"other":42}"#;
    assert_eq!(json_top_level_string(json, "name"), None);
}

#[aidoku_test]
fn manga_status_from_text_maps_known_values() {
    assert_eq!(manga_status_from_text("連載中"), MangaStatus::Ongoing);
    assert_eq!(manga_status_from_text("已完結"), MangaStatus::Completed);
    assert_eq!(manga_status_from_text("完結"), MangaStatus::Completed);
    assert_eq!(manga_status_from_text("休刊中"), MangaStatus::Hiatus);
    assert_eq!(manga_status_from_text("停刊"), MangaStatus::Hiatus);
    assert_eq!(manga_status_from_text(""), MangaStatus::Unknown);
    assert_eq!(manga_status_from_text("garbage"), MangaStatus::Unknown);
}

#[aidoku_test]
fn decode_entities_handles_common_html_entities() {
    let decoded = decode_entities("A &amp; B &quot;C&quot; &#039;D&#039; &lt;E&gt;");
    assert_eq!(decoded, "A & B \"C\" 'D' <E>");
}

#[aidoku_test]
fn has_next_page_detects_next_page_link() {
    // Numbered pagination: current_page=1 looks for "page=2"; the second
    // anchor matches. With current_page=3 there is no page=4 anchor and the
    // page text ("3") doesn't contain `下一頁`/`Next`, so we return false.
    let html = r#"
        <html><body>
            <a class="pg" href="/books?page=3">3</a>
            <a class="next" href="/books?page=2">2</a>
        </body></html>
    "#;
    assert!(has_next_page_from_html(html, 1));
    assert!(!has_next_page_from_html(html, 3));
    assert!(!has_next_page_from_html(html, 0));
}

#[aidoku_test]
fn has_next_page_detects_text_only_pagination() {
    // Some listing pages don't number their pagination — only `<a>下一頁</a>`.
    // The text fallback handles those.
    let html = "<html><body><a href=\"#\">下一頁</a></body></html>";
    assert!(has_next_page_from_html(html, 0));
}

// ---------- parse_manga_detail pure-function tests ----------

#[aidoku_test]
fn parse_manga_detail_reads_dl_rows() {
    // The new detail page renders `<dl class="site-book-data">` rows. Only
    // `作者` and `狀態` are surfaced to the app — `地區` / `更新` stay on
    // the page but aren't pulled through.
    let html = r#"
        <html><body>
            <h1>測試漫畫</h1>
            <img class="site-detail-cover" src="https://kelv47.xyz/c.jpg">
            <dl class="site-book-data">
                <dt>作者</dt><dd>刑作家&amp;橘皮</dd>
                <dt>狀態</dt><dd>連載中<!-- --> · <!-- -->77<!-- --> 話</dd>
                <dt>地區</dt><dd>韓國</dd>
                <dt>更新</dt><dd>9/7/2026</dd>
            </dl>
        </body></html>
    "#;
    let m = parse_manga_detail(html, "abc").expect("parse");
    assert_eq!(m.title, "測試漫畫");
    assert_eq!(m.cover.as_deref(), Some("https://kelv47.xyz/c.jpg"));
    let authors = m.authors.as_deref().expect("authors");
    assert_eq!(authors, &[String::from("刑作家&橘皮")][..]);
    assert_eq!(m.status, MangaStatus::Ongoing);
}

#[aidoku_test]
fn parse_manga_detail_reads_status_completed() {
    // The 狀態 value ships with trailing `· N 話` on completed titles; the
    // status mapper still picks up `完結`.
    let html = r#"
        <html><body>
            <h1>完結作</h1>
            <dl class="site-book-data">
                <dt>作者</dt><dd>Author</dd>
                <dt>狀態</dt><dd>已完結<!-- --> · <!-- -->12<!-- --> 話</dd>
            </dl>
        </body></html>
    "#;
    let m = parse_manga_detail(html, "k").expect("parse");
    assert_eq!(m.status, MangaStatus::Completed);
}

#[aidoku_test]
fn parse_manga_detail_prefers_synopsis_over_json_ld() {
    // When the page renders both the synopsis paragraph and JSON-LD, prefer
    // the rendered paragraph (it's the up-to-date copy).
    let html = r#"
        <html><body>
            <h1>頁面</h1>
            <div class="site-book-synopsis text-foreground"><p>這是渲染過的簡介</p></div>
            <script type="application/ld+json">
                {"@context":"https://schema.org","@type":"Book","name":"X","description":"JSON-LD 的備用描述"}
            </script>
        </body></html>
    "#;
    let m = parse_manga_detail(html, "k").expect("parse");
    assert_eq!(m.description.as_deref(), Some("這是渲染過的簡介"));
}

#[aidoku_test]
fn parse_manga_detail_falls_back_to_json_ld_description() {
    // When the page omits the synopsis paragraph, fall back to JSON-LD.
    // The site HTML-encodes the JSON-LD body (so `描述` ships as
    // `&#25551;&#36848;`); the parser decodes those entities before
    // extracting the field.
    let html = r#"
        <html><body>
            <h1>頁面</h1>
            <script type="application/ld+json">
                {"@context":"https://schema.org","@type":"Book","name":"X","description":"JSON-LD &#25551;&#36848;"}
            </script>
        </body></html>
    "#;
    let m = parse_manga_detail(html, "k").expect("parse");
    assert_eq!(m.description.as_deref(), Some("JSON-LD 描述"));
}

#[aidoku_test]
fn parse_manga_detail_chapters_scoped_to_site_chapters() {
    // Anchoring on `div.site-chapters` drops the `開始閱讀` CTA above the
    // grid and any related-manga links under it. Each anchor's `<span title>`
    // becomes the chapter title — anchors append a `<small>↗</small>` (or
    // `NEW` badge) after the span, so reading anchor.text() directly would
    // concatenate that suffix onto the title.
    let html = r#"
        <html><body>
            <h1>書</h1>
            <a class="site-button site-button-primary" href="/books/abc/0">開始閱讀</a>
            <section aria-label="章節目錄">
                <div class="site-chapters">
                    <a class="site-chapter-link" href="/books/abc/0">
                        <span title="第1話-開始">第1話-開始</span><small>↗</small>
                    </a>
                    <a class="site-chapter-link" href="/books/abc/1">
                        <span title="第2話-繼續">第2話-繼續</span><small>↗</small>
                    </a>
                    <a class="site-chapter-link" href="/books/abc/2">
                        <span title="第3話-NEW">第3話-NEW</span><small>NEW</small>
                    </a>
                </div>
            </section>
            <a class="site-comic" href="/books/related"><h3 title="相關作品">相關作品</h3></a>
        </body></html>
    "#;
    let m = parse_manga_detail(html, "abc").expect("parse");
    let chs = m.chapters.as_deref().expect("chapters");
    // Newest first (DOM is 0,1,2; reversed = 2,1,0).
    assert_eq!(chs.len(), 3);
    assert_eq!(chs[0].key, "2");
    assert_eq!(chs[0].title.as_deref(), Some("第3話-NEW"));
    assert_eq!(chs[1].key, "1");
    assert_eq!(chs[2].key, "0");
    assert_eq!(chs[2].title.as_deref(), Some("第1話-開始"));
    // No `開始閱讀` slipped in.
    for c in chs {
        assert_ne!(c.title.as_deref(), Some("開始閱讀"));
    }
}
