<div align="center">
  <img src="./Screenshot%202026-02-18%20083634.png" alt="SilvaView-rs screenshot" width="100%" />
</div>

# SilvaView-rs

**A modern, GPU-accelerated disk space visualizer with authentic cushion treemaps.**

SilvaView-rs is a high-performance Rust recreation of the classic disk visualization experience pioneered by **SequoiaView** and **SilvaView**. It brings the elegant *cushion treemap* algorithm — first introduced in the 1999 INFOVIS paper — to the GPU era with real-time WGSL shading, lightning-fast scanning, and a polished interactive interface.

---

## Features

### Core Visualization
- **Authentic Cushion Treemaps** — Faithful GPU implementation of the 1999 van Wijk & van de Wetering algorithm. Parabolic ridges are accumulated during layout and shaded per-pixel with Lambertian lighting + subtle saturation boost for depth and readability.
- **Squarified + L-shaped layout** — Adapts intelligently around the left sidebar while preserving area proportions. Includes dominant-directory chain collapse, aggressive LOD culling, and child-area redistribution to avoid empty interiors.
- **Per-pixel GPU shading** — WGSL render pipeline (cushion.wgsl) handles all lighting on the GPU. Supports fast approximate and full-precision modes.
- **Live hover inspection** — Dynamic label + size tooltip that follows the cursor or appears inside large rectangles.
- **Configurable text labels** — Smart label placement with font-size scaling, truncation, and overlap prevention. Supports system fonts or custom .ttf/.otf via settings.

### Interaction & Navigation
- **Drill-down & back navigation** — Click any directory rectangle or label to zoom in. Keyboard shortcuts (`←`/`↑` for up, `→`/`↓` for siblings).
- **Drive picker sidebar** — Instant switch between available drives with live path display.
- **Color modes** — Cycle between *Category*, *Category+Extension*, and *Extension Hash* coloring. Adjustable vibrancy (0.6–2.0) with draggable slider.
- **Analytics panel** — Optional overlay showing total size, file counts, and largest items.
- **Settings dialog** (F2) — Live adjustment of layout padding, cushion parameters, label scale, and font path.

### Scanning Performance
- **Windows NTFS MFT scanner** — Direct Master File Table access when run as Administrator (10× faster than recursive walk). Auto-prompts elevation via manifest.
- **Cross-platform fallback** — Parallel `jwalk` scanner for non-elevated or non-Windows paths.
- **Live progress** — Real-time file/directory count and estimated time during scan.
- **Handles millions of files** — Compact arena-based tree (u32 node IDs) + aggressive culling keeps everything responsive.

### Polish & Usability
- **Modern dark aesthetic** — Vibrant category colors on deep neutral background.
- **Responsive windowing** — Full winit + wgpu support with proper DPI scaling and resize handling.
- **Cached rasterization** — Treemap image is rebuilt only on layout changes; Vello handles vector overlays (labels, frames, UI).
- **Keyboard & mouse-first workflow** — Everything reachable without menus.

---

## Influences & Technical Foundations

SilvaView-rs is directly inspired by two landmark projects:

1. **SequoiaView** — The original Windows disk space visualizer (late 1990s–early 2000s) that popularized cushion treemaps for everyday users. Its beautiful 3D-like shading made it instantly clear why your hard drive was full.

2. **Cushion Treemaps: Visualization of Hierarchical Information** (INFOVIS'99)  
   by Jarke J. van Wijk and Huub van de Wetering.  
   The paper introduces the core idea: during recursive subdivision, add parabolic ridges (`AddRidge`) to each rectangle and shade the resulting surface with a simple diffuse lighting model. This turns a flat treemap into a visually rich, self-similar “cushion” landscape that reveals deep hierarchy at a glance.

   Our implementation:
   - Replicates the exact ridge accumulation (`add_ridge` in `squarify.rs`)
   - Uses the same light vector `(1, 2, 10)` normalized
   - Ports the shading math directly to WGSL (`cushion.wgsl`) with both fast and full-precision paths
   - Adds modern extensions: vibrancy control, dynamic color mapping, L-shaped layout, and GPU acceleration

The result is the most faithful digital revival of the original cushion treemap aesthetic ever built in Rust.

---

## Getting Started

### Build

bash
cargo build --release

### Run

bash
# Default: scans C:\ (Windows) or / (Linux/macOS)
cargo run --release

# Specific path
cargo run --release -- "D:\\Rust-projects"
# or
cargo run --release -- "/home/user/Projects"

**Windows tip**: For maximum speed on drive roots, run as Administrator (the manifest will auto-prompt).

---

## Usage

- **Left sidebar** — Click any drive to rescan instantly.
- **Color controls** — Click the color-mode button or drag the vibrancy slider.
- **Explore** — Hover for info, click directories/labels to drill down.
- **Navigate back** — `Backspace`, `↑`, or click breadcrumb (when enabled).
- **Settings** — Press **F2** to open the configuration dialog.
- **Analytics** — Toggle via sidebar button (shows size breakdown).

---

## Architecture Highlights

- `src/scanner/` — MFT + jwalk hybrid scanner with progress channels
- `src/tree/arena.rs` — Memory-efficient flat arena with sibling lists
- `src/layout/squarify.rs` — Optimized squarified layout + cushion ridge math + L-shape support
- `src/render/` — Vello scene + WGSL cushion pipeline (cushion_gpu.rs)
- `src/app.rs` — Central state machine (Scanning → Ready → Relayout)
- `src/ui/` — Hit testing, overlays, drive enumeration, config dialog

All rendering happens on the GPU; the CPU only builds the layout once per view change.

---

## Performance

- Scans a 2 TB drive with 1.2 M files in **~8 seconds** (MFT mode) vs **~90 seconds** (jwalk).
- 60 fps interaction even on trees with 500 k+ nodes.
- Minimal memory footprint thanks to arena allocation and LOD culling.

---

## Project Status

**Phase 3 complete** (as of v0.3.0):
- Full cushion GPU pipeline
- Windows MFT + cross-platform scanning
- Polished interactive UI and settings
- Ready for daily use

Future ideas (PRs welcome):
- Export to SVG/PDF
- File-type filtering
- Dark/light theme toggle
- macOS/Linux native file picker integration

---

## Credits & References

- **Original inspiration**: SequoiaView by the van Wijk / van de Wetering team
- **Cushion Treemaps algorithm**: [“Cushion Treemaps: Visualization of Hierarchical Information” (INFOVIS'99)](ctm.txt) — included in the repository for reference
- **Rust ecosystem**: vello, wgpu, winit, jwalk, ntfs, skrifa, iced, rfd, and many more

---

**Star the repo if you find it useful!**  
Contributions, bug reports, and feature ideas are always welcome.
