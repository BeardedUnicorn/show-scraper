use dirs::data_dir;
use once_cell::sync::Lazy;
use std::{fs, path::PathBuf};

static DATA_ROOT: Lazy<PathBuf> = Lazy::new(|| {
    let base = data_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let root = base.join("show-scrape");
    if let Err(err) = fs::create_dir_all(&root) {
        eprintln!("failed to create data root {:?}: {err}", root);
    }
    root
});

pub fn data_root() -> PathBuf {
    DATA_ROOT.clone()
}

pub fn database_path() -> PathBuf {
    data_root().join("show-scrape.sqlite")
}

pub fn config_path() -> PathBuf {
    data_root().join("config.json")
}

pub fn ensure_parent(path: &PathBuf) {
    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            eprintln!("failed to create parent {:?}: {err}", parent);
        }
    }
}
