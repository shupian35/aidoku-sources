//! Typed client for the site's `/api` JSON endpoints.
//!
//! The upstream normalises most payloads, but a few endpoints still return
//! raw upstream shapes, and numeric ids sometimes arrive as JSON numbers and
//! sometimes as strings. Every field is therefore optional and the string
//! helpers accept numbers, booleans and arrays of strings as well.
//!
//! Endpoints are namespaced by catalogue: `/api/v2/jm/…` or `/api/v2/bika/…`.
//! The segment is resolved per request from the user's selection, and the
//! secondary catalogue additionally needs the stored session cookie.

use aidoku::{
	Result,
	alloc::{String, Vec, format},
	helpers::uri::encode_uri,
	imports::net::Request,
	serde::{Deserialize, de::DeserializeOwned},
};

use crate::flavor;
use crate::source_url::{USER_AGENT, get_base_url};

/// Builds a namespaced endpoint URL for the selected catalogue.
fn v2(path: &str) -> String {
	format!("{}/api/v2/{}/{}", get_base_url(), flavor::flavor(), path)
}

/// The API's standard `{data, msg, st}` envelope.
#[derive(Deserialize)]
struct Envelope<T> {
	data: Option<T>,
	#[serde(default)]
	msg: String,
	st: i64,
}

/// A `{id, title}` reference used for categories.
#[derive(Deserialize, Default, Clone)]
pub(crate) struct CategoryRef {
	#[serde(default, deserialize_with = "flex_string")]
	pub title: String,
}

/// A listing entry as returned by search, leaderboard and random.
#[derive(Deserialize, Default)]
pub(crate) struct Summary {
	#[serde(default, deserialize_with = "flex_string")]
	pub comic_id: String,
	#[serde(default, deserialize_with = "flex_string")]
	pub title: String,
	#[serde(default, deserialize_with = "flex_option_string")]
	pub author: Option<String>,
	#[serde(default)]
	pub cover_url: Option<String>,
	#[serde(default, deserialize_with = "flex_string_list")]
	pub tags: Vec<String>,
}

/// The `/api/latest` payload keeps the upstream field names.
#[derive(Deserialize, Default)]
pub(crate) struct LatestItem {
	#[serde(default, deserialize_with = "flex_string")]
	pub id: String,
	#[serde(default, deserialize_with = "flex_string")]
	pub name: String,
	#[serde(default, deserialize_with = "flex_option_string")]
	pub author: Option<String>,
	#[serde(default)]
	pub image: Option<String>,
	#[serde(default)]
	pub category: Option<CategoryRef>,
}

/// A top-level category. The nested `sub_categories` are not modelled: the
/// leaderboard silently ignores their ids and slugs alike.
#[derive(Deserialize, Default)]
pub(crate) struct Category {
	#[serde(default, deserialize_with = "flex_string")]
	pub id: String,
	#[serde(default, deserialize_with = "flex_string")]
	pub name: String,
	/// The leaderboard only accepts slugs here — numeric ids are ignored.
	#[serde(default, deserialize_with = "flex_string")]
	pub slug: String,
}

#[derive(Deserialize, Default, Clone)]
pub(crate) struct ChapterEntry {
	#[serde(default, deserialize_with = "flex_string")]
	pub id: String,
	#[serde(default, deserialize_with = "flex_string")]
	pub title: String,
	#[serde(default)]
	pub order: i64,
}

#[derive(Deserialize, Default)]
pub(crate) struct Comic {
	#[serde(default, deserialize_with = "flex_string")]
	pub comic_id: String,
	#[serde(default, deserialize_with = "flex_string")]
	pub title: String,
	#[serde(default, deserialize_with = "flex_option_string")]
	pub author: Option<String>,
	#[serde(default)]
	pub cover_url: Option<String>,
	#[serde(default)]
	pub description: Option<String>,
	#[serde(default, deserialize_with = "flex_string_list")]
	pub tags: Vec<String>,
	#[serde(default, rename = "chapters")]
	raw_chapters: Vec<ChapterEntry>,
}

impl Comic {
	pub(crate) fn chapters(&self) -> Vec<ChapterEntry> {
		self.raw_chapters.clone()
	}
}

#[derive(Deserialize, Default, Clone)]
pub(crate) struct PageEntry {
	#[serde(default, deserialize_with = "flex_string")]
	pub name: String,
}

#[derive(Deserialize, Default)]
struct ChapterRaw {
	#[serde(default, deserialize_with = "flex_string")]
	album_id: String,
	#[serde(default, deserialize_with = "flex_string")]
	scramble_id: String,
	#[serde(default, deserialize_with = "flex_string_list")]
	images: Vec<String>,
}

#[derive(Deserialize, Default)]
pub(crate) struct ChapterPages {
	#[serde(default)]
	pub images: Vec<PageEntry>,
	#[serde(default)]
	raw: ChapterRaw,
}

impl ChapterPages {
	pub(crate) fn album_id(&self) -> &str {
		&self.raw.album_id
	}

	pub(crate) fn scramble_id(&self) -> &str {
		&self.raw.scramble_id
	}

	pub(crate) fn fallback_names(&self) -> &[String] {
		&self.raw.images
	}
}

/// Queries the search endpoint. `q` is matched against title, author and id.
pub(crate) fn search(query: &str, page: i32) -> Result<Vec<Summary>> {
	let url = format!("{}?q={}&page={}", v2("search"), encode_uri(query), page);
	unwrap(Envelope::<Vec<Summary>>::fetch(&url)?)
}

