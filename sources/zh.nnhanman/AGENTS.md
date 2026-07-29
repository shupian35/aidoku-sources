# Repository Guidelines

Contributor guide for the `zh.nnhanman` Aidoku source extension (target site: https://nnhm7.com). The source is a single `no_std` Rust crate compiled to WebAssembly and packaged as an `.aix` module the Aidoku app loads at runtime.

## Project Structure & Module Organization

```
zh.nnhanman/
├── .cargo/config.toml     # Pins target=wasm32-unknown-unknown and runner=aidoku-test-runner
├── res/
│   ├── icon.png           # Source icon — lowercase filename
│   └── source.json        # id, name, version, url, languages, contentRating, listings (UTF-8, no BOM)
├── src/
│   └── lib.rs             # One source struct (`Nnhm7`) plus parsing/HTTP helpers
├── public/                # Generated artifacts; do not edit by hand
│   ├── index.json / index.min.json   # Aggregated source lists
│   ├── sources/*.aix                 # Packaged modules
│   └── icons/*.png                   # Versioned icon copies
├── Cargo.toml             # Crate "nnhanman", edition 2024, cdylib
└── package.aix            # Latest local build
```

## Build, Test, and Development Commands

```bash
cargo build --release --target wasm32-unknown-unknown   # Build .wasm (size-optimized)
cargo test                                              # Run tests via aidoku-test-runner
aidoku package                                          # Wrap .wasm into an .aix
```

`target` is locked in `.cargo/config.toml`, so omit `--target`. Release profile is pinned to `opt-level = "s"`, `lto = true`, `panic = "abort"`, `strip = true` — keep these or the output balloons.

## Coding Style & Naming Conventions

- `#![no_std]`; pull `String`, `Vec`, and `format!` from `aidoku::alloc::*`.
- Follow `rustfmt` defaults (no `rustfmt.toml` is shipped at this level).
- Constants in `SCREAMING_SNAKE_CASE` (`BASE_URL`, `USER_AGENT`); functions and locals in `snake_case`; the source struct in PascalCase matching the URL prefix (`Nnhm7`).
- Name helpers after what they parse (`parse_manga_grid`, `parse_manga_list`, `slug_from_url`, `has_next_page`).
- End `lib.rs` with `register_source!(Nnhm7, …)` listing every Aidoku trait the source implements (`Source`, `ListingProvider`, `Home`, `DynamicFilters`, `DeepLinkHandler`).

## Testing Guidelines

- Framework: `aidoku-test` (dev-dependency) executed by `aidoku-test-runner`, set as the wasm runner in `.cargo/config.toml`.
- Put tests in `src/lib.rs` or a sibling `src/test.rs`; name each test by the behavior under test (e.g. `fn search_returns_results`).
- Run with `cargo test` from this directory. Tests execute inside the wasm sandbox — avoid host-only APIs (`std::fs`, `std::net`, etc.).

## Commit & Pull Request Guidelines

History mixes Conventional Commits (`refactor: rename source ...`, `chore: add .gitignore`) and short Chinese summaries (`修复乱码`, `保存为utf-8`). Either is fine, but:

- Subject ≤ 50 characters; imperative mood for English commits.
- Reference issues when applicable (`Fix #123: …`).
- Never commit `public/` or `target/`; they are regenerated.

PRs should describe the change, link related issues, run `cargo build` and `cargo test` locally, and paste the regenerated `public/index.json` entry whenever `res/source.json` changes.

## Source-Specific Notes

- `BASE_URL` and `USER_AGENT` live at the top of `src/lib.rs`; update them together if the upstream site changes.
- Manga keys are slugs derived from `/comic/<slug>.html` URLs via `slug_from_url` — keep them slugs; `DeepLinkHandler` and listings depend on them.
- Deep links of the form `/comic/<slug>/chapter-<n>` are routed by `DeepLinkHandler::handle_deep_link`.
- `contentRating` is `2` (NSFW); do not change it without owner approval.
- **Filter-driven browse via `get_search_manga_list`.** The source publishes no top-level listings (`res/source.json` has `listings: []`) and is registered with `DynamicFilters`, `Home`, and `DeepLinkHandler` only (no `ListingProvider`). On first open Aidoku routes through `__wasm_get_search_manga_list(query=None, filters=[…default…], page=1)`, and the source dispatches by a top-level `kind` SelectFilter to one of three roots:

  | `kind` value | URL | page | parser |
  |---|---|---|---|
  | `latest` (最新更新) | `/update` | single | `parse_manga_list` (cards: `div.itemBox`) |
  | `ranking` (排行榜) | `/ranking` (总榜) | single | `parse_manga_list` |
  | `all` (全部分类, default) | `/comics/all/{category}/{sort}/st/{status}/page/{n}` | paginated | `parse_manga_grid` (grid: `ul.col_3_1 > li`) |

  The remaining `category` / `status` / `sort` filters are only consulted when `kind == "all"`; they are silently ignored for `latest` and `ranking`. Single-page roots swallow parser errors via `unwrap_or_default()` so a layout change degrades to an empty list instead of erroring. Bump `info.version` in `res/source.json` whenever the filter set or any fetched URL changes.

  The `kind` SelectFilter uses `ids: ["all", "latest", "ranking"]` and `default: Some("all")` so dispatch keys are stable even if the display strings change.
- `parse_manga_list` reads `div.itemBox` cards; `parse_manga_grid` reads `ul.col_3_1 > li` items with `a.ImgA` / `a.txtA`. Don't mix the two on the same page — pick the one that matches the upstream markup. The two URL groups above each use a single consistent layout.