# Recent Changes (2026-02-16)

This update focused on three persistent issues: stripe-heavy layout, empty-looking folder interiors, and missing text overlays.

## 1) Treemap LOD + chain compression improvements

File: `src/layout/squarify.rs`

- Added coverage-based child truncation:
  - New config: `child_coverage_target` (default `0.995`)
  - New config: `max_children_per_dir` (default `1200`)
- Kept children are re-scaled to fill parent area. This avoids large dark/blank-looking interior regions caused by dropping tiny tails after layout.
- Added strict single-child directory chain compression (`collapse_single_dir_chain`) so repeated full-rectangle nesting is collapsed before recursion.
- Applied chain compression in dominant-child fast path as well.
- Recursion still respects `recurse_min_side` to avoid pathological micro-recursion.

## 2) Cross-platform text font loading fix

File: `src/render/text.rs`

- Replaced single hardcoded font path with fallback search across:
  - `%WINDIR%\\Fonts\\segoeui.ttf`
  - `C:\\Windows\\Fonts\\segoeui.ttf`
  - `/mnt/c/Windows/Fonts/segoeui.ttf` (WSL)
  - common Linux DejaVu paths
- Added startup logging for the exact loaded font path.
- `render_text()` now actually applies `max_width` via `LayoutSettings` and returns `None` for empty glyph output.

## 3) Folder overlays tuned for visibility and behavior

File: `src/render/scene.rs`

- Labels now target directories only and include folder size:
  - `"<folder_name>  <formatted_size>"`
- Label thresholds relaxed for visibility on real disk trees:
  - lower minimum area
  - reduced min width/height
  - deeper max depth
  - increased max labels
- Overlays remain visible on hover so folder-name hit targets stay clickable.
- Added debug metrics for label pipeline:
  - `Text overlays: candidates=..., drawn=...`

## Validation

- `cargo.exe check -q` passes.
- `cargo.exe test -q layout::squarify::tests -- --nocapture` passes.
- `cargo.exe run -q --bin debug-layout -- "D:\\Rust-projects\\SilvaView-rs"` confirms:
  - LOD truncation is active with explicit coverage logs.
  - Layout generation remains stable.
  - Label candidates are present.

## 4) Navigation interaction refinement (label-only drill-down)

Files: `src/render/scene.rs`, `src/app.rs`, `src/main.rs`, `src/ui/input.rs`

- Scene now records clickable label hit regions for rendered folder overlays.
- Left-click drill-down is now label-only:
  - Clicking a folder name drills down one level.
  - Clicking treemap squares does nothing (reserved for future file-inspection behavior).
- Hover no longer suppresses label drawing, so labels remain clickable.
- Window title now updates to current navigation path (for clear landing context), e.g.:
  - `SilvaView-rs — C:\Users\...`

## 5) Framed directory nesting model (parent/child readability)

Files: `src/layout/squarify.rs`, `src/render/scene.rs`

- Added directory frame geometry in layout:
  - New config: `dir_frame_px`
  - New config: `dir_header_px`
  - New config: `dir_frame_falloff`
- Children are now laid out inside the parent's inset frame + header band.
  - This makes subfolders visually nested within their folder instead of appearing as peer stripes.
- Added render-side frame/header overlays for directories:
  - subtle top header band
  - thin border strips around directory bounds
- Labels are now anchored to directory header bands (matching the new visual hierarchy).

## 6) Temporary Iced settings page (not startup, not path control)

Files: `Cargo.toml`, `src/ui/config_dialog.rs`, `src/ui/mod.rs`, `src/main.rs`

- Added `iced` and `rfd` dependencies for a temporary settings page.
- Kept the core app/event/render loop winit-based.
- `F2` opens settings-only dialog (no startup gating, no drive/path control).
- Settings page updates:
  - treemap layout parameters (`min_area`, `min_side`, `recurse_min_side`, frame/header sizes)
  - cushion shading parameters (`ambient`, `diffuse`)
  - label visibility toggle

## 7) Vello path entry overlay for scan control

Files: `src/ui/overlay.rs`, `src/app.rs`, `src/main.rs`

- Added top-center GPU-rendered path bar (`render_path_bar`) in Vello scene.
- Default text starts at current scan path (`C:\` by default).
- Hover/click same area to edit path.
- Press `Enter` to start/rescan using typed path.
- Added drive normalization so `C:` is treated as `C:\`.
