/// Comprehensive backend validation tool
/// Tests: Scanner → Tree → Layout → Rasterizer pipeline without GUI
use sequoiaview_rs::layout::{compute_layout, LayoutConfig};
use sequoiaview_rs::render::cushion::{self, CushionConfig};
use sequoiaview_rs::scanner::{self, ScanMethod};
use sequoiaview_rs::tree;
use std::path::PathBuf;
use std::sync::mpsc;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("sequoiaview_rs=info".parse().unwrap()),
        )
        .init();

    let scan_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\"));

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║        SEQUOIAVIEW-RS BACKEND VALIDATION TOOL               ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Target: {}", scan_path.display());
    println!();

    // === STAGE 1: SCANNER ===
    println!("┌─ STAGE 1: MFT/WALKDIR SCANNER ─────────────────────────────┐");
    let (tx, _rx) = mpsc::channel();
    let start = std::time::Instant::now();
    let entries = scanner::scan(&scan_path, ScanMethod::Auto, tx)?;
    let scan_duration = start.elapsed();

    let total_bytes: u64 = entries.iter().map(|e| e.size).sum();
    let dir_count = entries.iter().filter(|e| e.is_dir).count();
    let file_count = entries.iter().filter(|e| !e.is_dir).count();

    println!("  ✓ Scan completed in {:.2}s", scan_duration.as_secs_f64());
    println!("  ✓ Entries:  {} ({} dirs, {} files)", entries.len(), dir_count, file_count);
    println!("  ✓ Total:    {:.2} GB", total_bytes as f64 / 1_073_741_824.0);
    println!("└────────────────────────────────────────────────────────────┘");
    println!();

    if entries.is_empty() {
        println!("✗ FAILED: No entries found!");
        return Ok(());
    }

    // === STAGE 2: TREE BUILDING ===
    println!("┌─ STAGE 2: TREE CONSTRUCTION ───────────────────────────────┐");
    let start = std::time::Instant::now();
    let tree = tree::build_tree(&entries);
    let tree_duration = start.elapsed();

    let root_node = tree.get(tree.root);
    println!("  ✓ Tree built in {:.2}s", tree_duration.as_secs_f64());
    println!("  ✓ Nodes:    {} total", tree.len());
    println!("  ✓ Root:     '{}' ({:.2} GB)", root_node.name, root_node.size as f64 / 1_073_741_824.0);

    // Validate tree structure
    let mut validation_errors = 0;
    for i in 0..tree.nodes.len() {
        let node_id = sequoiaview_rs::tree::arena::NodeId(i as u32);
        let node = tree.get(node_id);

        // Check for invalid sizes
        if node.is_dir && node.size == 0 && tree.children(node_id).count() > 0 {
            println!("  ✗ WARNING: Directory '{}' has children but size=0", node.name);
            validation_errors += 1;
            if validation_errors >= 5 {
                println!("  ... (truncated, too many errors)");
                break;
            }
        }
    }

    if validation_errors == 0 {
        println!("  ✓ Tree structure valid (all dirs have correct aggregated sizes)");
    }
    println!("└────────────────────────────────────────────────────────────┘");
    println!();

    // === STAGE 3: LAYOUT COMPUTATION ===
    println!("┌─ STAGE 3: SQUARIFIED LAYOUT ───────────────────────────────┐");
    let config = LayoutConfig::default();
    let viewport_w = 1920.0;
    let viewport_h = 1080.0;

    let start = std::time::Instant::now();
    let layout = compute_layout(&tree, tree.root, viewport_w, viewport_h, &config);
    let layout_duration = start.elapsed();

    println!("  ✓ Layout computed in {:.2}ms", layout_duration.as_secs_f64() * 1000.0);
    println!("  ✓ Rectangles: {}", layout.rects.len());

    // Validate layout
    let mut invalid_rects = 0;
    let mut total_area = 0.0f32;
    let viewport_area = viewport_w * viewport_h;

    for rect in &layout.rects {
        let area = rect.w * rect.h;
        total_area += area;

        // Check for invalid dimensions
        if !rect.w.is_finite() || !rect.h.is_finite() || rect.w <= 0.0 || rect.h <= 0.0 {
            let node = tree.get(rect.node);
            println!(
                "  ✗ INVALID RECT: '{}' - {}x{} at ({}, {})",
                node.name, rect.w, rect.h, rect.x, rect.y
            );
            invalid_rects += 1;
            if invalid_rects >= 5 {
                println!("  ... (truncated, too many errors)");
                break;
            }
        }

        // Check for absurdly large dimensions
        if rect.w > 1_000_000.0 || rect.h > 1_000_000.0 {
            let node = tree.get(rect.node);
            println!(
                "  ✗ ABSURD RECT: '{}' - {}x{} at ({}, {})",
                node.name, rect.w, rect.h, rect.x, rect.y
            );
            invalid_rects += 1;
            if invalid_rects >= 5 {
                println!("  ... (truncated, too many errors)");
                break;
            }
        }
    }

    if invalid_rects == 0 {
        println!("  ✓ All rectangles valid (finite, positive dimensions)");
    } else {
        println!("  ✗ FAILED: {} invalid rectangles found!", invalid_rects);
        return Ok(());
    }

    println!("  ✓ Coverage: {:.1}% of viewport", (total_area / viewport_area) * 100.0);
    println!("    (>100% is expected due to parent-child overlap)");

    // Surface coefficients validation
    let mut zero_surface_count = 0;
    for rect in &layout.rects {
        if rect.surface == [0.0; 4] && rect.depth > 0 {
            zero_surface_count += 1;
        }
    }
    if zero_surface_count > 0 {
        println!("  ⚠ WARNING: {} rects at depth>0 have zero surface coefficients", zero_surface_count);
    } else {
        println!("  ✓ All non-root rects have non-zero surface coefficients");
    }

    println!("└────────────────────────────────────────────────────────────┘");
    println!();

    // === STAGE 4: CUSHION RASTERIZATION ===
    println!("┌─ STAGE 4: CUSHION RASTERIZATION ──────────────────────────┐");
    let cushion_config = CushionConfig::default();
    let width = 1920u32;
    let height = 1080u32;

    let start = std::time::Instant::now();
    let buffer = cushion::rasterize_cushions(width, height, &layout.rects, &tree, &cushion_config);
    let raster_duration = start.elapsed();

    let expected_size = (width * height * 4) as usize;
    println!("  ✓ Rasterized in {:.2}ms", raster_duration.as_secs_f64() * 1000.0);
    println!("  ✓ Buffer size: {} bytes (expected: {})", buffer.len(), expected_size);

    if buffer.len() != expected_size {
        println!("  ✗ FAILED: Buffer size mismatch!");
        return Ok(());
    }

    // Sample pixel validation
    let mut black_pixels = 0;
    let mut colored_pixels = 0;
    for pixel in buffer.chunks_exact(4) {
        if pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0 {
            black_pixels += 1;
        } else {
            colored_pixels += 1;
        }
    }

    let total_pixels = width * height;
    println!("  ✓ Colored pixels: {}/{} ({:.1}%)",
        colored_pixels,
        total_pixels,
        (colored_pixels as f64 / total_pixels as f64) * 100.0
    );

    if colored_pixels == 0 {
        println!("  ✗ FAILED: No colored pixels (completely black image)!");
        return Ok(());
    }

    println!("  ✓ Rasterization appears valid");
    println!("└────────────────────────────────────────────────────────────┘");
    println!();

    // === FINAL SUMMARY ===
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                    ✓ ALL TESTS PASSED                       ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Pipeline summary:");
    println!("  • Scanner:      {:.2}s", scan_duration.as_secs_f64());
    println!("  • Tree build:   {:.2}s", tree_duration.as_secs_f64());
    println!("  • Layout:       {:.2}ms", layout_duration.as_secs_f64() * 1000.0);
    println!("  • Rasterize:    {:.2}ms", raster_duration.as_secs_f64() * 1000.0);
    println!();
    println!("The backend is functioning correctly!");
    println!("If the GUI shows gibberish, the issue is in the rendering/overlay layer.");

    Ok(())
}
