#![no_std]

use aidoku::alloc::string::ToString;
use aidoku::alloc::{String, Vec, format, vec};
use aidoku::imports::canvas::ImageRef;
use aidoku::imports::net::Request;
use aidoku::register_source;
use aidoku::{
    AidokuError, BaseUrlProvider, Chapter, DeepLinkHandler, DeepLinkResult, DynamicSettings,
    FilterValue, HashMap, Home, HomeLayout, ImageRequestProvider, ImageResponse, Listing,
    ListingProvider, Manga, MangaPageResult, Page, PageContent, PageContext, PageImageProcessor,
    Result, Source,
};

mod chapter;
mod detail;
mod home;
mod image;
mod listing;
mod source_url;
mod utils;

use chapter::parse_chapter_pages;
use detail::parse_manga_detail;
use home::parse_home_layout;
use image::{unscramble_image, unscramble_image_url};
use listing::parse_manga_listing;
use source_url::{BASE_URL, get_base_url, html_get_string};
use utils::{site_page, urlencode};

pub struct Roumanwu;

impl Source for Roumanwu {
    fn new() -> Self {
        Self
    }

    fn get_search_manga_list(
        &self,
        query: Option<String>,
        page: i32,
        _filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        let sp = site_page(page);
        let url = match query.as_deref() {
            Some(q) if !q.trim().is_empty() => {
                format!(
                    "{}/search?term={}&page={}",
                    get_base_url(),
                    urlencode(q),
                    sp
                )
            }
            _ => format!("{}/books?page={}", get_base_url(), sp),
        };
        let html = html_get_string(&url)?;
        parse_manga_listing(&html, sp)
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
        let url = format!("{}/books/{}", get_base_url(), key);
        let html = html_get_string(&url)?;
        parse_manga_detail(&html, &key)
    }

    fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
        let path = chapter
            .url
            .clone()
            .unwrap_or_else(|| format!("/books/{}/{}", manga.key, chapter.key));
        let full = format!("{}{}", get_base_url(), path);
        let html = html_get_string(&full)?;
        let (page_count, mut urls) = parse_chapter_pages(&html)?;

        if page_count > 0 && urls.len() > page_count as usize {
            urls.truncate(page_count as usize);
        }

        // Return every URL as PageContent::Url. Previously the parser eagerly
        // downloaded and unscrambled every sr:1 image before returning, which
        // serialized tens of large HTTP fetches on the chapter-open thread and
        // made Aidoku's chapter load dramatically slower than the web reader.
        // Scrambled URLs are tagged via PageContext so process_page_image can
        // unscramble them as the app fetches each image lazily.
        let pages = urls
            .into_iter()
            .map(|u| {
                if unscramble_image_url(&u) {
                    let mut ctx: PageContext = HashMap::new();
                    ctx.insert("scramble".into(), "1".into());
                    Page {
                        content: PageContent::url_context(u, ctx),
                        thumbnail: None,
                        has_description: false,
                        description: None,
                    }
                } else {
                    Page {
                        content: PageContent::url(u),
                        thumbnail: None,
                        has_description: false,
                        description: None,
                    }
                }
            })
            .collect();

        Ok(pages)
    }
}

impl ListingProvider for Roumanwu {
    fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
        let sp = site_page(page);
        let filter = match listing.id.as_str() {
            "ongoing" => "true",
            "completed" => "false",
            "all" | "default" => "",
            _ => "",
        };
        let url = format!("{}/books?continued={}&page={}", get_base_url(), filter, sp);
        let html = html_get_string(&url)?;
        parse_manga_listing(&html, sp)
    }
}

impl Home for Roumanwu {
    fn get_home(&self) -> Result<HomeLayout> {
        let html = html_get_string(&format!("{}/home", get_base_url()))?;
        parse_home_layout(&html)
    }
}

impl DeepLinkHandler for Roumanwu {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        let url = url.trim();
        let marker = "/books/";
        let Some(idx) = url.find(marker) else {
            return Ok(None);
        };
        let after = &url[idx + marker.len()..];
        let seg: String = after
            .chars()
            .take_while(|c| *c != '/' && *c != '?' && *c != '#')
            .collect();
        if seg.is_empty() {
            return Ok(None);
        }
        let rest_after_seg = &after[seg.len()..];
        if rest_after_seg.starts_with('/') {
            let chap_str: String = rest_after_seg[1..]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(chapter) = chap_str.parse::<i32>() {
                return Ok(Some(DeepLinkResult::Chapter {
                    manga_key: seg.into(),
                    key: chapter.to_string().into(),
                }));
            }
        }
        Ok(Some(DeepLinkResult::Manga { key: seg.into() }))
    }
}

impl DynamicSettings for Roumanwu {
    fn get_dynamic_settings(&self) -> Result<Vec<aidoku::Setting>> {
        let current_url = get_base_url();
        Ok(vec![
            aidoku::TextSetting {
                key: "base_url".into(),
                title: "源地址".into(),
                placeholder: Some(BASE_URL.into()),
                default: Some(current_url.into()),
                ..Default::default()
            }
            .into(),
            aidoku::LinkSetting {
                key: "address_link".into(),
                title: "地址发布：https://rdz3.xyz/dizhi".into(),
                url: "https://rdz3.xyz/dizhi".into(),
                external: Some(true),
                ..Default::default()
            }
            .into(),
        ])
    }
}

impl BaseUrlProvider for Roumanwu {
    fn get_base_url(&self) -> Result<String> {
        Ok(get_base_url())
    }
}

// ---------- Image fetch + decode ----------

// Some chapter pages ship as `sr:1` URLs whose rows have been reordered on
// the CDN. We tag those in get_page_list's PageContext and let the app fetch
// each image lazily (one per page render), then unscramble the bytes here.
// The web reader only appears fast because it streams one image at a time
// as the user scrolls; before this change Aidoku did the equivalent of
// pre-loading every scrambled page before opening the chapter.
impl PageImageProcessor for Roumanwu {
    fn process_page_image(
        &self,
        response: ImageResponse,
        context: Option<PageContext>,
    ) -> Result<ImageRef> {
        let needs_unscramble = context
            .as_ref()
            .and_then(|c| c.get("scramble"))
            .is_some_and(|v| v == "1");
        let url = response.request.url.as_deref().unwrap_or("");
        if needs_unscramble {
            if let Some(image) = unscramble_image(url, response.image.data().as_slice()) {
                return Ok(image);
            }
        }
        Ok(response.image)
    }
}

// Pass the URL through unmodified. The existing eager Request::get path in
// get_page_list (now removed) worked without extra headers, so the app's
// default image fetch should too.
impl ImageRequestProvider for Roumanwu {
    fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
        Ok(Request::get(&url)?)
    }
}

#[cfg(test)]
mod test;

register_source!(
    Roumanwu,
    ListingProvider,
    Home,
    DeepLinkHandler,
    DynamicSettings,
    BaseUrlProvider,
    PageImageProcessor,
    ImageRequestProvider
);
