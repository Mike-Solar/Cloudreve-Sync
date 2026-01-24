use std::error::Error;
use serde::{Deserialize, Serialize};
use crate::core::error::CloudreveError;
use crate::core::requests::Connection;
use crate::core::requests::Response;
use crate::core::requests::user::User;

#[derive(Serialize, Deserialize, Clone)]
pub struct CaptchaData{
    pub image:String,
    pub ticket:String,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct LoginData{
    pub token: Token,
    pub user: User,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct Token{
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires: String,
    pub refresh_expires: String,
}


impl Connection{
    pub async fn get_captcha(&self) -> Result<CaptchaData, Box<dyn Error>> {
        let response: Response<CaptchaData>=self.get("/site/captcha").await?;
        if response.code == 0{
            Ok(response.data)
        }else{
            Err(Box::new(CloudreveError::from_u32(response.code)))
        }
    }
    
}
