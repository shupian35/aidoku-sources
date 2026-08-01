use aidoku::alloc::String;
use aidoku::imports::net::Request;
use aidoku::Result;

pub(crate) const BASE_URL: &str = "https://nnhm7.com";

pub(crate) const USER_AGENT: &str =
	"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
	 Chrome/120.0.0.0 Safari/537.36";

pub(crate) fn html_get_string(url: &str) -> Result<String> {
	Request::get(url)?
		.header("User-Agent", USER_AGENT)
		.header("Accept-Language", "zh-TW,zh;q=0.9,en;q=0.8")
		.string()
}