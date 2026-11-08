# ✨ MagiDiva

> **AI Product Image Magic · 100% local · zero cloud · for e-commerce sellers**

Your images never leave your computer. Forever free for single-image work. Upgrade to Pro for batch processing, folder scanning, and cancellation.

![License](https://img.shields.io/badge/license-AGPLv3-6c5ce7.svg)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-00cec9.svg)
![Rust](https://img.shields.io/badge/rust-1.80+-orange.svg)
![Tauri](https://img.shields.io/badge/tauri-v2-7e7e86.svg)

---

## What it does

MagiDiva removes backgrounds from product photos, auto-centers them on preset canvas sizes (Shopify / Amazon / Etsy / Custom), and composites a soft drop shadow with a clean custom background color — all inside a single desktop app.

- 🖼️ **Local AI** — BiRefNet / ISNet / U2Net run on your machine via ONNX Runtime + DirectML (Windows GPU acceleration auto-detected)
- 📦 **Batch Processing (Pro)** — Drop a folder with subdirectories, we walk it, dedupe, extract every image, and emit per-file progress events
- 🎨 **E-commerce workflow** — 1-click white background, custom RGB background, 4 shadow styles (Soft / Medium / Strong / Offset), watermark with 9 anchor positions
- 📐 **Size presets** — 8 ready-made presets (Shopify 2048², Amazon Main, Etsy 2000², Square 800²...) + auto-center that preserves relative product position
- 💾 **Output formats** — PNG (lossless), JPG (quality slider 60–100), WebP (lossless)
- 🖱️ **Native desktop UX** — Tauri v2 custom title bar, double-click to maximize, drag-drop files, gallery view, thumbnail strip

---

## Community vs Pro

| Feature | Community | Pro |
|---|---|---|
| Single-image processing | ✅ | ✅ |
| All synthesis (shadow / watermark / bg color / format) | ✅ | ✅ |
| DirectML GPU acceleration | ✅ | ✅ |
| Batch processing | ❌ | ✅ |
| Folder recursive scan | ❌ | ✅ |
| Mid-batch cancellation | ❌ | ✅ |
| Gallery + thumbnail strip | Basic | ✅ |
| License | **Free, source-available** | **$49 perpetual license** |

> Pricing details at [magidiva.com/pricing](https://magidiva.com/pricing)

---

## Download

Pre-built binaries are shipped in GitHub Releases:

| Platform | Community | Pro |
|---|---|---|
| Windows x64 | `magidiva-community-x64-setup.exe` | `magidiva-pro-x64-setup.exe` |
| macOS ARM64 | `magidiva-community-arm64.dmg` | `magidiva-pro-arm64.dmg` |
| Linux x64 | `magidiva-community-x86_64.AppImage` | `magidiva-pro-x86_64.AppImage` |

Check the [Releases](../../releases) tab.

---

## Build from source

### Prerequisites

- Rust 1.80+ (toolchain via `rustup`)
- Node.js 20+ + pnpm 9+
- Windows: MSVC toolchain + WebView2 Runtime (bundled on Win10+)
- macOS: Xcode command-line tools
- Linux: webkit2gtk-4.1, libayatana-appindicator3, libxdo, libgtk-3, libsoup-3

```bash
# One-time
pnpm install

# Community (default)
pnpm release          # exe only, no bundling
pnpm installer        # full NSIS/MSI/DMG bundle

# Pro (enables batch_process + walk_paths + cancel_batch)
pnpm pro-release      # → tauri build --features pro --no-bundle
pnpm pro-installer    # → tauri build --features pro
```

Want raw Cargo?

```bash
cargo build --release               # Community
cargo build --release --features pro # Pro
```

---

## Tech stack

| Layer | Choice | Why |
|---|---|---|
| Frontend | React 19 + Vite 8 + TypeScript | Fast HMR, zero-config |
| UI | Tailwind CSS 3 + hand-rolled components | No UI framework bloat, custom title bar |
| Backend | Rust + Tauri v2 + ONNX Runtime v2 | Rust safety, Tauri's small bundle (~8 MB), ONNX cross-platform AI inference |
| AI models | BiRefNet-General-Lite (default) · ISNet-General · U2Net | open-weight, MIT-friendly license, covers speed/quality/size tradeoffs |
| GPU | DirectML (Windows auto), CoreML (macOS), Vulkan (Linux) | picked by ort crate features |

---

## Architecture

```
┌───────────────────┐     invoke()     ┌──────────────────────┐
│  React frontend   │ ───────────────→ │  Rust backend        │
│  (App.tsx, 265 KB │                  │  (lib.rs, ~300 KB)   │
│   gzipped 81 KB)  │ ←─────────────── │  + ONNX Runtime      │
└───────────────────┘   batch-progress │  + DirectML         │
                        events          └──────────────────────┘
                                              │
                              ┌───────────────┼───────────────┐
                              ▼               ▼               ▼
                        model.rs        bg_remover.rs    processor.rs
                     (list/download)  (inferring)     (center/shadow/
                                                      watermark/encode)
```

Pro features are isolated behind `#[cfg(feature = "pro")]` — they vanish from the Community binary entirely (no dead code bloat).

---

## License

**Community**: AGPLv3 — source-available, use freely, modify and redistribute changes under the same license.

**Pro**: Commercial license — perpetual, 1 seat = 1 developer, bundle Pro binary into your workflow. Purchase at [magidiva.com/pro](https://magidiva.com/pro).

Full text in [LICENSE-AGPL](LICENSE-AGPL) (Community) and [LICENSE-COMMERCIAL](LICENSE-COMMERCIAL) (Pro).

---

## Contributing

Bug reports, model accuracy improvements, and platform portability fixes welcome via issues.

Code contributions:

1. Fork the repo
2. Create a feature branch: `git checkout -b feature-your-idea`
3. Open a PR targeting `main`

See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

---

## Acknowledgements

- [BiRefNet](https://github.com/zhengpeng7/BiRefNet) — the default model
- [U2Net](https://github.com/xuebinqin/U-2-Net) — classic baseline
- [ISNet](https://github.com/xuebinqin/DIS) — detail-preserving alternative
- [ONNX Runtime](https://github.com/microsoft/onnxruntime) by Microsoft
- [Tauri](https://github.com/tauri-apps/tauri) framework

---

Made with ✨ by the MagiDiva team.
