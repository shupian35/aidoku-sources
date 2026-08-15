use aidoku::alloc::String;
use aidoku::imports::defaults::defaults_get;
use aidoku::imports::net::Request;
use aidoku::Result;

pub(crate) const BASE_URL: &str = "https://nnhm7.com";

pub(crate) const USER_AGENT: &str =
	"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
	 Chrome/120.0.0.0 Safari/537.36";

// Returns the effective base URL, or `BASE_URL` when no override is set.
// Every URL the source builds (search, listing, manga detail, chapter
// detail, image Referer) goes through here so a custom address takes
// effect across the board.
//
// Priority: the source's own "自定义网址" text setting (the `base_url`
// defaults key) is the user's explicit override, so it wins over the
// app-generated Base URL picker (`config.allowsBaseUrlSelect` + `info.urls`
// in source.json, which the app exposes as the `url` defaults key). The
// picker always registers a default value, so it must come second — with
// the old order a typed custom URL would never take effect. `BASE_URL` is
// the final fallback.
pub(crate) fn get_base_url() -> String {
	match defaults_get::<String>("base_url") {
		Some(url) if !url.trim().is_empty() => url,
		_ => match defaults_get::<String>("url") {
			Some(url) if !url.trim().is_empty() => url,
			_ => String::from(BASE_URL),
		},
	}
}

pub(crate) fn html_get_string(url: &str) -> Result<String> {
	Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Accept-Language", "zh-TW,zh;q=0.9,en;q=0.8")
		.string()
}