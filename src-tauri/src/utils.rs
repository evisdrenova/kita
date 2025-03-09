pub fn get_category_from_extension(extension: &str) -> String {
    let ext = extension.to_lowercase();

    match ext.as_str() {
        // Documents
        "pdf" | "docx" | "doc" | "txt" | "rtf" | "odt" | "md" | "tex" => "document".to_string(),

        // Spreadsheets
        "xlsx" | "xls" | "csv" | "ods" | "numbers" => "spreadsheet".to_string(),

        // Presentations
        "pptx" | "ppt" | "key" | "odp" => "presentation".to_string(),

        // Images
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tiff" | "svg" | "webp" => "image".to_string(),

        // Audio
        "mp3" | "wav" | "ogg" | "flac" | "aac" | "m4a" => "audio".to_string(),

        // Video
        "mp4" | "avi" | "mov" | "wmv" | "mkv" | "webm" | "flv" => "video".to_string(),

        // Archives
        "zip" | "rar" | "tar" | "gz" | "7z" | "bz2" => "archive".to_string(),

        // Code
        "py" | "js" | "html" | "css" | "java" | "cpp" | "c" | "rs" | "go" | "php" | "rb"
        | "swift" | "kt" | "ts" | "jsx" | "tsx" | "json" | "xml" | "yaml" | "toml" => {
            "code".to_string()
        }

        // Executables
        "exe" | "msi" | "app" | "dmg" | "deb" | "rpm" => "executable".to_string(),

        // Default for unknown extensions
        _ => "other".to_string(),
    }
}
