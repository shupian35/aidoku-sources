#![no_std]

use aidoku::{
	AidokuError, Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, DynamicFilters,
	DynamicSettings, Filter, FilterValue, Home, HomeComponent, HomeComponentValue, HomeLayout,
	HomePartialResult, ImageRequestProvider, ImageResponse, Link, LinkValue, Listing, ListingKind,
	ListingProvider, Manga, MangaPageResult, MangaStatus, Page, PageContent, PageContext,
	PageImageProcessor, Result, SelectFilter, Setting, SortFilter, Source, TextSetting, Viewer,
	alloc::{String, Vec, borrow::Cow, format, string::ToString, vec},
	imports::canvas::ImageRef,
	imports::net::Request,
	imports::std::send_partial_result,
	prelude::*,
};

mod api;
mod md5;
mod scramble;
mod source_url;

#[cfg(test)]
mod test;

use scramble::{CTX_ALBUM, CTX_NAME, CTX_SCRAMBLE};
use source_url::{BASE_URL, get_base_url, image_request};

/// The `category` value that means "no category filter".
const ALL_CATEGORIES: &str = "0";

struct JmAura;

// ---------- Search and browse ----------

impl Source for JmAura {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let query = query.unwrap_or_default().trim().to_string();

		if !query.is_empty() {
			let entries = api::search(&query, page)?
				.iter()
				.map(summary_to_manga)
				.collect::<Vec<_>>();
			return Ok(MangaPageResult {
				has_next_page: !entries.is_empty(),
				entries,
			});
		}

		// Browsing mode. Category browsing and ranking share one endpoint, so
		// both are expressed as filter values.
		let (mut category, mut sort) = (String::from(ALL_CATEGORIES), String::from("mr"));
		for filter in &filters {
			match filter {
				FilterValue::Select { id, value } if id.as_str() == "category" => {
					category = value.clone();
				}
				FilterValue::Sort {
					id,
					index,
					ascending: _,
				} if id.as_str() == "sort" => {
					sort = sort_key(*index).to_string();
				}
				_ => {}
			}
		}

		let entries = api::leaderboard(&sort, &category, page)?
			.iter()
			.map(summary_to_manga)
			.collect::<Vec<_>>();

		Ok(MangaPageResult {
			has_next_page: !entries.is_empty(),
			entries,
		})
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if manga.key.is_empty() {
			return Err(AidokuError::message("missing manga key"));
		}
		let comic = api::comic(&manga.key)?;

		if needs_details {
			if !comic.title.is_empty() {
				manga.title = comic.title.clone();
			}
			if let Some(author) = &comic.author {
				if !author.is_empty() {
					manga.authors = Some(vec![author.clone()]);
				}
			}
			if let Some(cover) = &comic.cover_url {
				if !cover.is_empty() {
					manga.cover = Some(cover.clone());
				}
			}
			if let Some(description) = &comic.description {
				if !description.is_empty() {
					manga.description = Some(description.clone());
				}
			}
			if !comic.tags.is_empty() {
				manga.tags = Some(comic.tags.clone());
			}
			manga.status = MangaStatus::Completed;
			manga.content_rating = ContentRating::NSFW;
			manga.viewer = Viewer::RightToLeft;
			manga.url = Some(api::album_url(&comic.comic_id));
		}

		if needs_chapters {
			let mut chapters = Vec::new();
			for chapter in comic.chapters() {
				if chapter.id.is_empty() {
					continue;
				}
				chapters.push(Chapter {
					key: chapter.id.clone(),
					title: Some(chapter.title.clone()),
					chapter_number: Some(chapter.order as f32),
					url: Some(chapter_url(&chapter.id)),
					..Default::default()
				});
			}
			// Newest first, matching the order the site presents them in.
			chapters.reverse();
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let data = api::chapter(&chapter.key)?;

		// The chapter payload repeats the file names in `raw.images`; prefer the
		// structured entries and fall back to the raw list when `images[]` is
		// missing (which happens for albums with no readable pages).
		let mut names: Vec<String> = data
			.images
			.iter()
			.filter(|entry| !entry.name.is_empty())
			.map(|entry| entry.name.clone())
			.collect();
		if names.is_empty() {
			names = data
				.fallback_names()
				.iter()
				.filter(|name| !name.is_empty())
				.cloned()
				.collect();
		}
		if names.is_empty() {
			return Ok(Vec::new());
		}

		let album = data.album_id().to_string();
		let scramble = data.scramble_id().to_string();
		let base = get_base_url();

		let mut pages = Vec::with_capacity(names.len());
		for name in names {
			let mut context = PageContext::default();
			// The strip count and the image path are both derived from the album
			// id, so the processor needs it alongside the scramble id.
			context.insert(String::from(CTX_ALBUM), album.clone());
			context.insert(String::from(CTX_SCRAMBLE), scramble.clone());
			context.insert(String::from(CTX_NAME), name.clone());

			let url = format!("{}/api/chapter_image/{}/{}", base, album, name);
			pages.push(Page {
				content: PageContent::url_context(url, context),
				..Default::default()
			});
		}
		Ok(pages)
	}
}

