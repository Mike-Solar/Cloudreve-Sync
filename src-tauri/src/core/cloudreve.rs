use crate::core::config::ApiPaths;
use crate::core::error::CloudreveError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
    pub code: u32,
    pub msg: String,
}

#[derive(Debug, Clone)]
pub struct RemoteFile {
    pub id: String,
    pub name: String,
    pub uri: String,
    pub size: u64,
    pub updated_at: String,
    pub metadata: HashMap<String, String>,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadUrl {
    pub url: String,
    pub stream_saver_display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadUrlResponse {
    pub urls: Vec<DownloadUrl>,
    pub expires: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadSession {
    pub session_id: String,
    pub upload_id: Option<String>,
    pub chunk_size: u64,
    pub expires: u64,
    pub upload_urls: Option<Vec<String>>,
    pub credential: Option<String>,
    pub completeURL: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires: String,
    pub refresh_expires: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: TokenPair,
}

#[derive(Debug, Clone)]
pub struct CloudreveClient {
    client: reqwest::Client,
    base_url: String,
    access_token: Option<String>,
    api_paths: ApiPaths,
}

#[derive(Debug, Deserialize)]
pub struct ListFilesData {
    #[serde(default)]
    pub files: Vec<FileEntry>,
    #[serde(default)]
    pub context_hint: Option<String>,
    #[serde(default)]
    pub next_marker: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FileEntry {
    #[serde(rename = "type")]
    file_type: i64,
    id: String,
    name: String,
    size: u64,
    updated_at: String,
    path: String,
    #[serde(default)]
    metadata: Option<HashMap<String, String>>,
}

impl CloudreveClient {
    pub fn new(base_url: String, access_token: Option<String>, api_paths: ApiPaths) -> Self {
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

    pub fn set_access_token(&mut self, token: Option<String>) {
        self.access_token = token;
    }

    pub async fn ping(&self) -> Result<(), Box<dyn Error>> {
        let url = format!("{}/site/ping", self.base_url);
        let response = self
            .client
            .get(url)
            .send()
            .await?
            .json::<ApiResponse<Value>>()
            .await?;
        if response.code != 0 {
            return Err(Box::new(CloudreveError::from_u32(response.code)));
        }
        Ok(())
    }

    pub async fn list_files(&self, uri: &str, page: Option<u32>) -> Result<ListFilesData, Box<dyn Error>> {
        let mut url = format!(
            "{}{}?uri={}",
            self.base_url,
            self.api_paths.list_files,
            urlencoding::encode(uri)
        );
        if let Some(page) = page {
            url.push_str(&format!("&page={}", page));
        }
        let response = self
            .apply_auth(self.client.get(url))
            .send()
            .await?
            .json::<ApiResponse<ListFilesData>>()
            .await?;
        if response.code != 0 {
            return Err(Box::new(CloudreveError::from_u32(response.code)));
        }
        Ok(response.data)
    }

    pub async fn list_all_files(&self, uri: &str) -> Result<Vec<RemoteFile>, Box<dyn Error>> {
        let mut page = 1u32;
        let mut output = Vec::new();
        loop {
            let data = self.list_files(uri, Some(page)).await?;
            for item in data.files {
                let metadata = item.metadata.unwrap_or_default();
                let is_dir = item.file_type == 1;
                output.push(RemoteFile {
                    id: item.id,
                    name: item.name,
                    uri: item.path,
                    size: item.size,
                    updated_at: item.updated_at,
                    metadata,
                    is_dir,
                });
            }
            if data.next_marker.is_none() {
                break;
            }
            page += 1;
        }
        Ok(output)
    }

    pub async fn list_storage_policies(&self) -> Result<Vec<Value>, Box<dyn Error>> {
        let url = format!("{}/user/setting/policies", self.base_url);
        let response = self
            .apply_auth(self.client.get(url))
            .send()
            .await?
            .json::<ApiResponse<Vec<Value>>>()
            .await?;
        if response.code != 0 {
            return Err(Box::new(CloudreveError::from_u32(response.code)));
        }
        Ok(response.data)
    }

    pub async fn create_download_urls(
        &self,
        uris: Vec<String>,
        download: bool,
    ) -> Result<DownloadUrlResponse, Box<dyn Error>> {
        let url = format!("{}{}", self.base_url, self.api_paths.create_download);
        let response = self
            .apply_auth(self.client.post(url))
            .json(&serde_json::json!({
                "uris": uris,
                "download": download
            }))
            .send()
            .await?
            .json::<ApiResponse<DownloadUrlResponse>>()
            .await?;
        if response.code != 0 {
            return Err(Box::new(CloudreveError::from_u32(response.code)));
        }
        Ok(response.data)
    }

    pub async fn download_file(&self, uri: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        let result = self.create_download_urls(vec![uri.to_string()], true).await?;
        let url = result
            .urls
            .first()
            .map(|item| item.url.clone())
            .ok_or("download url missing")?;
        let bytes = self.client.get(url).send().await?.bytes().await?;
        Ok(bytes.to_vec())
    }

    pub async fn update_file_content(&self, uri: &str, content: &[u8]) -> Result<(), Box<dyn Error>> {
        let url = format!(
            "{}{}?uri={}",
            self.base_url,
            self.api_paths.update_content,
            urlencoding::encode(uri)
        );
        let request = self
            .apply_auth(self.client.put(url))
            .header(reqwest::header::CONTENT_LENGTH, content.len() as u64)
            .body(content.to_vec());
        let response = request.send().await?.json::<ApiResponse<Value>>().await?;
        if response.code != 0 {
            return Err(Box::new(CloudreveError::from_u32(response.code)));
        }
        Ok(())
    }

    pub async fn create_upload_session(
        &self,
        uri: &str,
        size: u64,
        policy_id: &str,
        last_modified: Option<i64>,
        mime_type: Option<&str>,
    ) -> Result<UploadSession, Box<dyn Error>> {
        let url = format!("{}{}", self.base_url, self.api_paths.create_upload_session);
        let mut payload = serde_json::json!({
            "uri": uri,
            "size": size,
            "policy_id": policy_id
        });
        if let Some(last_modified) = last_modified {
            payload["last_modified"] = serde_json::json!(last_modified);
        }
        if let Some(mime_type) = mime_type {
            payload["mime_type"] = serde_json::json!(mime_type);
        }
        let response = self
            .apply_auth(self.client.put(url))
            .json(&payload)
            .send()
            .await?
            .json::<ApiResponse<UploadSession>>()
            .await?;
        if response.code != 0 {
            return Err(Box::new(CloudreveError::from_u32(response.code)));
        }
        Ok(response.data)
    }

    pub async fn upload_chunk(
        &self,
        session_id: &str,
        index: u64,
        chunk: &[u8],
    ) -> Result<(), Box<dyn Error>> {
        let url = format!(
            "{}{}/{}/{}",
            self.base_url, self.api_paths.upload_chunk, session_id, index
        );
        let response = self
            .apply_auth(self.client.post(url))
            .header(reqwest::header::CONTENT_LENGTH, chunk.len() as u64)
            .body(chunk.to_vec())
            .send()
            .await?
            .json::<ApiResponse<Value>>()
            .await?;
        if response.code != 0 {
            return Err(Box::new(CloudreveError::from_u32(response.code)));
        }
        Ok(())
    }

    pub async fn patch_metadata(
        &self,
        uris: Vec<String>,
        patches: Vec<MetadataPatch>,
    ) -> Result<(), Box<dyn Error>> {
        let url = format!("{}{}", self.base_url, self.api_paths.patch_metadata);
        let response = self
            .apply_auth(self.client.patch(url))
            .json(&serde_json::json!({
                "uris": uris,
                "patches": patches
            }))
            .send()
            .await?
            .json::<ApiResponse<Value>>()
            .await?;
        if response.code != 0 {
            return Err(Box::new(CloudreveError::from_u32(response.code)));
        }
        Ok(())
    }

    pub async fn delete_files(
        &self,
        uris: Vec<String>,
        skip_soft_delete: bool,
    ) -> Result<(), Box<dyn Error>> {
        if uris.is_empty() {
            return Ok(());
        }
        let url = format!("{}{}", self.base_url, self.api_paths.delete_file);
        let response = self
            .apply_auth(self.client.delete(url))
            .json(&serde_json::json!({
                "uris": uris,
                "skip_soft_delete": skip_soft_delete,
                "unlink": false
            }))
            .send()
            .await?
            .json::<ApiResponse<Value>>()
            .await?;
        if response.code != 0 {
            return Err(Box::new(CloudreveError::from_u32(response.code)));
        }
        Ok(())
    }

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

    fn apply_auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(token) = &self.access_token {
            request.bearer_auth(token)
        } else {
            request
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataPatch {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove: Option<bool>,
}

pub async fn password_sign_in(
    base_url: &str,
    email: &str,
    password: &str,
    captcha: Option<&str>,
    ticket: Option<&str>,
) -> Result<LoginResponse, Box<dyn Error>> {
    let base_url = if base_url.ends_with("/api/v4") {
        base_url.to_string()
    } else if base_url.ends_with('/') {
        format!("{}api/v4", base_url.trim_end_matches('/'))
    } else {
        format!("{}/api/v4", base_url)
    };
    let url = format!("{}/session/token", base_url);
    let mut body = serde_json::json!({
        "email": email,
        "password": password
    });
    if let Some(captcha) = captcha {
        body["captcha"] = serde_json::json!(captcha);
    }
    if let Some(ticket) = ticket {
        body["ticket"] = serde_json::json!(ticket);
    }
    let response = reqwest::Client::new()
        .post(url)
        .json(&body)
        .send()
        .await?
        .json::<ApiResponse<LoginResponse>>()
        .await?;
    if response.code != 0 {
        return Err(Box::new(CloudreveError::from_u32(response.code)));
    }
    Ok(response.data)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptchaData {
    pub image: String,
    pub ticket: String,
}

pub async fn get_captcha(base_url: &str) -> Result<CaptchaData, Box<dyn Error>> {
    let base_url = if base_url.ends_with("/api/v4") {
        base_url.to_string()
    } else if base_url.ends_with('/') {
        format!("{}api/v4", base_url.trim_end_matches('/'))
    } else {
        format!("{}/api/v4", base_url)
    };
    let url = format!("{}/site/captcha", base_url);
    let response = reqwest::Client::new()
        .get(url)
        .send()
        .await?
        .json::<ApiResponse<CaptchaData>>()
        .await?;
    if response.code != 0 {
        return Err(Box::new(CloudreveError::from_u32(response.code)));
    }
    Ok(response.data)
}
