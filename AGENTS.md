# AGENTS.md

## Repository Guidelines

This repository contains Aidoku source extensions for manga websites. The project supports multiple sources, each compiled to WebAssembly modules that integrate with the Aidoku manga reading app.

## Project Structure & Module Organization

```
aidoku-sources/
├── src/rust/                    # Source implementations
│   └── zh.rouman5/             # rouman5.com source
│       ├── .cargo/             # Cargo configuration
│       ├── res/                # Source resources (source.json, Icon.png)
│       ├── src/                # Rust source code
│       │   ├── lib.rs          # Main source implementation
│       │   └── utils.rs        # Utility functions
│       ├── Cargo.toml          # Rust project configuration
│       ├── Cargo.lock          # Dependency lock file
│       ├── build.sh            # Build script for this source
│       └── rustfmt.toml        # Rust formatting config
├── public/                      # Generated public directory
│   ├── index.json              # Source list (pretty)
│   ├── index.min.json          # Source list (minified)
│   ├── sources/                # Compiled .aix packages
│   └── icons/                  # Source icons
├── .github/workflows/          # GitHub Actions CI/CD
│   └── build.yaml              # Build workflow
├── build.ps1                   # Windows build script
├── README.md                   # Project documentation
└── AGENTS.md                   # This file
```

## Build, Test, and Development Commands

### Build single source
```bash
# Navigate to source directory
cd src/rust/zh.rouman5

# Build the WebAssembly module
cargo build --release --target wasm32-unknown-unknown

# Run tests
cargo test

# Create .aix package
aidoku package
```

### Build all sources (Windows)
```bash
# Build all sources and generate public directory
./build.ps1

# Build specific source
./build.ps1 -SourceName "zh.rouman5"
```

### Start local server
```bash
# Serve a specific source
aidoku serve src/rust/zh.rouman5/package.aix

# Serve from public directory
aidoku serve
```

## Adding a New Source

1. Create a new directory under `src/rust/` with the source ID (e.g., `src/rust/zh.example/`)
2. Copy the structure from an existing source (e.g., `zh.rouman5`)
3. Update `res/source.json` with the new source metadata
4. Implement the source in `src/lib.rs`
5. Add the source icon as `res/Icon.png`
6. Run `./build.ps1` to build and aggregate all sources

## Coding Style & Naming Conventions

- Use Rust standard style (rustfmt)
- Prefer `snake_case` for functions and variables
- Use `SCREAMING_SNAKE_CASE` for constants
- Keep functions focused and under 50 lines when possible
- Use descriptive names for HTTP helper functions
- Add comments for complex parsing logic

## Source Development Notes

- Each source implements Aidoku's `Source` trait for web scraping
- Uses `reqwest` for HTTP requests and `scraper` for HTML parsing
- Content rating: `NSFW` (rating: 2) or `SFW` (rating: 1)
- Primary language follows the source ID prefix (e.g., `zh` for Chinese)

## Commit & Pull Request Guidelines

### Commit Messages
- Use imperative mood: "Add feature" not "Added feature"
- Keep subject line under 50 characters
- Reference issues when applicable: "Fix #123: Handle empty manga list"

### Pull Requests
- Include description of changes
- Link related issues
- Test locally before submitting
- Ensure CI passes
