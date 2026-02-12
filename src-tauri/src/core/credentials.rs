use std::error::Error;

const SERVICE_NAME: &str = "cloudreve-sync";

#[derive(Debug, Clone)]
pub struct StoredToken {
    pub access_token: String,
    pub refresh_token: String,
}

pub fn store_tokens(
    account: &str,
    access_token: &str,
    refresh_token: &str,
) -> Result<(), Box<dyn Error>> {
    let entry = keyring::Entry::new(SERVICE_NAME, account)?;
    let payload = format!("{}\n{}", access_token, refresh_token);
    entry.set_password(&payload)?;
    Ok(())
}

pub fn load_tokens(account: &str) -> Result<StoredToken, Box<dyn Error>> {
    let entry = keyring::Entry::new(SERVICE_NAME, account)?;
    let payload = entry.get_password()?;
    let mut parts = payload.splitn(2, '\n');
    let access_token = parts.next().unwrap_or_default().to_string();
    let refresh_token = parts.next().unwrap_or_default().to_string();
    Ok(StoredToken {
        access_token,
        refresh_token,
    })
}

pub fn clear_tokens(account: &str) -> Result<(), Box<dyn Error>> {
    let entry = keyring::Entry::new(SERVICE_NAME, account)?;
    entry.delete_password()?;
    Ok(())
}
