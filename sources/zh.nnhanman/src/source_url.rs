use aidoku::alloc::String;
use aidoku::imports::defaults::defaults_get;
use aidoku::imports::net::Request;
use aidoku::Result;

pub(crate) const BASE_URL: &str = "https://nnhm7.com";

pub(crate) const USER_AGENT: &str =
	"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
	 Chrome/120.0.0.0 Safari/537.36";

// Returns the user-configured base URL, or `BASE_URL` when the dynamic
// `base_url` setting hasn't been set. Every URL the source builds
// (search, listing, manga detail, chapter detail, image Referer) goes
// through here so a custom address takes effect across the board.
pub(crate) fn get_base_url() -> String {
	match defaults_get::<String>("base_url") {
		Some(url) if !url.trim().is_empty() => url,
		_ => String::from(BASE_URL),
	}
}

pub(crate) fn html_get_string(url: &str) -> Result<String> {
	Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Accept-Language", "zh-TW,zh;q=0.9,en;q=0.8")
		.string()
}