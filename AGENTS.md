# Repository Guidelines

This repository contains an Aidoku source extension for the rouman5.com manga website. The project is written in Rust and compiles to a WebAssembly module that integrates with the Aidoku manga reading app.

## Project Structure & Module Organization

```
aidoku-sources/
├── src/
│   └── lib.rs          # Main source implementation
├── res/
│   ├── icon.png        # Source icon
│   └── source.json     # Source metadata (id, name, url, languages)
├── public/
│   ├── index.json      # Public registry manifest
│   ├── icons/          # Generated source icons
│   └── sources/        # Compiled .aix packages
├── Cargo.toml          # Rust project configuration
└── Cargo.lock          # Dependency lock file
```

The main source code is in `src/lib.rs`, which implements the Aidoku `Source` trait for the rouman5.com website.

## Build, Test, and Development Commands

```bash
# Build the WebAssembly module
cargo build --target wasm32-unknown-unknown --release

# Run tests
cargo test

# Build for development (with debug info)
cargo build

# Clean build artifacts
cargo clean
```

## Coding Style & Naming Conventions

- Use Rust standard style (rustfmt)
- Prefer `snake_case` for functions and variables
- Use `SCREAMING_SNAKE_CASE` for constants
- Keep functions focused and under 50 lines when possible
- Use descriptive names for HTTP helper functions
- Add comments for complex parsing logic

## Testing Guidelines

- Test framework: Built-in Rust testing with `aidoku-test`
- Run tests: `cargo test`
- Test files should be in the same module as the code they test
- Use descriptive test function names that explain the scenario
- Mock HTTP requests when testing network-dependent code

## Commit & Pull Request Guidelines

### Commit Messages
- Use imperative mood: "Add feature" not "Added feature"
- Keep subject line under 50 characters
- Reference issues when applicable: "Fix #123: Handle empty manga list"

### Pull Requests
- Include description of changes
- Link related issues
- Test locally before submitting
- Ensure CI passes (if configured)

## Source Development Notes

- The source implements Aidoku's `Source` trait for web scraping
- Uses `reqwest` for HTTP requests and `scraper` for HTML parsing
- Content rating is set to `NSFW` (rating: 2)
- Primary language: Chinese (zh)
- Base URL: https://rouman5.com
