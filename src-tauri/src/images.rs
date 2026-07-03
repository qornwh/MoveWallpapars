use rand::seq::SliceRandom;
use std::path::Path;

const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "bmp", "webp", "gif"];

/// List image files in `folder`, sorted by name (case-insensitive extension match).
pub fn list_images(folder: &str) -> Vec<String> {
    let path = Path::new(folder);
    let Ok(entries) = std::fs::read_dir(path) else {
        return Vec::new();
    };

    let mut files: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            if !p.is_file() {
                return None;
            }
            let ext = p.extension()?.to_string_lossy().to_lowercase();
            if IMAGE_EXTS.contains(&ext.as_str()) {
                Some(p.to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .collect();

    files.sort();
    files
}

pub fn shuffle(files: &mut [String]) {
    let mut rng = rand::thread_rng();
    files.shuffle(&mut rng);
}
