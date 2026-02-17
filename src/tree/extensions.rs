/// File type categories for color mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileCategory {
    /// Images: jpg, png, gif, bmp, svg, webp, ico, tiff
    Image,
    /// Video: mp4, avi, mkv, mov, wmv, flv, webm
    Video,
    /// Audio: mp3, wav, flac, aac, ogg, wma, m4a
    Audio,
    /// Documents: pdf, doc, docx, txt, rtf, odt, xls, xlsx, ppt, csv
    Document,
    /// Ebooks: epub, mobi, azw3, djvu
    Ebook,
    /// Archives: zip, rar, 7z, tar, gz, bz2, xz
    Archive,
    /// Code: rs, py, js, ts, c, cpp, h, java, go, rb, php, html, css
    Code,
    /// Executables: exe, dll, sys, msi, bat, cmd, ps1
    Executable,
    /// System/config: ini, cfg, toml, yaml, yml, json, xml, reg
    Config,
    /// Fonts: ttf, otf, woff, woff2
    Font,
    /// Installers/packages: msi, pkg, deb, rpm, appimage
    Installer,
    /// 3D / assets
    Asset3D,
    /// Backups / snapshots
    Backup,
    /// Database: db, sqlite, mdb
    Database,
    /// Disk images / VM: iso, img, vhd, vmdk
    DiskImage,
    /// Unknown / no extension
    Other,
}

/// Classify a file extension into a category.
pub fn categorize_extension(ext: &str) -> FileCategory {
    match ext.to_ascii_lowercase().as_str() {
        // Images
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "svg" | "webp" | "ico" | "tiff" | "tif"
        | "raw" | "cr2" | "nef" | "heic" | "avif" => FileCategory::Image,

        // Video
        "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg"
        | "3gp" | "ts" => FileCategory::Video,

        // Audio
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "wma" | "m4a" | "opus" | "mid" | "midi" => {
            FileCategory::Audio
        }

        // Documents
        "pdf" | "doc" | "docx" | "txt" | "rtf" | "odt" | "xls" | "xlsx" | "ppt" | "pptx"
        | "csv" | "md" => FileCategory::Document,

        // Ebooks
        "epub" | "mobi" | "azw3" | "djvu" | "djv" => FileCategory::Ebook,

        // Archives
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" | "lz4" | "cab" => {
            FileCategory::Archive
        }

        // Code
        "rs" | "py" | "js" | "jsx" | "tsx" | "c" | "cpp" | "h" | "hpp" | "java" | "go"
        | "rb" | "php" | "html" | "htm" | "css" | "scss" | "less" | "swift" | "kt" | "cs"
        | "lua" | "sh" | "bash" | "zsh" | "fish" | "sql" | "r" | "dart" | "zig" | "wasm"
        | "vue" | "svelte" => FileCategory::Code,

        // Executables
        "exe" | "dll" | "sys" | "bat" | "cmd" | "ps1" | "com" | "scr" | "so"
        | "dylib" | "elf" => FileCategory::Executable,

        // Installers
        "msi" | "pkg" | "deb" | "rpm" | "appimage" => FileCategory::Installer,

        // Config
        "ini" | "cfg" | "toml" | "yaml" | "yml" | "json" | "xml" | "reg" | "conf" | "env"
        | "properties" | "lock" => FileCategory::Config,

        // Fonts
        "ttf" | "otf" | "woff" | "woff2" | "eot" => FileCategory::Font,

        // 3D assets
        "blend" | "fbx" | "obj" | "stl" | "dae" | "gltf" | "glb" | "usd" | "usdz" => {
            FileCategory::Asset3D
        }

        // Database
        "db" | "sqlite" | "sqlite3" | "mdb" | "accdb" => FileCategory::Database,

        // Disk images
        "iso" | "img" | "vhd" | "vhdx" | "vmdk" | "qcow2" => FileCategory::DiskImage,

        // Backups
        "bak" | "old" | "backup" => FileCategory::Backup,

        _ => FileCategory::Other,
    }
}
