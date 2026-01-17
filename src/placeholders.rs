use std::error::Error;
use std::fs;
use std::path::Path;

#[cfg(target_os = "windows")]
pub fn create_placeholder(path: &Path, size: u64, modified: &str) -> Result<(), Box<dyn Error>> {
    use std::os::windows::fs::OpenOptionsExt;
    use std::fs::OpenOptions;

    const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;
    const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x0004_0000;
    let mut options = OpenOptions::new();
    options.write(true).create(true);
    options.attributes(FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS | FILE_ATTRIBUTE_RECALL_ON_OPEN);
    let file = options.open(path)?;
    file.set_len(0)?;
    let meta_path = path.with_extension("cloudreve.placeholder.json");
    let payload = serde_json::json!({
        "size": size,
        "modified": modified,
    });
    fs::write(meta_path, serde_json::to_vec_pretty(&payload)?)?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn create_placeholder(_path: &Path, _size: u64, _modified: &str) -> Result<(), Box<dyn Error>> {
    Err("placeholders are only supported on Windows 11".into())
}
