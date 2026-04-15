use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Top-level: map of AWS profile name -> profile credentials.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SavedCredentials {
    #[serde(default)]
    pub profiles: HashMap<String, ProfileCredentials>,
}

/// Per-profile: map of service -> instances.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileCredentials {
    #[serde(default)]
    pub rds: HashMap<String, RdsCredential>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RdsCredential {
    pub username: String,
    pub password: String, // base64 encoded
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
}

fn credentials_path() -> Option<PathBuf> {
    let config = dirs::config_dir()?;
    Some(config.join("lazy-aws").join("credentials.json"))
}

pub fn load() -> SavedCredentials {
    let path = match credentials_path() {
        Some(p) => p,
        None => return SavedCredentials::default(),
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return SavedCredentials::default(),
    };
    serde_json::from_str(&content).unwrap_or_default()
}

pub fn save(creds: &SavedCredentials) -> Result<(), String> {
    let path = credentials_path().ok_or("cannot determine config directory")?;

    // Create parent directory
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
    }

    let json = serde_json::to_string_pretty(creds).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(&path, &json).map_err(|e| format!("write: {e}"))?;

    // Set file permissions to 0600 (owner read/write only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&path, perms).map_err(|e| format!("chmod: {e}"))?;
    }

    log::info!("saved credentials to {}", path.display());
    Ok(())
}

pub fn encode_password(plain: &str) -> String {
    STANDARD.encode(plain)
}

pub fn decode_password(encoded: &str) -> Result<String, String> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|e| format!("base64 decode: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        let plain = "my-secret-password!@#$%";
        let encoded = encode_password(plain);
        assert_ne!(encoded, plain);
        let decoded = decode_password(&encoded).unwrap();
        assert_eq!(decoded, plain);
    }

    #[test]
    fn decode_invalid_base64() {
        assert!(decode_password("not-valid-base64!!!").is_err());
    }

    #[test]
    fn serialize_deserialize() {
        let mut creds = SavedCredentials::default();
        let mut profile = ProfileCredentials::default();
        profile.rds.insert(
            "my-db".to_string(),
            RdsCredential {
                username: "admin".to_string(),
                password: encode_password("secret"),
                database: Some("mydb".to_string()),
            },
        );
        creds.profiles.insert("my-profile".to_string(), profile);
        let json = serde_json::to_string_pretty(&creds).unwrap();
        let loaded: SavedCredentials = serde_json::from_str(&json).unwrap();
        let cred = loaded.profiles["my-profile"].rds.get("my-db").unwrap();
        assert_eq!(cred.username, "admin");
        assert_eq!(decode_password(&cred.password).unwrap(), "secret");
        assert_eq!(cred.database.as_deref(), Some("mydb"));
    }
}