// ---------- Listings ----------

impl ListingProvider for JmAura {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let entries: Vec<Manga> = match listing.id.as_str() {
			"latest" => api::latest(page)?.iter().map(latest_to_manga).collect(),
			// The random endpoint is a single unpaginated batch of ten.
			"random" => api::random()?.iter().map(summary_to_manga).collect(),
			// "leaderboard" and any unknown id fall back to the default ranking.
			_ => api::leaderboard("mr", ALL_CATEGORIES, page)?
				.iter()
				.map(summary_to_manga)
				.collect(),
		};
		let empty = entries.is_empty();
		Ok(MangaPageResult {
			has_next_page: !empty,
			entries,
		})
	}
}

impl Home for JmAura {
	fn get_home(&self) -> Result<HomeLayout> {
		let mut layout = HomeLayout::default();
		// An empty layout first, so the app can render sections as they arrive
		// instead of waiting for every request to finish.
		send_partial_result(&HomePartialResult::Layout(layout.clone()));

		if let Ok(items) = api::latest(1) {
			let component = HomeComponent {
				title: Some(String::from("最新上架")),
				subtitle: None,
				value: HomeComponentValue::Scroller {
					entries: items.iter().map(latest_to_link).collect(),
					listing: Some(Listing {
						id: String::from("latest"),
						name: String::from("最新上架"),
						kind: ListingKind::Default,
					}),
				},
			};
			send_partial_result(&HomePartialResult::Component(component.clone()));
			layout.components.push(component);
		}

		if let Ok(items) = api::leaderboard("mv", ALL_CATEGORIES, 1) {
			let component = HomeComponent {
				title: Some(String::from("点击排行")),
				subtitle: None,
				value: HomeComponentValue::MangaList {
					ranking: true,
					page_size: Some(10),
					entries: items.iter().map(summary_to_link).collect(),
					listing: None,
				},
			};
			send_partial_result(&HomePartialResult::Component(component.clone()));
			layout.components.push(component);
		}

		if let Ok(items) = api::random() {
			let component = HomeComponent {
				title: Some(String::from("随便看看")),
				subtitle: None,
				value: HomeComponentValue::BigScroller {
					entries: items.iter().map(summary_to_manga).collect(),
					auto_scroll_interval: None,
				},
			};
			send_partial_result(&HomePartialResult::Component(component.clone()));
			layout.components.push(component);
		}

		Ok(layout)
	}
}

// ---------- Filters and settings ----------

impl DynamicFilters for JmAura {
	fn get_dynamic_filters(&self) -> Result<Vec<Filter>> {
		let mut filters = vec![
			SortFilter {
				id: "sort".into(),
				title: Some("排序".into()),
				can_ascend: false,
				options: vec![
					"最新".into(),
					"最多浏览".into(),
					"最多图片".into(),
					"最多收藏".into(),
				],
				..Default::default()
			}
			.into(),
		];

		// The category tree is served by the API rather than hard-coded, so a
		// new upstream category shows up without a source update. Only the
		// top-level slugs are usable: the endpoint silently ignores numeric ids
		// and the nested sub-category slugs.
		if let Ok(categories) = api::categories() {
			let mut ids = vec![Cow::Borrowed(ALL_CATEGORIES)];
			let mut names = vec![Cow::Borrowed("全部")];
			for category in categories {
				let slug = if category.slug.is_empty() {
					category.id
				} else {
					category.slug
				};
				if slug.is_empty() || slug == ALL_CATEGORIES {
					continue;
				}
				ids.push(Cow::Owned(slug));
				names.push(Cow::Owned(category.name));
			}
			filters.push(
				SelectFilter {
					id: "category".into(),
					title: Some("分类".into()),
					options: names,
					ids: Some(ids),
					default: Some(Cow::Borrowed(ALL_CATEGORIES)),
					..Default::default()
				}
				.into(),
			);
		}

		Ok(filters)
	}
}

