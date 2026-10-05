//! Plugin resolution: built-in -> `semoxide-plugin-<name>` on PATH -> pinned file + sha256.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

#[derive(Debug, PartialEq)]
pub enum Resolved {
    Builtin(String),
    Executable(PathBuf),
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ResolveError {
    #[error("plugin '{0}' not found (no built-in, no semoxide-plugin-{0} on PATH, no pinned source)")]
    NotFound(String),
    #[error("sha256 mismatch for {path}: expected {expected}, got {actual}")]
    ChecksumMismatch { path: PathBuf, expected: String, actual: String },
    #[error("io: {0}")]
    Io(String),
}

/// Stand-in for `{ url, sha256 }`: the "download" is a local file.
#[derive(Debug, Clone)]
pub struct Pinned {
    pub file: PathBuf,
    pub sha256: String,
}

pub fn resolve(
    name: &str,
    builtins: &[&str],
    path_var: Option<&OsString>,
    pathext: Option<&str>,
    pinned: Option<&Pinned>,
) -> Result<Resolved, ResolveError> {
    if builtins.contains(&name) {
        return Ok(Resolved::Builtin(name.into()));
    }
    if let Some(p) = find_on_path(&format!("semoxide-plugin-{name}"), path_var, pathext) {
        return Ok(Resolved::Executable(p));
    }
    if let Some(pin) = pinned {
        verify_sha256(&pin.file, &pin.sha256)?;
        return Ok(Resolved::Executable(pin.file.clone()));
    }
    Err(ResolveError::NotFound(name.into()))
}

/// Windows: try each PATHEXT extension (`.EXE;.CMD;...`), like cmd.exe does.
pub fn find_on_path(stem: &str, path_var: Option<&OsString>, pathext: Option<&str>) -> Option<PathBuf> {
    let exts: Vec<String> = if cfg!(windows) {
        pathext.unwrap_or(".EXE;.CMD;.BAT").split(';').filter(|e| !e.is_empty()).map(|e| e.to_ascii_lowercase()).collect()
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(path_var?) {
        for ext in &exts {
            let cand = dir.join(format!("{stem}{ext}"));
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    None
}

pub fn sha256_file(path: &Path) -> Result<String, ResolveError> {
    let bytes = std::fs::read(path).map_err(|e| ResolveError::Io(e.to_string()))?;
    Ok(Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect())
}

pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), ResolveError> {
    let actual = sha256_file(path)?;
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(ResolveError::ChecksumMismatch { path: path.into(), expected: expected.into(), actual })
    }
}
