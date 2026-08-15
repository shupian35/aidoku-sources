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
    // Priority: the source's own "自定义网址" text setting (the `base_url`
    // defaults key) is the user's explicit override, so it wins over the
    // app-generated Base URL picker (`config.allowsBaseUrlSelect` +
    // `info.urls` in source.json, which the app exposes as the `url`
    // defaults key). The picker always registers a default value, so it
    // must come second — with the old order a typed custom URL would never
    // take effect. `BASE_URL` is the final fallback.
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
