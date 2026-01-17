use std::collections::HashMap;
use lazy_static::lazy_static;
use std::sync::RwLock;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::hash::Hasher;
use crate::core::error::CloudreveError;

pub mod auth;
pub mod common;
pub mod user;

use user::User;
pub struct Connection {
    client: reqwest::Client,
    access_token: String,
    refresh_token: String,
    base_url: String,
}

impl Connection {
    pub fn new(base_url: String) -> Self {
        let client = reqwest::Client::new();
        let access_token = String::new();
        let refresh_token = String::new();
        let base_url = if base_url.ends_with("/api/v4") {
            base_url
        } else if base_url.ends_with("/") {
            base_url + "api/v4"
        } else {
            base_url + "/api/v4"
        };

        Self { client, access_token, refresh_token, base_url }
    }

    pub fn set_base_url(&mut self, base_url: String) {
        let base_url = if base_url.ends_with("/api/v4") {
            base_url
        } else if base_url.ends_with("/api/v4/") {
            base_url.strip_suffix("/").unwrap_or(base_url.as_str()).to_string()
        } else if base_url.ends_with("/") {
            base_url + "api/v4"
        } else {
            base_url + "/api/v4"
        };
        self.base_url = base_url;
    }

    pub async fn ping(&self) -> Result<(), Box<dyn Error>> {
        let response: Response<String> = self.get("/site/ping").await?;
        if response.code == 0 {
            return Ok(())
        } else {
            let err: CloudreveError = CloudreveError::from_u32(response.code);
            Err(Box::new(err))
        }
    }

    async fn get<T>(&self, uri: &str) -> Result<Response<T>, Box<dyn Error>>
    where
        T: Serialize + for<'de> Deserialize<'de> + Clone
    {
        let res = self.get_with_query(uri, HashMap::new())
            .await?;
        Ok(res)
    }
    async fn get_with_query<T>(&self, uri: &str, query: HashMap<String, String>) -> Result<Response<T>, Box<dyn Error>>
    where
        T: Serialize + for<'de> Deserialize<'de> + Clone
    {
        let mut my_uri = if uri.ends_with("/") {
            uri.to_string().strip_suffix("/").unwrap_or(uri).to_string()
        } else {
            uri.to_string()
        };
        my_uri += "&";
        for pair in query {
            my_uri = my_uri + urlencoding::encode(&*pair.0).as_ref() + "=" + urlencoding::encode(&*pair.1).as_ref() + "&";
        }
        let my_uri = my_uri.strip_suffix("&").unwrap_or(my_uri.as_str());
        let text = self.client.get(self.base_url.clone() + my_uri)
            .send()
            .await?
            .text()
            .await?;
        let res: Response<T> = serde_json::from_str(text.as_str())?;
        Ok(res)
    }

    async fn post<T, S>(&self, uri: &str, body: S) -> Result<Response<T>, Box<dyn Error>>
    where
        T: Serialize + for<'de> Deserialize<'de> + Clone,
        S: Serialize + for<'de> Deserialize<'de> + Clone
    {
        let res = self.post_with_query(uri, body, HashMap::new())
            .await?;
        Ok(res)
    }

    async fn post_with_query<T, S>(&self, uri: &str, body: S, query: HashMap<String, String>) -> Result<Response<T>, Box<dyn Error>>
    where
        T: Serialize + for<'de> Deserialize<'de> + Clone,
        S: Serialize + for<'de> Deserialize<'de> + Clone{
        let mut my_uri = if uri.ends_with("/") {
            uri.to_string().strip_suffix("/").unwrap_or(uri).to_string()
        } else {
            uri.to_string()
        };
        my_uri += "&";
        for pair in query {
            my_uri = my_uri + urlencoding::encode(&*pair.0).as_ref() + "=" + urlencoding::encode(&*pair.1).as_ref() + "&";
        }
        let my_uri = my_uri.strip_suffix("&").unwrap_or(my_uri.as_str());
        let text = self.client.post(self.base_url.clone() + my_uri)
            .json(&body)
            .send()
            .await?
            .text()
            .await?;
        let res: Response<T> = serde_json::from_str(text.as_str())?;
        Ok(res)
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Response<T>
{
    data: T ,
    code: u32,
    msg: String,
}

impl<T> Response<T>
where T: Serialize + for<'de> Deserialize<'de> + Clone{
    pub fn new(data: T, code: u32, msg: String) -> Self {
        Self{ data, code, msg }
    }

    pub fn data(&self) -> T {
        self.data.clone()
    }

    pub fn code(&self) -> u32 {
        self.code
    }

    pub fn msg(&self) -> String {
        self.msg.clone()
    }
}

