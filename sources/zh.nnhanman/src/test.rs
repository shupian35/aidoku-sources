use aidoku::alloc::String;
use aidoku::{Manga, PageContent, Source};
use aidoku_test::aidoku_test;

use super::Nnhm7;
use crate::source_url::{BASE_URL, get_base_url};

fn new_source() -> Nnhm7 {
    <Nnhm7 as Source>::new()
}

#[aidoku_test]
fn get_base_url_defaults_to_const_when_no_override() {
    // With no `base_url` dynamic setting set, every URL the source builds
    // must use the hard-coded `BASE_URL` constant. The override path is
    // exercised at runtime when a user sets the dynamic setting.
    assert_eq!(get_base_url(), BASE_URL);
}

#[aidoku_test]
fn manga_url_is_absolute() {
    // Regression: the manga detail page renders an "open in browser"
    // button. Aidoku dispatches that using the manga's `url` field, which
    // must be an absolute URL. Previously `Manga.url` was left as
    // `..Default::default()` (None), so the button silently did nothing.
    let s = new_source();
    let manga = Manga {
        key: String::from("wo-de-i-n-yuan-tuan"),
        ..Default::default()
    };
    let updated = s
        .get_manga_update(manga, true, false)
        .expect("get manga update should succeed");
    let url = updated
        .url
        .as_deref()
        .expect("manga url should be set");
    assert!(
        url.starts_with("http://") || url.starts_with("https://"),
        "manga url must be absolute, got {url}"
    );
    assert!(
        url.contains("/comic/"),
        "manga url must point at the comic, got {url}"
    );
}

#[aidoku_test]
fn chapter_url_is_absolute() {
    // Regression: the chapter detail page renders an "open in browser"
    // button. The previous code stored only the relative path
    // (`/comic/<slug>/chapter-N.html`) so the button silently did nothing.
    let s = new_source();
    let manga = Manga {
        key: String::from("wo-de-i-n-yuan-tuan"),
        ..Default::default()
    };
    let updated = s
        .get_manga_update(manga, false, true)
        .expect("get manga update should succeed");
    let chs = updated.chapters.as_deref().expect("chapters");
    assert!(
        !chs.is_empty(),
        "chapter list should not be empty, got {}",
        chs.len()
    );
    let c = chs.first().expect("at least one chapter");
    let url = c.url.as_deref().expect("chapter url set");
    assert!(
        url.starts_with("http://") || url.starts_with("https://"),
        "chapter url must be absolute, got {url}"
    );
    assert!(
        url.contains("/chapter-"),
        "chapter url must point at a chapter, got {url}"
    );
}

#[aidoku_test]
fn viewer_is_webtoon_for_plain_webtoon() {
    // Default reading mode for a webtoon with no published/3D/JP tag.
    let s = new_source();
    let manga = Manga {
        key: String::from("wo-de-i-n-yuan-tuan"),
        ..Default::default()
    };
    let updated = s
        .get_manga_update(manga, true, false)
        .expect("get manga update should succeed");
    assert_eq!(
        updated.viewer,
        aidoku::Viewer::Webtoon,
        "default viewer should be Webtoon for plain webtoon"
    );
}

#[aidoku_test]
fn viewer_is_right_to_left_for_3d_manga() {
    // A manga tagged `3D` reads right-to-left.
    let s = new_source();
    let manga = Manga {
        key: String::from("3d-wo-de-qi-zi-bu-da-dui-jin"),
        ..Default::default()
    };
    let updated = s
        .get_manga_update(manga, true, false)
        .expect("get manga update should succeed");
    assert_eq!(
        updated.viewer,
        aidoku::Viewer::RightToLeft,
        "viewer should be RightToLeft for 3D manga"
    );
}

#[aidoku_test]
fn page_list_returns_nnpic_image_urls() {
    // get_page_list must surface every chapter page as a URL pointing at
    // the nnpic.xyz CDN. The host then fetches each image lazily through
    // ImageRequestProvider::get_image_request, which injects Referer +
    // User-Agent so the CDN doesn't route the request to a slow edge.
    let s = new_source();
    let manga = Manga {
        key: String::from("wo-de-i-n-yuan-tuan"),
        ..Default::default()
    };
    let updated = s
        .get_manga_update(manga.clone(), false, true)
        .expect("get manga update");
    let chs = updated.chapters.as_deref().expect("chapters");
    let first = chs.first().expect("at least one chapter");
    let pages = s
        .get_page_list(manga, first.clone())
        .expect("get page list");
    assert!(
        pages.len() >= 10,
        "should have many pages, got {}",
        pages.len()
    );
    for p in &pages {
        match &p.content {
            PageContent::Url(url, _) => {
                assert!(
                    url.starts_with("http://") || url.starts_with("https://"),
                    "image url must be absolute, got {url}"
                );
                assert!(
                    url.contains("nnpic.xyz"),
                    "image url must point at the nnpic CDN, got {url}"
                );
            }
            other => panic!("page must be a URL, got {other:?}"),
        }
    }
}

#[aidoku_test]
fn viewer_is_right_to_left_for_published_manga() {
    // A manga tagged `出版漫画` reads right-to-left.
    let s = new_source();
    let manga = Manga {
        key: String::from("zui-hou-de-chong-ci"),
        ..Default::default()
    };
    let updated = s
        .get_manga_update(manga, true, false)
        .expect("get manga update should succeed");
    assert_eq!(
        updated.viewer,
        aidoku::Viewer::RightToLeft,
        "viewer should be RightToLeft for published manga"
    );
}