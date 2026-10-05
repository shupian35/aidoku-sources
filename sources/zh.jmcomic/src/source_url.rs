use aidoku::{
	Result,
	alloc::{String, string::ToString},
	imports::net::Request,
};

pub(crate) const BASE_URL: &str = "https://web.jmcomic.uk";

pub(crate) const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
	 Chrome/126.0.0.0 Safari/537.36";

// Returns the effective base URL, or `BASE_URL` when no override is set.
// Every URL the source builds goes through here, so a custom address takes
// effect across search, listings, details, chapters and deep links alike.
//
// Priority: the source's own "自定义网址" text setting (the `base_url`
// defaults key) is the user's explicit override, so it wins over the
// app-generated Base URL picker (`config.allowsBaseUrlSelect` + `info.urls`
// in source.json, which the app exposes as the `url` defaults key). The
// picker always registers a default value, so it has to come second.
pub(crate) fn get_base_url() -> String {
	match aidoku::imports::defaults::defaults_get::<String>("base_url") {
		Some(url) if !url.trim().is_empty() => url.trim_end_matches('/').to_string(),
		_ => match aidoku::imports::defaults::defaults_get::<String>("url") {
			Some(url) if !url.trim().is_empty() => url.trim_end_matches('/').to_string(),
			_ => String::from(BASE_URL),
		},
	}
}

// Page and cover images are served behind a hotlink check that rejects
// requests without a browser User-Agent, so every image request is built here.
pub(crate) fn image_request(url: &str) -> Result<Request> {
	Ok(Request::get(url)?.header("User-Agent", USER_AGENT))
}