/// Fetches a category listing. `category` `"0"` means "all".
pub(crate) fn leaderboard(sort: &str, category: &str, page: i32) -> Result<Vec<Summary>> {
	let url = format!(
		"{}?sort={}&category={}&page={}",
		v2("leaderboard"),
		encode_uri(sort),
		encode_uri(category),
		page
	);
	unwrap(Envelope::<Vec<Summary>>::fetch(&url)?)
}

/// Returns ten random entries.
pub(crate) fn random() -> Result<Vec<Summary>> {
	unwrap(Envelope::<Vec<Summary>>::fetch(&v2("random"))?)
}

/// Fetches the most recently uploaded albums.
///
/// This endpoint is not namespaced and is not enveloped: it is the site's own
/// feed, which only ever carries the public catalogue.
pub(crate) fn latest(page: i32) -> Result<Vec<LatestItem>> {
	let url = format!("{}/api/latest?page={}", get_base_url(), page);
	get_json::<Vec<LatestItem>>(&url)
}

/// Fetches the full category tree.
pub(crate) fn categories() -> Result<Vec<Category>> {
	unwrap(Envelope::<Vec<Category>>::fetch(&v2("categories"))?)
}

/// Fetches details and the chapter list for one album.
pub(crate) fn comic(id: &str) -> Result<Comic> {
	let url = v2(&format!("comic/{}", encode_uri(id)));
	unwrap(Envelope::<Comic>::fetch(&url)?)
}

/// Fetches the page list for one chapter.
pub(crate) fn chapter(id: &str) -> Result<ChapterPages> {
	let url = v2(&format!("chapter/{}", encode_uri(id)));
	unwrap(Envelope::<ChapterPages>::fetch(&url)?)
}

/// The public web URL for an album, used for deep links.
pub(crate) fn album_url(id: &str) -> String {
	format!("{}/comic/{}", get_base_url(), id)
}

// ---------- Transport ----------

impl<T: DeserializeOwned> Envelope<T> {
	fn fetch(url: &str) -> Result<Self> {
		get_json(url)
	}
}

fn get_json<T: DeserializeOwned>(url: &str) -> Result<T> {
	let base = get_base_url();
	let referer = format!("{}/", base);
	let mut request = Request::get(url)?;
	request.set_header("User-Agent", USER_AGENT);
	request.set_header("Accept", "application/json");
	request.set_header("Referer", &referer);
	// The secondary catalogue is only served to a signed-in session, and
	// Aidoku keeps no cookies between requests, so the stored one is replayed
	// on every call.
	if let Some(cookie) = flavor::session() {
		request.set_header("Cookie", &cookie);
	}
	request.json_owned()
}

/// Unwraps an envelope, turning the API's own error string into a Rust error.
///
/// The upstream's "login expired" message is replaced with one that says what
/// the reader actually has to do about it.
fn unwrap<T>(envelope: Envelope<T>) -> Result<T> {
	match envelope.data {
		Some(data) => Ok(data),
		None => {
			let msg = if envelope.msg.is_empty() {
				format!("upstream returned st={}", envelope.st)
			} else {
				envelope.msg
			};
			if flavor::needs_session() {
				return Err(aidoku::AidokuError::message(
					if flavor::session().is_some() {
						"哔咔登录已失效，请在源设置中重新登录"
					} else {
						"哔咔需要登录，请在源设置中登录禁漫账号"
					},
				));
			}
			Err(aidoku::AidokuError::message(msg))
		}
	}
}

// ---------- Lenient scalar decoding ----------

/// A JSON scalar in any of the forms this API uses for ids and names.
#[derive(Deserialize)]
#[serde(untagged)]
enum Scalar {
	Text(String),
	Int(i64),
	Float(f64),
	Flag(bool),
}

impl Scalar {
	fn render(&self) -> String {
		match self {
			Self::Text(text) => text.clone(),
			Self::Int(number) => format!("{}", number),
			Self::Float(number) => format_float(*number),
			Self::Flag(flag) => format!("{}", flag),
		}
	}
}

/// Formats a number without `f64::fract`, which core does not provide.
fn format_float(number: f64) -> String {
	// The ids and counts here are whole numbers far inside the exact range.
	if number == (number as i64) as f64 {
		format!("{}", number as i64)
	} else {
		format!("{}", number)
	}
}

/// Accepts a string, number or boolean wherever a string is expected.
fn flex_string<'de, D: serde::Deserializer<'de>>(de: D) -> core::result::Result<String, D::Error> {
	Ok(match Option::<Scalar>::deserialize(de)? {
		None => String::new(),
		Some(value) => value.render(),
	})
}

/// Same as [`flex_string`] for optional fields.
fn flex_option_string<'de, D: serde::Deserializer<'de>>(
	de: D,
) -> core::result::Result<Option<String>, D::Error> {
	Ok(Option::<Scalar>::deserialize(de)?.map(|value| value.render()))
}

/// Same as [`flex_string`] but joins arrays, which upstream uses for
/// multi-author entries. A bare scalar is treated as no list.
fn flex_string_list<'de, D: serde::Deserializer<'de>>(
	de: D,
) -> core::result::Result<Vec<String>, D::Error> {
	#[derive(Deserialize)]
	#[serde(untagged)]
	enum Flex {
		List(Vec<Scalar>),
		/// Matched but discarded, so a non-list value yields an empty vec.
		Other(#[allow(dead_code)] Scalar),
	}

	Ok(match Option::<Flex>::deserialize(de)? {
		None | Some(Flex::Other(_)) => Vec::new(),
		Some(Flex::List(items)) => items.iter().map(Scalar::render).collect(),
	})
}
