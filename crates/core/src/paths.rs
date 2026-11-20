use std::path::PathBuf;

use directories::ProjectDirs;

use crate::error::{Error, Result};

/// Where zman keeps its files, following each platform's conventions.
#[derive(Debug, Clone)]
pub struct Paths {
    pub config_file: PathBuf,
    pub data_dir: PathBuf,
    pub db_file: PathBuf,
    /// Held locked by the daemon while it runs, so only one runs at a time.
    pub lock_file: PathBuf,
}

impl Paths {
    pub fn resolve() -> Result<Paths> {
        let dirs = ProjectDirs::from("", "", "zman")
            .ok_or_else(|| Error::Config("could not find the user's home directory".into()))?;
        let data_dir = dirs.data_local_dir().to_path_buf();
        Ok(Paths {
            config_file: dirs.config_dir().join("config.toml"),
            db_file: data_dir.join("zman.db"),
            lock_file: data_dir.join("daemon.lock"),
            data_dir,
        })
    }
}
