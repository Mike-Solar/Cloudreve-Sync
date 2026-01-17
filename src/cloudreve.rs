use crate::config::ApiPaths;
use crate::error::CloudreveError;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;

#[derive(Debug, Clone)]
pub struct RemoteFile {
    pub path: String,
    pub size: u64,
    pub modified: String,
}

#[derive(Debug, Clone)]
pub struct CloudreveClient {
    client: reqwest::Client,
    base_url: String,
    access_token: String,
    api_paths: ApiPaths,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
    pub code: u32,
    pub msg: String,
}

impl CloudreveClient {
    pub fn new(base_url: String, access_token: String, api_paths: ApiPaths) -> Self {
        let base_url = if base_url.ends_with("/api/v4") {
            base_url
        } else if base_url.ends_with('/') {
            format!("{}api/v4", base_url.trim_end_matches('/'))
        } else {
            format!("{}/api/v4", base_url)
        };
        Self {
            client: reqwest::Client::new(),
            base_url,
            access_token,
            api_paths,
        }
    }

    pub async fn list_files(&self, remote_root: &str) -> Result<Vec<RemoteFile>, Box<dyn Error>> {
        let url = format!(
            "{}{}?path={}",
            self.base_url,
            self.api_paths.list_files,
            urlencoding::encode(remote_root)
        );
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.access_token)
            .send()
            .await?
            .json::<ApiResponse<Value>>()
            .await?;
        if response.code != 0 {
            return Err(Box::new(CloudreveError::from_u32(response.code)));
        }
        let mut out = Vec::new();
        let objects = response
            .data
            .get("objects")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for obj in objects {
            let path = pick_string(&obj, &["path", "full_path", "name"]).unwrap_or_default();
            let size = pick_u64(&obj, &["size", "bytes"]).unwrap_or(0);
            let modified = pick_string(&obj, &["updated_at", "modified_at", "mtime"])
                .unwrap_or_else(|| "".to_string());
            let is_dir = pick_bool(&obj, &["is_dir", "folder", "isFolder"]).unwrap_or(false);
            if path.is_empty() || is_dir {
                continue;
            }
            out.push(RemoteFile { path, size, modified });
        }
        Ok(out)
    }

    pub async fn download_file(
        &self,
        remote_path: &str,
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        let url = format!("{}{}", self.base_url, self.api_paths.create_download);
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.access_token)
            .json(&serde_json::json!({ "path": remote_path }))
            .send()
            .await?
            .json::<ApiResponse<Value>>()
            .await?;
        if response.code != 0 {
            return Err(Box::new(CloudreveError::from_u32(response.code)));
        }
        let download_url = response
            .data
            .get("url")
            .or_else(|| response.data.get("download_url"))
            .and_then(|v| v.as_str())
            .ok_or("download url missing")?;
        let bytes = self
            .client
            .get(download_url)
            .bearer_auth(&self.access_token)
            .send()
            .await?
            .bytes()
            .await?;
        Ok(bytes.to_vec())
    }

    pub async fn upload_file(
        &self,
        remote_path: &str,
        content: &[u8],
    ) -> Result<(), Box<dyn Error>> {
        let url = format!("{}{}", self.base_url, self.api_paths.update_content);
        let payload = base64::engine::general_purpose::STANDARD.encode(content);
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.access_token)
            .json(&serde_json::json!({ "path": remote_path, "content": payload }))
            .send()
            .await?
            .json::<ApiResponse<Value>>()
            .await?;
        if response.code != 0 {
            return Err(Box::new(CloudreveError::from_u32(response.code)));
        }
        Ok(())
    }

    pub async fn create_share_link(&self, remote_path: &str) -> Result<String, Box<dyn Error>> {
        let url = format!("{}{}", self.base_url, self.api_paths.create_share_link);
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.access_token)
            .json(&serde_json::json!({ "path": remote_path }))
            .send()
            .await?
            .json::<ApiResponse<Value>>()
            .await?;
        if response.code != 0 {
            return Err(Box::new(CloudreveError::from_u32(response.code)));
        }
        let link = response
            .data
            .get("url")
            .or_else(|| response.data.get("link"))
            .and_then(|v| v.as_str())
            .ok_or("share link missing")?;
        Ok(link.to_string())
    }

    pub async fn delete_files(&self, uris: Vec<String>) -> Result<(), Box<dyn Error>> {
        if uris.is_empty() {
            return Ok(());
        }
        let url = format!("{}{}", self.base_url, self.api_paths.delete_file);
        let response = self
            .client
            .delete(url)
            .bearer_auth(&self.access_token)
            .json(&serde_json::json!({ "uris": uris, "skip_soft_delete": true }))
            .send()
            .await?
            .json::<ApiResponse<Value>>()
            .await?;
        if response.code != 0 {
            return Err(Box::new(CloudreveError::from_u32(response.code)));
        }
        Ok(())
    }

    /// 构造 Cloudreve File URI，默认使用 my 文件系统。
    pub fn build_file_uri(remote_path: &str) -> String {
        if remote_path.starts_with("cloudreve://") {
            return remote_path.to_string();
        }
        let mut path = remote_path.trim().to_string();
        if !path.starts_with('/') {
            path = format!("/{}", path);
        }
        format!("cloudreve://my{}", path)
    }
}

fn pick_string(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(s) = value.get(*key).and_then(|v| v.as_str()) {
            return Some(s.to_string());
        }
    }
    None
}

fn pick_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    for key in keys {
        if let Some(n) = value.get(*key).and_then(|v| v.as_u64()) {
            return Some(n);
        }
    }
    None
}

fn pick_bool(value: &Value, keys: &[&str]) -> Option<bool> {
    for key in keys {
        if let Some(b) = value.get(*key).and_then(|v| v.as_bool()) {
            return Some(b);
        }
    }
    None
}
