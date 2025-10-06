use std::{fs, path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize};

use crate::utils;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    pub facebook_access_token: Option<String>,
    pub facebook_token_expires_at: Option<String>,
    pub facebook_user_id: Option<String>,
    pub facebook_user_name: Option<String>,
    pub facebook_group_id: Option<String>,
}

pub struct ConfigStore {
    path: PathBuf,
    data: Mutex<AppConfig>,
}

impl ConfigStore {
    pub fn load() -> Self {
        let path = utils::config_path();
        let data = read_config(&path).unwrap_or_default();
        Self {
            path,
            data: Mutex::new(data),
        }
    }

    pub fn read(&self) -> AppConfig {
        self.data.lock().expect("config mutex poisoned").clone()
    }

    pub fn update<F>(&self, transform: F) -> Result<AppConfig, String>
    where
        F: FnOnce(&mut AppConfig),
    {
        let mut guard = self
            .data
            .lock()
            .map_err(|_| "config mutex poisoned".to_string())?;
        transform(&mut guard);
        write_config(&self.path, &guard)?;
        Ok(guard.clone())
    }
}

fn read_config(path: &PathBuf) -> Result<AppConfig, String> {
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let contents = fs::read_to_string(path).map_err(|err| err.to_string())?;
    serde_json::from_str(&contents).map_err(|err| err.to_string())
}

fn write_config(path: &PathBuf, config: &AppConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            return Err(err.to_string());
        }
    }
    let contents = serde_json::to_string_pretty(config).map_err(|err| err.to_string())?;
    fs::write(path, contents).map_err(|err| err.to_string())
}
