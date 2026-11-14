use rand::seq::SliceRandom;
use std::path::Path;

pub const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "bmp", "webp", "gif"];

fn is_image(p: &Path) -> bool {
    p.is_file()
        && p.extension()
            .map(|e| IMAGE_EXTS.contains(&e.to_string_lossy().to_lowercase().as_str()))
            .unwrap_or(false)
}

/// List image files in `folder`, sorted by name (case-insensitive extension match).
pub fn list_images(folder: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(Path::new(folder)) else {
        return Vec::new();
    };

    let mut files: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| is_image(p))
        .map(|p| p.to_string_lossy().into_owned())
        .collect();

    files.sort();
    files
}

/// Keep only the entries of an explicit list that exist and are images,
/// preserving the given order.
pub fn filter_images(files: &[String]) -> Vec<String> {
    files
        .iter()
        .map(|f| f.trim())
        .filter(|f| !f.is_empty() && is_image(Path::new(f)))
        .map(|f| f.to_string())
        .collect()
}

pub fn shuffle(files: &mut [String]) {
    let mut rng = rand::thread_rng();
    files.shuffle(&mut rng);
}
