use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::time::{interval, Duration};
use dirs::home_dir;
use crate::{out, database::db::{database_connect, get_selected_items, get_site_setting}, structs::SyncStatus};
use crate::config;

const SCRIPT: &str = "VIRTUAL_FILES";

/// Manages virtual files in the file system
/// Creates placeholder files for items in sync list that don't exist locally
pub struct VirtualFileManager {
    running: Arc<tokio::sync::RwLock<bool>>,
}

impl VirtualFileManager {
    pub fn new() -> Self {
        Self {
            running: Arc::new(tokio::sync::RwLock::new(false)),
        }
    }

    /// Start the virtual file manager service
    pub async fn start(&self) {
        let mut running = self.running.write().await;
        if *running {
            out::warning(SCRIPT, "Virtual file manager already running");
            return;
        }
        *running = true;
        drop(running);

        out::ok(SCRIPT, "Starting virtual file manager");

        let running_clone = Arc::clone(&self.running);
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5)); // Check every 5 seconds
            loop {
                interval.tick().await;
                
                let is_running = *running_clone.read().await;
                if !is_running {
                    break;
                }

                if let Err(e) = Self::update_virtual_files().await {
                    out::error(SCRIPT, &format!("Error updating virtual files: {}", e));
                }
            }
            out::ok(SCRIPT, "Virtual file manager stopped");
        });
    }

    /// Stop the virtual file manager
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        out::ok(SCRIPT, "Stopping virtual file manager");
    }

    /// Update virtual files based on sync list
    async fn update_virtual_files() -> Result<(), Box<dyn std::error::Error>> {
        let conn = database_connect()?;
        let selected_items = match get_selected_items(&conn)? {
            Some(items) => items,
            None => return Ok(()),
        };

        // Get folders from Syncthing API
        let folders = match Self::get_folders().await {
            Ok(f) => f,
            Err(_) => return Ok(()), // If we can't get folders, skip this update
        };

        let home = home_dir().ok_or("Failed to get home directory")?;
        let home_str = home.to_string_lossy().to_string();

        for item in selected_items {
            // Only create virtual files for items that are in_cloud (not synced)
            if item.sync_status != SyncStatus::InCloud {
                continue;
            }

            // Find the folder
            let folder = match folders.iter().find(|f| f.id == item.root) {
                Some(f) => f,
                None => continue,
            };

            let folder_path = folder.path.replace("~", &home_str);
            let full_path = PathBuf::from(&folder_path).join(&item.path);

            // Check if file exists
            if full_path.exists() {
                continue; // File already exists, skip
            }

            // Create parent directories if needed
            if let Some(parent) = full_path.parent() {
                if let Err(e) = fs::create_dir_all(parent) {
                    out::error(SCRIPT, &format!("Failed to create parent directory {:?}: {}", parent, e));
                    continue;
                }
            }

            // Create placeholder file or directory
            if item.is_file {
                Self::create_virtual_file(&full_path)?;
            } else {
                if let Err(e) = fs::create_dir_all(&full_path) {
                    out::error(SCRIPT, &format!("Failed to create virtual directory {:?}: {}", full_path, e));
                } else {
                    Self::mark_as_virtual(&full_path)?;
                }
            }
        }

        Ok(())
    }

    /// Create a virtual file placeholder
    fn create_virtual_file(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // Create an empty file
        fs::File::create(path)?;
        
        // Mark as virtual using Windows file attributes
        #[cfg(target_os = "windows")]
        {
            Self::mark_as_virtual(path)?;
        }

        out::debug(SCRIPT, &format!("Created virtual file: {:?}", path));
        Ok(())
    }

    /// Get folders from Syncthing API
    async fn get_folders() -> Result<Vec<crate::structs::Folder>, Box<dyn std::error::Error>> {
        let client = config::create_http_client()?;
        let conn = database_connect()?;
        
        let ss = get_site_setting(&conn, "st-url")
            .ok_or("st-url not found in settings")?;
        let st_api_key = get_site_setting(&conn, "api-key")
            .ok_or("api-key not found in settings")?;
        
        let response = client
            .get(format!("https://{}/rest/config/folders", ss.value))
            .header("X-API-Key", st_api_key.value)
            .send()
            .await?;
        
        let folders: Vec<crate::structs::Folder> = response.json().await?;
        Ok(folders)
    }

    /// Mark a file as virtual using file attributes
    fn mark_as_virtual(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::winnt::FILE_ATTRIBUTE_OFFLINE;
            use winapi::um::fileapi::SetFileAttributesW;
            use std::os::windows::ffi::OsStrExt;

            let path_str: Vec<u16> = path.as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            unsafe {
                SetFileAttributesW(path_str.as_ptr(), FILE_ATTRIBUTE_OFFLINE);
            }
        }

        // On other platforms, we could use extended attributes
        #[cfg(not(target_os = "windows"))]
        {
            // Could use xattr crate for extended attributes
            // For now, just mark as read-only or use a different method
        }

        Ok(())
    }

    /// Remove virtual file when it's actually synced
    pub async fn remove_virtual_marker(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(target_os = "windows")]
        {
            use winapi::um::fileapi::SetFileAttributesW;
            use std::os::windows::ffi::OsStrExt;
            use winapi::um::winnt::FILE_ATTRIBUTE_NORMAL;

            let path_str: Vec<u16> = path.as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            unsafe {
                SetFileAttributesW(path_str.as_ptr(), FILE_ATTRIBUTE_NORMAL);
            }
        }
        Ok(())
    }

    /// Check if a file is virtual (exists but is a placeholder)
    pub fn is_virtual_file(path: &Path) -> bool {
        if !path.exists() {
            return false;
        }

        #[cfg(target_os = "windows")]
        {
            use winapi::um::fileapi::GetFileAttributesW;
            use std::os::windows::ffi::OsStrExt;
            use winapi::um::winnt::FILE_ATTRIBUTE_OFFLINE;

            let path_str: Vec<u16> = path.as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            unsafe {
                let attrs = GetFileAttributesW(path_str.as_ptr());
                if attrs != winapi::um::fileapi::INVALID_FILE_ATTRIBUTES {
                    return (attrs & FILE_ATTRIBUTE_OFFLINE) != 0;
                }
            }
        }

        false
    }

    /// Handle file access - if it's a virtual file, trigger sync
    pub async fn handle_file_access(path: &Path) -> Result<bool, Box<dyn std::error::Error>> {
        if Self::is_virtual_file(path) {
            out::ok(SCRIPT, &format!("Virtual file accessed: {:?}, triggering sync", path));
            // TODO: Trigger actual file sync here
            // For now, we'll let the sync_file endpoint handle it
            return Ok(true);
        }
        Ok(false)
    }
}