impl DynamicSettings for JmAura {
	fn get_dynamic_settings(&self) -> Result<Vec<Setting>> {
		Ok(vec![
			TextSetting {
				key: "base_url".into(),
				title: "自定义网址".into(),
				placeholder: Some(BASE_URL.into()),
				refreshes: Some(vec!["content".into(), "settings".into()]),
				..Default::default()
			}
			.into(),
		])
	}
}

// ---------- Deep links ----------

impl DeepLinkHandler for JmAura {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		// Album pages are `/comic/<id>`; a `cid` query selects one chapter.
		let Some(rest) = url.split("/comic/").nth(1) else {
			return Ok(None);
		};
		let mut parts = rest.split(['/', '?', '#']);
		let id = parts.next().unwrap_or("").trim();
		if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
			return Ok(None);
		}

		let chapter_id = url
			.split_once("cid=")
			.and_then(|(_, tail)| tail.split(['&', '#']).next())
			.filter(|value| value.bytes().all(|b| b.is_ascii_digit()));

		Ok(Some(match chapter_id {
			Some(chapter) => DeepLinkResult::Chapter {
				manga_key: id.to_string(),
				key: chapter.to_string(),
			},
			None => DeepLinkResult::Manga {
				key: id.to_string(),
			},
		}))
	}
}

// ---------- Images ----------

impl PageImageProcessor for JmAura {
	fn process_page_image(
		&self,
		response: ImageResponse,
		context: Option<PageContext>,
	) -> Result<ImageRef> {
		let image = response.image;
		let Some(context) = context else {
			return Ok(image);
		};
		Ok(scramble::unscramble(image, &context))
	}
}

impl ImageRequestProvider for JmAura {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		image_request(&url)
	}
}

// Every optional trait implemented above must be listed here by name: the
// macro only exports a handler for the traits passed to it, so an omission
// fails silently at runtime and `aidoku verify` cannot catch it (it only
// checks the three mandatory functions). Verify the export list after any
// change, e.g. by grepping the built wasm for the handler symbols.
register_source!(
	JmAura,
	ListingProvider,
	Home,
	DynamicFilters,
	DynamicSettings,
	DeepLinkHandler,
	PageImageProcessor,
	ImageRequestProvider
);

// ---------- Mapping helpers ----------

fn summary_to_manga(summary: &api::Summary) -> Manga {
	let key = summary.comic_id.clone();
	Manga {
		key: key.clone(),
		title: summary.title.clone(),
		cover: summary.cover_url.clone(),
		authors: summary
			.author
			.as_ref()
			.filter(|author| !author.is_empty())
			.map(|author| vec![author.clone()]),
		tags: Some(summary.tags.clone()),
		url: Some(api::album_url(&key)),
		content_rating: ContentRating::NSFW,
		viewer: Viewer::RightToLeft,
		..Default::default()
	}
}

fn summary_to_link(summary: &api::Summary) -> Link {
	let manga = summary_to_manga(summary);
	let cover = manga.cover.clone();
	Link {
		title: manga.title.clone(),
		subtitle: manga.authors.clone().map(|authors| authors.join(", ")),
		image_url: cover,
		value: Some(LinkValue::Manga(manga)),
	}
}

fn latest_to_manga(item: &api::LatestItem) -> Manga {
	let key = item.id.clone();
	Manga {
		key: key.clone(),
		title: item.name.clone(),
		cover: item.image.clone(),
		authors: item
			.author
			.as_ref()
			.filter(|author| !author.is_empty())
			.map(|author| vec![author.clone()]),
		tags: item
			.category
			.as_ref()
			.map(|category| vec![category.title.clone()]),
		url: Some(api::album_url(&key)),
		content_rating: ContentRating::NSFW,
		viewer: Viewer::RightToLeft,
		..Default::default()
	}
}

fn latest_to_link(item: &api::LatestItem) -> Link {
	let manga = latest_to_manga(item);
	let cover = manga.cover.clone();
	Link {
		title: manga.title.clone(),
		subtitle: manga.authors.clone().map(|authors| authors.join(", ")),
		image_url: cover,
		value: Some(LinkValue::Manga(manga)),
	}
}

fn chapter_url(id: &str) -> String {
	format!("{}/reader/{}", get_base_url(), id)
}

/// Maps the sort filter onto the upstream `sort` query value. The endpoint
/// only understands the "most" oriented keys, so ascending is not offered.
fn sort_key(index: i32) -> &'static str {
	match index {
		1 => "mv",
		2 => "mp",
		3 => "tf",
		_ => "mr",
	}
}
