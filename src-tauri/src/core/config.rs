use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiPaths {
    pub list_files: String,
    pub create_download: String,
    pub update_content: String,
    pub create_upload_session: String,
    pub upload_chunk: String,
    pub patch_metadata: String,
    pub create_share_link: String,
    pub delete_file: String,
}

impl Default for ApiPaths {
    fn default() -> Self {
        Self {
            list_files: "/file".to_string(),
            create_download: "/file/url".to_string(),
            update_content: "/file/content".to_string(),
            create_upload_session: "/file/upload".to_string(),
            upload_chunk: "/file/upload".to_string(),
            patch_metadata: "/file/metadata".to_string(),
            create_share_link: "/share".to_string(),
            delete_file: "/file".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub base_url: String,
    pub local_root: String,
    pub remote_root: String,
    pub sync_interval_secs: u64,
    pub api_paths: ApiPaths,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            base_url: "https://example.com/api/v4".to_string(),
            local_root: String::new(),
            remote_root: "/".to_string(),
            sync_interval_secs: 60,
            api_paths: ApiPaths::default(),
        }
    }
}

pub fn config_dir() -> Result<PathBuf, Box<dyn Error>> {
    let proj = directories::ProjectDirs::from("org", "cloudreve", "CloudreveSync")
        .ok_or("failed to locate config dir")?;
    Ok(proj.config_dir().to_path_buf())
}

pub fn config_path() -> Result<PathBuf, Box<dyn Error>> {
    Ok(config_dir()?.join("config.json"))
}

pub fn state_path() -> Result<PathBuf, Box<dyn Error>> {
    Ok(config_dir()?.join("state.json"))
}

pub fn logs_path() -> Result<PathBuf, Box<dyn Error>> {
    Ok(config_dir()?.join("sync.log.jsonl"))
}

pub fn ensure_dir(path: &Path) -> Result<(), Box<dyn Error>> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

impl AppConfig {
    pub fn load() -> Result<Self, Box<dyn Error>> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let path = config_path()?;
        ensure_dir(path.parent().ok_or("config path invalid")?)?;
        fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}
