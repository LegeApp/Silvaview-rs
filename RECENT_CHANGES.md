# Recent Changes (2026-02-16)

This document summarizes the latest troubleshooting and implementation changes made to the treemap pipeline.

## 1) Squarify layout bug fixed

File: `src/layout/squarify.rs`

- Fixed a core geometry error in the squarify algorithm where row `thickness` was computed from the short side of the remaining rectangle.
- This caused width/height inversion in some cases (for example, `1080x1920` inside a `1920x1080` viewport), which then collapsed remaining layout space and skipped many nodes.
- The fix computes thickness from the correct long side:
  - Horizontal row: divide by `w`
  - Vertical column: divide by `h`
- Degenerate remaining-space logging was reduced to avoid warning spam.

## 2) Squarify regression tests added

File: `src/layout/squarify.rs`

Added unit tests:

- `single_item_fills_viewport_without_axis_swap`
- `layout_preserves_area_for_simple_case`

These guard against future regressions in dimension orientation and area preservation.

## 3) Label rendering de-cluttered and made legible

File: `src/render/scene.rs`

Label rendering was reworked to reduce visual noise while preserving useful context:

- Candidate filtering by area, rectangle dimensions, and depth
- Sort by largest-first (prioritize the most important regions)
- Hard cap on number of labels per frame
- Overlap rejection (greedy placement)
- Name truncation with ellipsis when needed
- Semi-transparent background behind text for readability

## 4) Non-GUI diagnostic alignment improved

File: `src/bin/debug-layout.rs`

- Updated label candidate counting to use the same thresholds as the production label heuristics.
- This makes headless diagnostics better reflect what the GUI would show.

## Validation summary

Using `debug-layout` on `D:\Rust-projects\SequoiaView-rs`:

- Before: ~25 layout rectangles, axis-swapped top rectangle observed, heavy "remaining space too small" warnings.
- After: ~3209 layout rectangles, correct top-level rectangle orientation, dramatically improved node coverage and much lower label clutter.

Test status:

- `layout::squarify::tests` pass in release test run.

## Notes

- Full `C:\` scan diagnostics were started, but long-running processes can lock `debug-layout.exe` during rebuilds.
- If needed, tuneable runtime label controls can be added next (`max_labels`, `label_depth`, `label_area_fraction`).
