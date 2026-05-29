#![allow(dead_code)]
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub struct FileUtils;

impl FileUtils {
    pub fn ensure_dir(path: &Path) -> io::Result<()> {
        if !path.exists() {
            fs::create_dir_all(path)?;
        }
        Ok(())
    }

    pub fn extension(path: &Path) -> Option<String> {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
    }

    pub fn has_extension(path: &Path, allowed: &[impl AsRef<str>]) -> bool {
        if let Some(ext) = Self::extension(path) {
            allowed.iter().any(|a| ext == a.as_ref())
        } else {
            false
        }
    }

    pub fn read_to_string_limited(path: &Path, max_size: u64) -> io::Result<String> {
        let metadata = fs::metadata(path)?;
        if metadata.len() > max_size {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "File too large"));
        }
        fs::read_to_string(path)
    }

    pub fn relative_path(path: &Path, base: &Path) -> Option<PathBuf> {
        path.strip_prefix(base).ok().map(|p| p.to_path_buf())
    }

    pub fn unique_filename(path: &Path) -> PathBuf {
        if !path.exists() {
            return path.to_path_buf();
        }
        let stem = path.file_stem().unwrap_or_default();
        let ext = path.extension().unwrap_or_default();
        let parent = path.parent().unwrap_or(Path::new("."));
        let mut counter = 1;
        loop {
            let new_name = format!(
                "{}_{}.{}",
                stem.to_string_lossy(),
                counter,
                ext.to_string_lossy()
            );
            let new_path = parent.join(new_name);
            if !new_path.exists() {
                return new_path;
            }
            counter += 1;
        }
    }

    pub fn sanitize_filename(name: &str) -> String {
        name.chars()
            .map(|c| match c {
                '<' | '>' | ':' | '\"' | '/' | '\\' | '|' | '?' | '*' | '\0' => '_',
                _ => c,
            })
            .collect()
    }

    pub fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
        fs::create_dir_all(&dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            if ty.is_dir() {
                Self::copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
            } else {
                fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
            }
        }
        Ok(())
    }

    pub fn dir_size(path: &Path) -> io::Result<u64> {
        let mut size = 0u64;
        for entry in walkdir::WalkDir::new(path) {
            let entry = entry?;
            if entry.file_type().is_file() {
                size += entry.metadata()?.len();
            }
        }
        Ok(size)
    }

    pub fn list_files_with_ext(dir: &Path, ext: &str) -> io::Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(file_ext) = Self::extension(&path) {
                    if file_ext == ext {
                        files.push(path);
                    }
                }
            }
        }
        Ok(files)
    }
}
