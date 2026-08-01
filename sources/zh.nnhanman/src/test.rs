use aidoku::alloc::String;
use aidoku::{Manga, Source};
use aidoku_test::aidoku_test;

use super::Nnhm7;

fn new_source() -> Nnhm7 {
    <Nnhm7 as Source>::new()
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
fn viewer_is_webtoon() {
    // nnhanman is overwhelmingly webtoon-style; only the 3D category uses
    // a different layout. Set the default to Webtoon so most chapters open
    // in the correct reader mode.
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
        "default viewer should be Webtoon"
    );
}