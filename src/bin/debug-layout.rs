/// Diagnostic tool to verify tree → layout → render pipeline
use sequoiaview_rs::layout::compute_layout;
use sequoiaview_rs::layout::LayoutConfig;
use sequoiaview_rs::scanner::{self, ScanMethod};
use sequoiaview_rs::tree;
use std::path::PathBuf;
use std::sync::mpsc;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("sequoiaview_rs=debug".parse().unwrap()),
        )
        .init();

    let scan_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\"));

    println!("=== DIAGNOSTIC: Tree → Layout Pipeline ===");
    println!("Scanning: {}", scan_path.display());

    // Scan
    let (tx, _rx) = mpsc::channel();
    let entries = scanner::scan(&scan_path, ScanMethod::Auto, tx)?;
    println!("\n[1] Scan completed: {} entries", entries.len());

    // Build tree
    let tree = tree::build_tree(&entries);
    println!("\n[2] Tree built: {} nodes", tree.len());

    let root_node = tree.get(tree.root);
    println!(
        "    Root: '{}' (size={:.2} GB, is_dir={})",
        root_node.name,
        root_node.size as f64 / 1_073_741_824.0,
        root_node.is_dir
    );

    // Show top 10 children of root by size
    println!("\n[3] Top 10 children of root:");
    let mut root_children: Vec<_> = tree.children(tree.root).collect();
    root_children.sort_by_key(|&id| std::cmp::Reverse(tree.get(id).size));

    for (i, child_id) in root_children.iter().take(10).enumerate() {
        let child = tree.get(*child_id);
        println!(
            "    [{}] '{}' - {:.2} GB (dir={}, children={})",
            i,
            child.name,
            child.size as f64 / 1_073_741_824.0,
            child.is_dir,
            if child.is_dir {
                tree.children(*child_id).count()
            } else {
                0
            }
        );
    }

    // Compute layout
    let config = LayoutConfig::default();
    let layout = compute_layout(&tree, tree.root, 1920.0, 1080.0, &config);

    println!("\n[4] Layout computed: {} rectangles", layout.rects.len());

    // Show top 10 largest rectangles
    println!("\n[5] Top 10 largest rectangles by area:");
    let mut sorted_rects = layout.rects.clone();
    sorted_rects.sort_by(|a, b| {
        let area_a = a.w * a.h;
        let area_b = b.w * b.h;
        area_b.partial_cmp(&area_a).unwrap()
    });

    for (i, rect) in sorted_rects.iter().take(10).enumerate() {
        let node = tree.get(rect.node);
        println!(
            "    [{}] '{}' - rect: {:.1}x{:.1} ({:.0}px²) at ({:.1}, {:.1}) - size: {:.2} GB (dir={})",
            i,
            node.name,
            rect.w,
            rect.h,
            rect.w * rect.h,
            rect.x,
            rect.y,
            node.size as f64 / 1_073_741_824.0,
            node.is_dir
        );
    }

    // Check for anomalies
    println!("\n[6] Checking for anomalies:");

    let mut area_sum = 0.0f32;
    let viewport_area = 1920.0 * 1080.0;

    for rect in &layout.rects {
        area_sum += rect.w * rect.h;
    }

    println!("    Total rect area: {:.0}px²", area_sum);
    println!("    Viewport area:   {:.0}px²", viewport_area);
    println!("    Coverage: {:.1}%", (area_sum / viewport_area) * 100.0);

    // Check if surface coefficients are being computed
    println!("\n[7] Surface coefficient samples:");
    for (i, rect) in sorted_rects.iter().take(5).enumerate() {
        let node = tree.get(rect.node);
        println!(
            "    [{}] '{}' - surface: [{:.3}, {:.3}, {:.3}, {:.3}]",
            i, node.name, rect.surface[0], rect.surface[1], rect.surface[2], rect.surface[3]
        );
    }

    // Count how many rects are label candidates with production thresholds
    let viewport_area = layout
        .rects
        .first()
        .map(|r| (r.w * r.h).max(1.0))
        .unwrap_or(1.0);
    let min_label_area = (viewport_area * 0.003).max(8_000.0);
    let labeled_count = layout
        .rects
        .iter()
        .filter(|r| r.w * r.h >= min_label_area && r.w >= 70.0 && r.h >= 20.0 && r.depth <= 5)
        .count();

    println!("\n[8] Text label count: {} rects (out of {})", labeled_count, layout.rects.len());

    Ok(())
}
