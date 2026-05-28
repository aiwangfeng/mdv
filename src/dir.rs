// src/dir.rs
// Directory scanning and file management

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub path: PathBuf,
    pub display_name: String,
    pub depth: usize,
    pub modified: SystemTime,
}

pub fn scan_markdown_files(dir: &Path) -> Vec<DirEntry> {
    let mut entries = Vec::new();
    let base = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    walk_dir(&base, &base, 0, &mut entries);
    entries.sort_by_key(|b| std::cmp::Reverse(b.modified));
    entries
}

fn walk_dir(base: &Path, current: &Path, depth: usize, entries: &mut Vec<DirEntry>) {
    if depth > 20 {
        log::warn!(
            "Reached maximum directory scanning depth of 20 at: {}",
            current.display()
        );
        return;
    }
    let Ok(read_dir) = fs::read_dir(current) else {
        log::warn!("Cannot read directory: {}", current.display());
        return;
    };

    for entry in read_dir.flatten() {
        let path = entry.path();

        let meta = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if meta.is_dir() {
            if let Some(name) = path.file_name() {
                let name_str = name.to_string_lossy();
                if name_str.starts_with('.') || name_str == "target" {
                    continue;
                }
            }
            walk_dir(base, &path, depth + 1, entries);
            continue;
        }

        if is_markdown_file(&path) {
            let display_name = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let rel_depth = path
                .strip_prefix(base)
                .map(|p| p.components().count().saturating_sub(1))
                .unwrap_or(0);
            entries.push(DirEntry {
                path,
                display_name,
                depth: rel_depth,
                modified,
            });
        }
    }
}

fn is_markdown_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e == "md" || e == "markdown")
        .unwrap_or(false)
}
