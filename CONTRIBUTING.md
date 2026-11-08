# Contributing

Thanks for considering contributing to MagiDiva! This doc covers what's worth
contributing, how to set up the dev environment, and code conventions.

## What we're most interested in

1. **Model accuracy** — edge cases (glass / hair / fur / semi-transparent).
   Open a PR with before/after screenshots if you tuned parameters.
2. **Platform portability** — we support Windows (Tier 1) and target macOS / Linux.
   Bug fixes and test runs on those platforms are gold.
3. **GPU acceleration verification** — DirectML, CoreML, Vulkan — if you can
   confirm a backend works or fails on specific hardware, document it.
4. **Batch processing UX** — the Pro Edition's gallery view, thumbnail strip,
   cancellation, and progress events have the most room for improvement.
5. **Translations** — Community Edition UI strings (English first) welcome i18n
   contributions.

## What we're less interested in

- Copy-paste UI tweaks (cosmetics without a real UX gain)
- Adding features that only exist in the Pro Edition to the Community build
- Large refactors without a motivating bug / benchmark

## Dev environment

```bash
# 1. Install tools
rustup default stable        # Rust 1.80+
rustup target add x86_64-pc-windows-msvc   # on Windows
node --version               # 20+
pnpm --version               # 9+

# 2. Install deps
pnpm install

# 3. Dev run (frontend + backend auto-relods)
pnpm tauri dev               # Community

# 4. Pro dev
pnpm tauri dev --features pro

# 5. Production builds
pnpm release                 # Community
pnpm pro-release             # Pro (see package.json)
```

## Code conventions

| Layer | Convention |
|---|---|
| Rust | `cargo fmt` + `cargo clippy` — zero warnings in CI |
| Rust · feature gates | Every Pro-only symbol is tagged `#[cfg(feature = "pro")]`. No runtime checks like `if is_pro { ... }` in Rust. |
| TypeScript | `strict: true` in tsconfig |
| React | Function components + hooks. No class components. |
| Tailwind | Hand-rolled classes. No UI component libraries. |
| File paths | Forward slashes everywhere. We test on Windows so we know the pain. |

## PR process

1. Fork → feature branch → commit → push
2. Describe: what you changed, why, how to test
3. Attach a screenshot if the UI changed
4. One maintainer reviews, requests changes or merges

## Issue labels

- `bug` — reproducible crash / incorrect output
- `perf` — slow, memory-heavy, or CPU-hungry
- `model-accuracy` — a specific image where the AI failed
- `pro-only` — only shows up when compiled with `--features pro`
- `enhancement` — feature request we'd be excited about
