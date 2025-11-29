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
            Some(items) => {
                out::debug(SCRIPT, &format!("Found {} selected items in database", items.len()));
                items
            },
            None => {
                out::debug(SCRIPT, "No selected items found in database");
                return Ok(());
            },
        };

        // Get folders from SSSS server (uses indexed data)
        let folders = match Self::get_folders().await {
            Ok(f) => {
                out::debug(SCRIPT, &format!("Retrieved {} folders from server", f.len()));
                f
            },
            Err(e) => {
                out::error(SCRIPT, &format!("Failed to get folders from server: {}", e));
                return Ok(()); // If we can't get folders, skip this update
            },
        };

        let home = home_dir().ok_or("Failed to get home directory")?;
        let home_str = home.to_string_lossy().to_string();

        let mut in_cloud_count = 0;
        let mut created_count = 0;
        let mut skipped_count = 0;

        for item in selected_items {
            // Only create virtual files for items that are in_cloud (not synced)
            if item.sync_status != SyncStatus::InCloud {
                skipped_count += 1;
                continue;
            }
            in_cloud_count += 1;

            // Find the folder
            let folder = match folders.iter().find(|f| f.id == item.root) {
                Some(f) => f,
                None => {
                    let available_ids: Vec<String> = folders.iter().map(|f| f.id.clone()).collect();
                    out::warning(SCRIPT, &format!("Folder with id '{}' not found in Syncthing. Available folder IDs: [{}]", 
                        item.root, 
                        available_ids.join(", ")));
                    out::warning(SCRIPT, &format!("Item path: '{}', root: '{}'", item.path, item.root));
                    skipped_count += 1;
                    continue;
                },
            };

            let folder_path = folder.path.replace("~", &home_str);
            let full_path = PathBuf::from(&folder_path).join(&item.path);

            out::debug(SCRIPT, &format!("Processing item: {} -> {:?}", item.path, full_path));

            // Check if file exists
            if full_path.exists() {
                // Check if it's already a virtual file
                if Self::is_virtual_file(&full_path) {
                    out::debug(SCRIPT, &format!("File exists and is already virtual: {:?}", full_path));
                    skipped_count += 1;
                    continue;
                } else {
                    out::debug(SCRIPT, &format!("File already exists and is synced (not virtual), skipping: {:?}", full_path));
                    skipped_count += 1;
                    continue; // File already exists and is real, skip
                }
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
                match Self::create_virtual_file(&full_path) {
                    Ok(_) => {
                        created_count += 1;
                        out::ok(SCRIPT, &format!("Created virtual file: {:?}", full_path));
                    },
                    Err(e) => {
                        out::error(SCRIPT, &format!("Failed to create virtual file {:?}: {}", full_path, e));
                        skipped_count += 1;
                    },
                }
            } else {
                match fs::create_dir_all(&full_path) {
                    Ok(_) => {
                        match Self::mark_as_virtual(&full_path) {
                            Ok(_) => {
                                created_count += 1;
                                out::ok(SCRIPT, &format!("Created virtual directory: {:?}", full_path));
                            },
                            Err(e) => {
                                out::error(SCRIPT, &format!("Failed to mark directory as virtual {:?}: {}", full_path, e));
                                skipped_count += 1;
                            },
                        }
                    },
                    Err(e) => {
                        out::error(SCRIPT, &format!("Failed to create virtual directory {:?}: {}", full_path, e));
                        skipped_count += 1;
                    },
                }
            }
        }

        if in_cloud_count > 0 {
            out::ok(SCRIPT, &format!("Virtual files update: {} in_cloud items, {} created, {} skipped", 
                in_cloud_count, created_count, skipped_count));
        } else {
            out::debug(SCRIPT, "No items with in_cloud status found");
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

    /// Get folders from Syncthing API directly
    /// Note: The virtual files manager runs in the background without a session, so it can't
    /// authenticate with the SSSS server. We get folder information directly from Syncthing.
    /// The SSSS server's indexed data is used for browsing files within folders (via the web UI).
    async fn get_folders() -> Result<Vec<crate::structs::Folder>, Box<dyn std::error::Error>> {
        let client = config::create_http_client()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
        let conn = database_connect()?;
        
        let st_url = get_site_setting(&conn, "st-url")
            .ok_or_else(|| "st-url not found in settings".to_string())?;
        let st_api_key = get_site_setting(&conn, "api-key")
            .ok_or_else(|| "api-key not found in settings".to_string())?;
        
        // Ensure URL has protocol
        let base_url = if st_url.value.starts_with("http://") || st_url.value.starts_with("https://") {
            st_url.value.clone()
        } else {
            format!("https://{}", st_url.value)
        };
        
        let url = format!("{}/rest/config/folders", base_url);
        out::debug(SCRIPT, &format!("Fetching folders from Syncthing: {}", url));
        
        let response = client
            .get(&url)
            .header("X-API-Key", st_api_key.value)
            .send()
            .await
            .map_err(|e| format!("Failed to send request to Syncthing: {}", e))?;
        
        if !response.status().is_success() {
            return Err(format!("Syncthing API returned error: {} ({})", 
                response.status(), 
                response.status().canonical_reason().unwrap_or("Unknown")).into());
        }
        
        let folders: Vec<crate::structs::Folder> = response.json().await
            .map_err(|e| format!("Failed to parse Syncthing response: {}", e))?;
        
        out::debug(SCRIPT, &format!("Retrieved {} folders from Syncthing", folders.len()));
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
                // Set FILE_ATTRIBUTE_OFFLINE to mark file as virtual/offline
                let result = SetFileAttributesW(path_str.as_ptr(), FILE_ATTRIBUTE_OFFLINE);
                if result == 0 {
                    // GetLastError would require errhandlingapi, but we can just report failure
                    return Err(format!("SetFileAttributesW failed for path: {:?}", path).into());
                }
                out::debug(SCRIPT, &format!("Marked as virtual (OFFLINE): {:?}", path));
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

