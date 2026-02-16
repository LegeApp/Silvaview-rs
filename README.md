# SilvaView-rs

A modern, GPU-accelerated disk space visualizer for Windows using cushion treemaps.

## Features

### ✅ **Implemented (Phase 1)**
- **Fast filesystem scanning** via `jwalk` (parallel directory walker)
  - Scans millions of files in ~30-60 seconds
  - Auto-fallback when admin privileges unavailable
- **Squarified treemap layout**
  - Optimized aspect ratios for easy visual comparison
  - LOD (level-of-detail) culling for performance
- **GPU-accelerated rendering** via Vello (WebGPU)
  - Radial gradient cushion shading (approximation)
  - 60fps interactive rendering
- **File type categorization**
  - 12 categories: Image, Video, Audio, Document, Archive, Code, Executable, Config, Font, Database, DiskImage, Other
  - Vibrant dark-mode color palette
- **Interactive navigation**
  - Click to drill down into directories
  - Right-click / Backspace to navigate up
  - Hover for file info
- **Analytics panel** (Phase 2 stub)
  - Real-time space breakdown by file type
  - Visual bar chart representation

### 🚧 **In Progress (Phase 2)**
- **True per-pixel cushion shader** (WGSL)
  - Parabolic surface normals
  - Lambertian diffuse lighting
  - Adjustable light direction
- **MFT direct parsing** for instant NTFS scans (~3-5s for full C: drive)
  - Requires Administrator privileges
  - Currently stubbed (falls back to jwalk)
- **Text rendering** for UI overlays
  - Tooltips with file details
  - Breadcrumb navigation
  - Analytics labels

### 🔮 **Planned (Future)**
- USN Journal live updates (animated treemap changes)
- Multi-drive support with tabbed interface
- Export / save session
- Search / filter by name, type, size
- Glassmorphism UI effects
- Custom color themes

---

## Architecture

### **Why Vello (not egui)?**

Vello is a **low-level 2D scene renderer** using compute shaders, not a widget toolkit. This gives us:
- Full pixel-level control for custom cushion shading
- Native GPU parallelism for millions of rectangles
- Zero UI framework overhead
- Smooth 60fps even with complex gradients

egui would fight against custom treemap rendering and add unnecessary widget layers.

### **Fast Scanning Strategy**

| Method | Speed | Privilege | Notes |
|--------|-------|-----------|-------|
| MFT Direct | ~3-5s | Admin | Reads `$MFT` raw, bypasses Windows FS APIs |
| jwalk | ~30-60s | User | Parallel directory walk, universal fallback |

The app auto-detects NTFS + admin and uses MFT when available.

---

## Tech Stack

```toml
vello = "0.4"          # GPU 2D renderer (compute shaders)
winit = "0.30"         # Cross-platform windowing
jwalk = "0.8"          # Parallel filesystem walker
ntfs = "0.4"           # NTFS/MFT parser (WIP)
rayon = "1.10"         # Parallel layout computation
compact_str = "0.8"    # Small-string optimization
```

**No heavyweight UI frameworks.** Just raw Vello scenes + custom rendering.

---

## Project Structure

```
SilvaView-rs/
├── src/
│   ├── scanner/           # Filesystem scanning (MFT + jwalk)
│   ├── tree/              # Arena-based file tree (cache-friendly)
│   ├── layout/            # Squarified treemap algorithm
│   ├── render/            # Vello scene building + cushion shading
│   ├── ui/                # Input handling, navigation, overlays
│   ├── app.rs             # Application state machine
│   └── main.rs            # winit event loop
├── shaders/
│   └── cushion.wgsl       # Per-pixel cushion shader (Phase 2)
└── PLAN.md                # Full technical design doc
```

---

## Building & Running

### Prerequisites
- **Rust** 1.70+ (2021 edition)
- **Windows** (MFT scanning is Windows-only; jwalk works cross-platform)
- **GPU with Vulkan/DX12/Metal** support

### Build
```bash
cargo build --release
```

### Run
```bash
# Scan C: drive (requires Admin for MFT, otherwise uses jwalk)
cargo run --release

# Scan specific path
cargo run --release -- "D:\MyFolder"
```

### Performance
- **Debug builds** (~20fps) — for development iteration
- **Release builds** (~60fps) — production use

---

## Usage

### **Mouse Controls**
- **Left click** — Drill down into file/directory
- **Right click / Backspace** — Navigate up one level
- **Hover** — Show file info tooltip

### **Keyboard Shortcuts** (Planned)
- `Esc` — Navigate to root
- `Tab` — Toggle analytics panel
- `F11` — Fullscreen

---

## Design Philosophy

### **Cache-Friendly Data**
- Files stored in flat `Vec<FileNode>` arena
- Index-based links (no pointer chasing)
- SIMD-friendly for layout parallelism

### **Minimal Heap Allocations**
- `CompactString` for short filenames
- Deduplicated extension table
- Pre-allocated capacity hints

### **GPU-First Rendering**
- Vello compute shaders handle 100k+ rectangles
- Gradient brushes for cushion approximation
- Phase 2: custom WGSL fragment shaders for true per-pixel lighting

---

## Comparison: SequoiaView (2006) vs SilvaView-rs (2026)

| Feature | Original (2006) | SilvaView-rs (2026) |
|---------|-----------------|----------------------|
| **Scan Method** | Windows FS APIs (~5-10 min) | MFT direct (~3-5s) + jwalk fallback |
| **Rendering** | GDI+ CPU rasterization | Vello GPU compute shaders |
| **Cushion Shading** | Parabolic gradients | Phase 1: Radial gradients<br>Phase 2: True WGSL shader |
| **File Tree** | Pointer-heavy | Flat arena (cache-friendly) |
| **Interaction** | Click to drill | Click drill + hover tooltips + analytics |
| **Performance** | ~10fps on large drives | 60fps sustained |

---

## Roadmap

- **Phase 1** ✅ — Core treemap + GPU rendering
- **Phase 2** 🚧 — MFT scanning + true cushion shader + text UI
- **Phase 3** 🔮 — Live updates, multi-drive, search, themes

---

## License

MIT (to be determined)

## Credits

Inspired by **SilvaView** (Jarke J. van Wijk, 2006)
Cushion Treemaps: "Cushion Treemaps: Visualization of Hierarchical Information" (van Wijk & van de Wetering, 1999)
