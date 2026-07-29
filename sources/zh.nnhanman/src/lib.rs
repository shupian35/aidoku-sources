#![no_std]
use aidoku::{
	AidokuError, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, Home,
	HomeComponent, HomeComponentValue, HomeLayout, Link, LinkValue, DynamicFilters, Filter,
	Manga, MangaPageResult, SelectFilter, SortFilter, MangaStatus, Page, PageContent, Result, Source, Viewer,
	alloc::{format, string::ToString, vec, String, Vec},
	helpers::uri::encode_uri,
	imports::{html::Html, net::Request},
	prelude::*,
};

const BASE_URL: &str = "https://nnhm7.com";
const USER_AGENT: &str =
	"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

// ---------- HTTP helpers ----------

fn html_get_string(url: &str) -> Result<String> {
	Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Accept-Language", "zh-TW,zh;q=0.9,en;q=0.8")
		.string()
}

// ---------- Parsing helpers ----------

fn slug_from_url(url: &str) -> Option<String> {
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

fn parse_manga_grid(html: &str) -> Result<Vec<Manga>> {
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

fn parse_manga_list(html: &str) -> Result<Vec<Manga>> {
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

fn has_next_page(html: &str, current_page: i32) -> bool {
	let next_page_str = format!("/page/{}", current_page + 1);
	html.contains(&next_page_str)
}

struct Nnhm7;

impl Source for Nnhm7 {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let q = query.unwrap_or_default();
		if !q.is_empty() {
			// Text search via the site's search endpoint.
			let encoded = encode_uri(q);
			let url = format!("{}/catalog.php?key={}", BASE_URL, encoded);
			let html = html_get_string(&url)?;
			let entries = parse_manga_grid(&html)?;
			let has_next = has_next_page(&html, page);
			return Ok(MangaPageResult {
				entries,
				has_next_page: has_next,
			});
		}

		// Filter-based browsing. The `kind` SelectFilter dispatches the top
		// browse surface to one of three upstream URLs; the remaining filters
		// (category / status / sort) only apply when kind == "all".
		let mut kind = String::from("all");
		let mut category = String::from("all");
		let mut status = String::from("all");
		let mut sort = String::from("time");

		for filter in &filters {
			match filter {
				FilterValue::Select { id, value } => {
					match id.as_str() {
						"kind" => kind = value.clone(),
						"category" => category = value.clone(),
						"status" => status = value.clone(),
						_ => {}
					}
				}
				FilterValue::Sort { id, index, ascending: _ } => {
					if id.as_str() == "sort" {
						sort = match index {
							0 => String::from("time"),
							1 => String::from("hits"),
							_ => String::from("time"),
						};
					}
				}
				_ => {}
			}
		}

		match kind.as_str() {
			// /update is a single page rendered with div.itemBox cards.
			"latest" => {
				let url = format!("{}/update", BASE_URL);
				let html = html_get_string(&url)?;
				let entries = parse_manga_list(&html).unwrap_or_default();
				Ok(MangaPageResult {
					entries,
					has_next_page: false,
				})
			}
			// /ranking (总榜) is a single page rendered with div.itemBox cards.
			"ranking" => {
				let url = format!("{}/ranking", BASE_URL);
				let html = html_get_string(&url)?;
				let entries = parse_manga_list(&html).unwrap_or_default();
				Ok(MangaPageResult {
					entries,
					has_next_page: false,
				})
			}
			// "all" (or any unknown value): paginated browse using
			// /comics/all/{cat}/{sort}/st/{status}/page/{n}, rendered with
			// the homepage ul.col_3_1 > li grid (parse_manga_grid).
			_ => {
				let url = format!(
					"{}/comics/{}/ob/{}/st/{}/page/{}",
					BASE_URL, category, sort, status, page
				);
				let html = html_get_string(&url)?;
				let entries = parse_manga_grid(&html).unwrap_or_default();
				let has_next = has_next_page(&html, page);
				Ok(MangaPageResult {
					entries,
					has_next_page: has_next,
				})
			}
		}
	}

	fn get_manga_update(
		&self,
		manga: Manga,
		_needs_details: bool,
		_needs_chapters: bool,
	) -> Result<Manga> {
		let key = manga.key.clone();
		if key.is_empty() {
			return Err(AidokuError::message("missing manga key"));
		}
		let url = format!("{}/comic/{}.html", BASE_URL, key);
		let html = html_get_string(&url)?;
		let doc = Html::parse(&html)?;

		// Parse cover
		let cover = doc
			.select_first("div#Cover img")
			.and_then(|img| img.attr("src").map(|s| s.to_string()));

		// Parse title from h1 - remove brackets
		let title_raw = doc
			.select_first("h1")
			.and_then(|h| h.text())
			.unwrap_or_else(|| manga.title.clone());
		let title = title_raw
			.trim_matches(|c| c == '\u{300A}' || c == '\u{300B}' || c == ' ')
			.to_string();

		// Parse author
		let author = doc
			.select_first("p.txtItme")
			.and_then(|p| p.text())
			.map(|s| s.trim().to_string());

		// Parse categories/tags
		let mut tags = Vec::new();
		if let Some(tag_els) = doc.select("p.txtItme a[href*='/comics/']") {
			for tag in tag_els {
				if let Some(tag_text) = tag.text() {
					let t = tag_text.trim().to_string();
					if !t.is_empty() {
						tags.push(t);
					}
				}
			}
		}

		// Parse status
		let status_text = doc
			.select_first("span.date")
			.and_then(|s| s.text())
			.unwrap_or_default();
		let status = if status_text.contains("\u{5B8C}\u{7EDD}")
			|| status_text.contains("\u{5B8C}\u{7D50}")
		{
			MangaStatus::Completed
		} else {
			MangaStatus::Ongoing
		};

		// Parse description
		let description = doc.select_first("p.txtDesc").and_then(|p| p.text()).map(|text| {
			text.strip_prefix("\u{4ECB}\u{7ECD}:")
				.unwrap_or(&text)
				.trim()
				.to_string()
		});

		// Parse chapters
		let mut chapters = Vec::new();
		if let Some(chap_els) = doc.select("ul#mh-chapter-list-ol-0 > li") {
			for li in chap_els {
				let a = match li.select_first("a") {
					Some(a) => a,
					None => continue,
				};
				let href = a.attr("href").unwrap_or_default();
				let chapter_title = a
					.select_first("span")
					.and_then(|s| s.text())
					.unwrap_or_else(|| a.text().unwrap_or_default());
				let chap_key = href
					.rsplit('/')
					.next()
					.unwrap_or("")
					.strip_suffix(".html")
					.unwrap_or("")
					.to_string();
				chapters.push(Chapter {
					key: chap_key,
					title: Some(chapter_title),
					url: Some(href.to_string()),
					..Default::default()
				});
			}
		}

		let authors = author.map(|a| vec![a]);

		Ok(Manga {
			key,
			title,
			cover,
			authors,
			description,
			tags: Some(tags),
			status,
			chapters: Some(chapters),
			content_rating: ContentRating::NSFW,
			viewer: Viewer::RightToLeft,
			..Default::default()
		})
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let chap_url = chapter
			.url
			.clone()
			.unwrap_or_else(|| {
				format!(
					"{}/comic/{}/chapter-{}.html",
					BASE_URL, _manga.key, chapter.key
				)
			});
		let full_url = if chap_url.starts_with("http") {
			chap_url
		} else {
			format!("{}{}", BASE_URL, chap_url)
		};
		let html = html_get_string(&full_url)?;
		let doc = Html::parse(&html)?;
		let mut pages = Vec::new();
		if let Some(img_els) = doc.select("img[data-index]") {
			for img in img_els {
				if let Some(src) = img.attr("data-src").or_else(|| img.attr("src")) {
					if src.contains("nnpic.xyz") {
						pages.push(Page {
							content: PageContent::url(src.to_string()),
							..Default::default()
						});
					}
				}
			}
		}
		Ok(pages)
	}
}

impl DynamicFilters for Nnhm7 {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		Ok(vec![
			// Top-level mode selector. The browse surface has three roots:
			//   "all"      -> /comics/all/{cat}/{sort}/st/{status}/page/{n} (paginated, uses col_3_1 grid)
			//   "latest"   -> /update (single page, itemBox cards)
			//   "ranking"  -> /ranking (single page, itemBox cards)
			// The other filters (category / status / sort) only apply when
			// kind == "all"; they are simply ignored for the other modes.
			SelectFilter {
				id: "kind".into(),
				title: Some("类型".into()),
				options: vec![
					"全部分类".into(),
					"最新更新".into(),
					"排行榜".into(),
				],
				ids: Some(vec![
					"all".into(),
					"latest".into(),
					"ranking".into(),
				]),
				default: Some("all".into()),
				..Default::default()
			}
			.into(),
			SelectFilter {
				id: "category".into(),
				title: Some("\u{5206}\u{7C7B}".into()),
				options: vec![
					"\u{5168}\u{90E8}".into(),
					"\u{6B63}\u{59B9}".into(),
					"\u{604B}\u{7231}".into(),
					"\u{51FA}\u{7248}\u{6F2B}\u{753B}".into(),
					"\u{8089}\u{6176}".into(),
					"\u{6D6A}\u{6F2B}".into(),
					"\u{5927}\u{5C3A}\u{5EA6}".into(),
					"\u{5DE8}\u{4E73}".into(),
					"\u{6709}\u{592B}\u{4E4B}\u{5A66}".into(),
					"\u{5973}\u{5927}\u{751F}".into(),
					"\u{72D7}\u{8840}\u{5287}".into(),
					"\u{540C}\u{5C45}".into(),
					"\u{597D}\u{53CB}".into(),
					"\u{8ABF}\u{6559}".into(),
					"\u{52A8}\u{4F5C}".into(),
					"\u{5F8C}\u{5BAE}".into(),
					"\u{4E0D}\u{502B}".into(),
					"3D".into(),
					"\u{6821}\u{5712}".into(),
					"\u{803D}\u{7F8E}".into(),
					"\u{65E5}\u{6F2B}".into(),
				],
				..Default::default()
			}
			.into(),
			SelectFilter {
				id: "status".into(),
				title: Some("\u{72B6}\u{6001}".into()),
				options: vec![
					"\u{5168}\u{90E8}".into(),
					"\u{8FDE}\u{8F7D}\u{4E2D}".into(),
					"\u{5DF2}\u{5B8C}\u{7EDD}".into(),
				],
				..Default::default()
			}
			.into(),
			SortFilter {
				id: "sort".into(),
				title: Some("\u{6392}\u{5E8F}".into()),
				can_ascend: false,
				options: vec![
					"\u{6309}\u{65F6}\u{95F4}".into(),
					"\u{6309}\u{70ED}\u{5EA6}".into(),
				],
				..Default::default()
			}
			.into(),
		])
	}
}

impl Home for Nnhm7 {
	fn get_home(&self) -> Result<HomeLayout> {
		let html = html_get_string(&format!("{}/", BASE_URL))?;
		let doc = Html::parse(&html)?;

		let mut components = Vec::new();
		let sections = match doc.select("div.imgBox") {
			Some(s) => s,
			None => return Ok(HomeLayout { components }),
		};

		for section in sections {
			let title = section
				.select_first("span.Title")
				.and_then(|s| s.text())
				.unwrap_or_default();

			let mut links: Vec<Link> = Vec::new();
			if let Some(li_els) = section.select("ul.col_3_1 > li") {
				for li in li_els {
					let a = match li.select_first("a.ImgA") {
						Some(l) => l,
						None => continue,
					};
					let href = a.attr("href").unwrap_or_default();
					let key = match slug_from_url(&href) {
						Some(k) => k,
						None => continue,
					};
					let m_title = a
						.attr("title")
						.or_else(|| {
							li.select_first("a.txtA")
								.and_then(|t| t.attr("title"))
						})
						.unwrap_or_default()
						.to_string();
					let cover = li
						.select_first("img")
						.and_then(|img| img.attr("src").map(|s| s.to_string()));
					let manga = Manga {
						key,
						title: m_title.clone(),
						cover: cover.clone(),
						..Default::default()
					};
					links.push(Link {
						title: m_title,
						subtitle: None,
						image_url: cover,
						value: Some(LinkValue::Manga(manga)),
					});
				}
			}

			if !links.is_empty() {
				components.push(HomeComponent {
					title: Some(title),
					subtitle: None,
					value: HomeComponentValue::MangaList {
						ranking: false,
						page_size: Some(3),
						entries: links,
						listing: None,
					},
				});
			}
		}

		Ok(HomeLayout { components })
	}
}

impl DeepLinkHandler for Nnhm7 {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let url = url.trim();
		if let Some(idx) = url.find("/comic/") {
			let after = &url[idx + 7..];
			if let Some(chap_idx) = after.find("/chapter-") {
				let slug = &after[..chap_idx];
				let chap_rest = &after[chap_idx + 9..];
				let chap_id: String = chap_rest
					.chars()
					.take_while(|c| c.is_ascii_digit())
					.collect();
				if !slug.is_empty() && !chap_id.is_empty() {
					return Ok(Some(DeepLinkResult::Chapter {
						manga_key: slug.into(),
						key: chap_id.into(),
					}));
				}
			}
			let slug = after
				.strip_suffix(".html")
				.unwrap_or(after)
				.strip_suffix('/')
				.unwrap_or(after);
			if !slug.is_empty() {
				return Ok(Some(DeepLinkResult::Manga { key: slug.into() }));
			}
		}
		Ok(None)
	}
}

register_source!(Nnhm7, DynamicFilters, Home, DeepLinkHandler);