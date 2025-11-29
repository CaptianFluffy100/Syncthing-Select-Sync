use std::fs;
use std::path::{Path, PathBuf};
use dirs;
use crate::out;

const SCRIPT: &str = "SYNCTHING_CONFIG";

/// Find and read Syncthing configuration file
pub struct SyncthingConfig {
    config_path: Option<PathBuf>,
}

impl SyncthingConfig {
    pub fn new() -> Self {
        Self {
            config_path: Self::find_config_file(),
        }
    }

    /// Find the Syncthing config file on the current platform
    fn find_config_file() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            // Windows: Check %LOCALAPPDATA%\Syncthing\config.xml and %APPDATA%\Syncthing\config.xml
            if let Some(local_app_data) = std::env::var("LOCALAPPDATA").ok() {
                let path = PathBuf::from(local_app_data).join("Syncthing").join("config.xml");
                if path.exists() {
                    out::ok(SCRIPT, &format!("Found Syncthing config at: {:?}", path));
                    return Some(path);
                }
            }

            if let Some(app_data) = std::env::var("APPDATA").ok() {
                let path = PathBuf::from(app_data).join("Syncthing").join("config.xml");
                if path.exists() {
                    out::ok(SCRIPT, &format!("Found Syncthing config at: {:?}", path));
                    return Some(path);
                }
            }

            // Also check common installation paths
            let common_paths = vec![
                PathBuf::from("C:\\ProgramData\\Syncthing\\config.xml"),
                PathBuf::from("C:\\Syncthing\\config.xml"),
            ];

            for path in common_paths {
                if path.exists() {
                    out::ok(SCRIPT, &format!("Found Syncthing config at: {:?}", path));
                    return Some(path);
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Linux: Check ~/.config/syncthing/config.xml
            if let Some(home) = dirs::home_dir() {
                let path = home.join(".config").join("syncthing").join("config.xml");
                if path.exists() {
                    out::ok(SCRIPT, &format!("Found Syncthing config at: {:?}", path));
                    return Some(path);
                }
            }

            // Check system-wide location
            let system_path = PathBuf::from("/var/syncthing/config.xml");
            if system_path.exists() {
                out::ok(SCRIPT, &format!("Found Syncthing config at: {:?}", system_path));
                return Some(system_path);
            }

            // Check /etc/syncthing/config.xml
            let etc_path = PathBuf::from("/etc/syncthing/config.xml");
            if etc_path.exists() {
                out::ok(SCRIPT, &format!("Found Syncthing config at: {:?}", etc_path));
                return Some(etc_path);
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS: Check ~/Library/Application Support/Syncthing/config.xml
            if let Some(home) = dirs::home_dir() {
                let path = home
                    .join("Library")
                    .join("Application Support")
                    .join("Syncthing")
                    .join("config.xml");
                if path.exists() {
                    out::ok(SCRIPT, &format!("Found Syncthing config at: {:?}", path));
                    return Some(path);
                }
            }
        }

        out::warning(SCRIPT, "Syncthing config file not found");
        None
    }

    /// Read the API key from the Syncthing config file
    pub fn get_api_key(&self) -> Option<String> {
        let config_path = self.config_path.as_ref()?;
        
        match fs::read_to_string(config_path) {
            Ok(content) => {
                Self::parse_api_key(&content)
            }
            Err(e) => {
                out::error(SCRIPT, &format!("Failed to read config file: {}", e));
                None
            }
        }
    }

    /// Parse the API key from XML content
    fn parse_api_key(xml_content: &str) -> Option<String> {
        // Simple XML parsing - look for <apikey> tag
        // The config file structure is: <configuration><gui><apikey>...</apikey></gui></configuration>
        
        // Find the apikey tag
        if let Some(start) = xml_content.find("<apikey>") {
            let start_pos = start + 8; // Length of "<apikey>"
            if let Some(end) = xml_content[start_pos..].find("</apikey>") {
                let api_key = xml_content[start_pos..start_pos + end].trim().to_string();
                if !api_key.is_empty() {
                    out::ok(SCRIPT, "Successfully extracted API key from config");
                    return Some(api_key);
                }
            }
        }

        // Try alternative format (with attributes)
        if let Some(start) = xml_content.find("apikey=\"") {
            let start_pos = start + 8; // Length of "apikey=\""
            if let Some(end) = xml_content[start_pos..].find("\"") {
                let api_key = xml_content[start_pos..start_pos + end].trim().to_string();
                if !api_key.is_empty() {
                    out::ok(SCRIPT, "Successfully extracted API key from config (attribute format)");
                    return Some(api_key);
                }
            }
        }

        out::warning(SCRIPT, "API key not found in config file");
        None
    }

    /// Get the config file path if found
    pub fn config_path(&self) -> Option<&Path> {
        self.config_path.as_deref()
    }

    /// Auto-configure the API key in the database if found
    pub fn auto_configure_api_key(&self) -> bool {
        if let Some(api_key) = self.get_api_key() {
            let conn = crate::database::db::database_connect();

            // Check if api-key is already set and not "NULL"
            if let Some(setting) = crate::database::db::get_site_setting(&conn, "api-key") {
                if setting.value != "NULL" && !setting.value.is_empty() {
                    out::info(SCRIPT, "API key already configured, skipping auto-configuration");
                    return true;
                }
            }

            // Set the API key
            if crate::database::db::set_site_setting(&conn, "api-key", &api_key) {
                out::ok(SCRIPT, "Auto-configured API key from Syncthing config");
                return true;
            } else {
                out::error(SCRIPT, "Failed to save API key to database");
                return false;
            }
        }
        false
    }
}

