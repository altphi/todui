use std::fs;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncConfig {
    pub server_url: String,
    pub token: String,
    pub email: String,
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("todui")
        .join("sync.json")
}

pub fn load_config() -> Option<SyncConfig> {
    let path = config_path();
    let contents = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&contents).ok()
}

#[allow(dead_code)]
pub fn save_config(config: &SyncConfig) -> io::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config).map_err(|e| io::Error::other(e.to_string()))?;
    fs::write(&path, json)
}

#[allow(dead_code)]
pub fn clear_config() -> io::Result<()> {
    let path = config_path();
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sync.json");

        let config = SyncConfig {
            server_url: "https://example.com".into(),
            token: "test-token".into(),
            email: "test@example.com".into(),
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(&path, &json).unwrap();

        let loaded: SyncConfig =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded.server_url, "https://example.com");
        assert_eq!(loaded.token, "test-token");
        assert_eq!(loaded.email, "test@example.com");
    }
}
