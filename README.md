<div align="center">
  <img src="./Screenshot%202026-02-18%20083634.png" alt="SilvaView-rs screenshot" width="100%" />
</div>

# SilvaView-rs

GPU-accelerated disk space visualizer inspired by SilvaView/SequoiaView, now in a near-final Rust build.

## Release Status

- Phase 1 complete: high-performance treemap layout, navigation, and GPU rendering
- Phase 2 complete: direct NTFS MFT scanning on Windows with automatic fallback scanning
- Phase 3 complete: WGSL per-pixel cushion shading, cross-platform runtime support, and polished UI workflow

## Key Capabilities

- WGSL cushion shader pipeline (`src/render/shaders/cushion.wgsl`) for per-pixel lighting on treemap tiles
- GPU renderer backed by `vello` + `wgpu` for responsive interaction on large trees
- Multi-strategy scanning:
  - Windows NTFS MFT path for fast privileged scans
  - Parallel `jwalk` fallback for non-privileged and non-Windows paths
- Cross-platform foundation with `winit` windowing and platform-gated integrations
- Interactive exploration features:
  - Click/label drill-down
  - Back navigation
  - Hover inspection
  - Drive picker sidebar
  - Live scan progress

## Platform Notes

- Windows: full feature set, including direct MFT scanning (when elevated)
- Linux: cross-platform scanning/rendering path via `jwalk` + `winit`/`wgpu`
- Other platforms: core runtime is portable through `winit`/`wgpu`; platform-specific integrations are conditionally compiled

## Build

```bash
cargo build --release
```

## Run

```bash
# Default path (platform-specific default)
cargo run --release

# Specific path
cargo run --release -- "D:\\Rust-projects"
```

## Architecture Snapshot

- `src/scanner/`: MFT scanner + parallel walk scanner + progress events
- `src/tree/`: compact arena tree used for layout and rendering
- `src/layout/`: squarified/cushion-aware treemap layout
- `src/render/`: GPU render path and WGSL shading pipeline
- `src/ui/`: overlays, drive selection, hit-testing, and controls
- `src/app.rs` and `src/main.rs`: app state machine and event loop integration

## Project State

This repository reflects the relatively final build of the program: the planned rendering/scanning phases are implemented, WGSL shader support is integrated, and cross-platform execution paths are in place.

## Credits

- Inspired by SilvaView / SequoiaView
- Cushion treemap concept from van Wijk & van de Wetering
