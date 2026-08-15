//! Source identity and HTTP entry point.
//!
//! Every provider impl reaches the live site through [`html_get_string`],
//! which builds a `Request` with the user-agent headers this source has
//! historically required.

use aidoku::Result;
use aidoku::alloc::String;
use aidoku::imports::defaults::defaults_get;
use aidoku::imports::net::Request;

pub(crate) const BASE_URL: &str = "https://rouman5.com";

pub(crate) const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
	(KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub(crate) fn get_base_url() -> String {
    // Official base-URL mechanism: with `config.allowsBaseUrlSelect` in
    // source.json, the app stores the user's pick from `info.urls` under
    // the `url` defaults key. Fall back to the pre-v15 `base_url` dynamic
    // setting (kept so users who customized it keep their override), then
    // to the constant.
    match defaults_get::<String>("url") {
        Some(url) if !url.trim().is_empty() => url,
        _ => match defaults_get::<String>("base_url") {
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
