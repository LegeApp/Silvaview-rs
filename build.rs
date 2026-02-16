fn main() {
    // Embed Windows manifest for UAC elevation
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_manifest_file("sequoiaview-rs.exe.manifest");
        res.set_icon("icon.ico"); // Optional: add an icon
        if let Err(e) = res.compile() {
            eprintln!("Warning: Failed to embed manifest: {}", e);
        }
    }
}
