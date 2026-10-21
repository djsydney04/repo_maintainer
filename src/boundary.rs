use crate::config::{Config, relative_path};
use anyhow::{Result, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn allowed(config: &Config, name: &str, smoke: bool) -> bool {
    if smoke {
        return name == "src/lib.rs";
    }
    if !relative_path(name) || name.split('/').any(|p| p.starts_with('.')) {
        return false;
    }
    let policy = &config.editable;
    if policy.root_files.iter().any(|v| v == name) {
        return true;
    }
    let parent = name.rsplit_once('/').map_or("", |(p, _)| p);
    if policy.markdown_dirs.iter().any(|v| v == parent) && name.ends_with(".md") {
        return true;
    }
    policy
        .source_dirs
        .iter()
        .any(|dir| name.starts_with(&format!("{dir}/")))
        && policy.extensions.iter().any(|ext| name.ends_with(ext))
}
pub fn guard(root: &Path, name: &str, edit: bool, smoke: bool, config: &Config) -> Result<PathBuf> {
    ensure!(
        name == "." || relative_path(name),
        "Path is outside the source boundary"
    );
    ensure!(
        !name
            .split('/')
            .any(|p| p.starts_with('.') && p != "." || ["node_modules", "target"].contains(&p)),
        "Path is outside the source boundary"
    );
    ensure!(
        !edit || allowed(config, name, smoke),
        "This file needs human maintenance approval"
    );
    let root = root.canonicalize()?;
    let mut path = root.clone();
    for part in name.split('/').filter(|p| *p != ".") {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(info) => {
                ensure!(
                    !info.file_type().is_symlink(),
                    "Symlink paths are outside the source boundary"
                );
                ensure!(
                    info.is_file() || info.is_dir(),
                    "Special files are outside the source boundary"
                );
                #[cfg(unix)]
                if edit && info.is_file() {
                    use std::os::unix::fs::MetadataExt;
                    ensure!(
                        info.nlink() == 1,
                        "Hardlinked files are outside the source boundary"
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(path)
}
