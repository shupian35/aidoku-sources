//! Content-source selection and the session needed by the secondary one.
//!
//! The site fronts two catalogues behind one origin: `jm`, which is public, and
//! `bika`, which the site only serves to a signed-in 禁漫天堂 account. Aidoku
//! issues every request independently and has no cookie jar, so the session is
//! captured once at login, persisted in UserDefaults, and replayed as a
//! `Cookie` header on each subsequent request.

use aidoku::{
	AidokuError, Result,
	alloc::{String, format},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
	imports::net::Request,
};

use crate::source_url::{USER_AGENT, get_base_url};

/// UserDefaults key holding the selected catalogue.
pub(crate) const FLAVOR_KEY: &str = "flavor";

/// UserDefaults key holding the captured session cookie.
pub(crate) const SESSION_KEY: &str = "session";

/// The public catalogue. No account required.
pub(crate) const FLAVOR_JM: &str = "jm";

/// The secondary catalogue. Requires a signed-in account.
pub(crate) const FLAVOR_BIKA: &str = "bika";

/// The catalogues offered in the source settings, as (value, display name).
pub(crate) const FLAVORS: [(&str, &str); 2] =
	[(FLAVOR_JM, "JM 禁漫天堂"), (FLAVOR_BIKA, "哔咔 (需登录)")];

/// The currently selected catalogue, defaulting to the public one.
pub(crate) fn flavor() -> String {
	match defaults_get::<String>(FLAVOR_KEY).as_deref() {
		Some(FLAVOR_BIKA) => String::from(FLAVOR_BIKA),
		_ => String::from(FLAVOR_JM),
	}
}

/// Whether the selected catalogue needs the session cookie.
pub(crate) fn needs_session() -> bool {
	flavor() == FLAVOR_BIKA
}

/// The stored session cookie, if the user has signed in.
pub(crate) fn session() -> Option<String> {
	defaults_get::<String>(SESSION_KEY).filter(|value| !value.trim().is_empty())
}

/// Signs in with a 禁漫天堂 account and stores the resulting session.
///
/// The site authenticates with `POST /api/site/login` and hands back a cookie;
/// there is no token in the response body, so the cookie is the session.
pub(crate) fn login(username: &str, password: &str) -> Result<()> {
	if username.trim().is_empty() || password.is_empty() {
		return Err(AidokuError::message("请填写用户名和密码"));
	}

	let base = get_base_url();
	let referer = format!("{}/login", base);
	let payload = format!(
		"{{\"username\":\"{}\",\"password\":\"{}\"}}",
		escape(username.trim()),
		escape(password)
	);

	let mut request = Request::post(format!("{}/api/site/login", base))?;
	request.set_header("User-Agent", USER_AGENT);
	request.set_header("Accept", "application/json");
	request.set_header("Content-Type", "application/json");
	request.set_header("Referer", &referer);
	request.set_body(&payload);

	let response = match request.send() {
		Ok(response) => response,
		Err(_) => return Err(AidokuError::message("登录失败：无法连接服务器")),
	};

	if let Some(cookie) = response
		.get_header("Set-Cookie")
		.and_then(|raw| cookie_pair(&raw))
	{
		defaults_set(SESSION_KEY, DefaultValue::String(cookie));
		return Ok(());
	}

	// A 401 here means the credentials were rejected; anything else is a site
	// problem worth reporting verbatim.
	if response.status_code() == 401 || response.status_code() == 403 {
		return Err(AidokuError::message("登录失败：用户名或密码不正确"));
	}
	Err(AidokuError::message("登录失败：服务器未返回会话信息"))
}

/// Reduces a `Set-Cookie` header to the `name=value` pair that goes back out.
pub(crate) fn cookie_pair(raw: &str) -> Option<String> {
	// A response can carry several Set-Cookie headers; only the first pair of
	// the first one is the session, and the attributes after `;` are dropped.
	let first = raw.split(',').next().unwrap_or(raw);
	let pair = first.split(';').next().unwrap_or("").trim();
	let (name, value) = pair.split_once('=')?;
	// A nameless cookie cannot be a session, and replaying one would only mask
	// a mis-parse, so it is rejected rather than forwarded.
	if name.trim().is_empty() || value.is_empty() {
		return None;
	}
	Some(String::from(pair))
}

/// Escapes a value for embedding in the login JSON body.
pub(crate) fn escape(value: &str) -> String {
	let mut out = String::new();
	for ch in value.chars() {
		match ch {
			'"' => out.push_str("\\\""),
			'\\' => out.push_str("\\\\"),
			'\n' | '\r' | '\t' => out.push(' '),
			other => out.push(other),
		}
	}
	out
}
