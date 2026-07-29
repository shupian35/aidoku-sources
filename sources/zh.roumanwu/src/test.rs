use aidoku::alloc::{String, Vec};
use aidoku::{
	ContentRating, DeepLinkHandler, DeepLinkResult, Home, HomeComponentValue,
	Link, LinkValue, Listing, ListingProvider, Manga, MangaPageResult, MangaStatus,
	Page, PageContent, Source, Viewer,
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
	use aidoku::prelude::println;
	use aidoku::imports::net::Request;
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
	assert!(updated.description.is_some(), "description should be present");
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
	assert!(chs.len() >= 10, "should have many chapters, got {}", chs.len());
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
	assert!(pages.len() >= 50, "should have many pages, got {}", pages.len());
	for (i, p) in pages.iter().enumerate().take(3) {
		match &p.content {
			PageContent::Url(u, _) => assert!(u.contains("r5.rmcdn"), "page {i} url = {u}"),
			PageContent::Image(_) => {}, // Image was unscrambled successfully
			_ => panic!("page {i} is not a Url or Image"),
		}
	}
}

#[aidoku_test]
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
