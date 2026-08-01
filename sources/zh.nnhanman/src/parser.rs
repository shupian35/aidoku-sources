use aidoku::alloc::string::ToString;
use aidoku::alloc::{String, Vec, format};
use aidoku::imports::html::Html;
use aidoku::{AidokuError, Manga, Result};

pub(crate) fn slug_from_url(url: &str) -> Option<String> {
	let marker = "/comic/";
	let idx = url.find(marker)? + marker.len();
	let rest = &url[idx..];
	let slug = rest.strip_suffix(".html").unwrap_or(rest);
	let slug = slug.strip_suffix('/').unwrap_or(slug);
	if slug.is_empty() {
		None
	} else {
		Some(slug.to_string())
	}
}

pub(crate) fn parse_manga_grid(html: &str) -> Result<Vec<Manga>> {
	let doc = Html::parse(html)?;
	let items = doc
		.select("ul.col_3_1 > li")
		.ok_or_else(|| AidokuError::message("no list items"))?;
	let mut entries = Vec::new();
	for item in items {
		let link = match item.select_first("a.ImgA") {
			Some(l) => l,
			None => continue,
		};
		let href = link.attr("href").unwrap_or_default();
		let key = match slug_from_url(&href) {
			Some(k) => k,
			None => continue,
		};
		let title = link
			.attr("title")
			.or_else(|| item.select_first("a.txtA").and_then(|t| t.attr("title")))
			.unwrap_or_default()
			.to_string();
		let cover = item
			.select_first("img")
			.and_then(|img| img.attr("src").map(|s| s.to_string()));
		entries.push(Manga {
			key,
			title,
			cover,
			..Default::default()
		});
	}
	Ok(entries)
}

pub(crate) fn parse_manga_list(html: &str) -> Result<Vec<Manga>> {
	let doc = Html::parse(html)?;
	let items = doc
		.select("div.itemBox")
		.ok_or_else(|| AidokuError::message("no items"))?;
	let mut entries = Vec::new();
	for item in items {
		let link = match item.select_first("div.itemImg > a") {
			Some(l) => l,
			None => continue,
		};
		let href = link.attr("href").unwrap_or_default();
		let key = match slug_from_url(&href) {
			Some(k) => k,
			None => continue,
		};
		let title = item
			.select_first("a.title")
			.and_then(|t| t.attr("title"))
			.or_else(|| link.attr("title"))
			.unwrap_or_default()
			.to_string();
		let cover = item
			.select_first("div.itemImg img")
			.and_then(|img| img.attr("src").map(|s| s.to_string()));
		entries.push(Manga {
			key,
			title,
			cover,
			..Default::default()
		});
	}
	Ok(entries)
}

pub(crate) fn has_next_page(html: &str, current_page: i32) -> bool {
	let next_page_str = format!("/page/{}", current_page + 1);
	html.contains(&next_page_str)
}