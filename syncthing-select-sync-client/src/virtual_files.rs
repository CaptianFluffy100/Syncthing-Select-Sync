use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::time::{interval, Duration};
use tokio::sync::{RwLock, mpsc};
use dirs::home_dir;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use crate::{out, database::db::{database_connect, get_selected_items, get_site_setting, create_selected_item}, structs::SyncStatus};
use crate::config;

const SCRIPT: &str = "VIRTUAL_FILES";
const ENABLE_LOGS: bool = false; // Set to true to enable all logging
const ENABLE_FILE_ACCESS_LOGS: bool = true; // Always log file access events

// Macro to conditionally log
macro_rules! vf_log {
    ($level:ident, $($arg:tt)*) => {
        if ENABLE_LOGS {
            out::$level(SCRIPT, &format!($($arg)*));
        }
    };
}

// Macro to always log file access events
macro_rules! vf_access_log {
    ($level:ident, $($arg:tt)*) => {
        if ENABLE_FILE_ACCESS_LOGS {
            out::$level(SCRIPT, &format!($($arg)*));
        }
    };
}

/// Manages virtual files in the file system
/// Creates placeholder files for items in sync list that don't exist locally
pub struct VirtualFileManager {
    running: Arc<tokio::sync::RwLock<bool>>,
    watchers: Arc<RwLock<HashMap<String, notify::RecommendedWatcher>>>,
}

impl VirtualFileManager {
    pub fn new() -> Self {
        Self {
            running: Arc::new(tokio::sync::RwLock::new(false)),
            watchers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the virtual file manager service
    pub async fn start(&self) {
        let mut running = self.running.write().await;
        if *running {
            vf_log!(warning, "Virtual file manager already running");
            return;
        }
        *running = true;
        drop(running);

        vf_log!(ok, "Starting virtual file manager");

        // Create channel for file access events
        let (tx, mut rx) = mpsc::channel::<PathBuf>(100);

        // Start file system watchers
        let watchers_clone = Arc::clone(&self.watchers);
        if let Err(e) = Self::start_file_watchers(watchers_clone, tx).await {
            vf_log!(error, "Failed to start file watchers: {}", e);
        }

        let running_clone = Arc::clone(&self.running);
        // Spawn task to process file access events immediately
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(path) = rx.recv() => {
                        // Spawn each file access in its own task for immediate processing
                        let path_clone = path.clone();
                        tokio::spawn(async move {
                            if path_clone.exists() && Self::is_virtual_file(&path_clone) {
                                vf_access_log!(ok, "Virtual file accessed, triggering sync: {:?}", path_clone);
                                if let Err(e) = Self::handle_file_access(&path_clone).await {
                                    vf_access_log!(error, "Failed to handle file access for {:?}: {}", path_clone, e);
                                }
                            }
                        });
                    }
                    _ = tokio::time::sleep(Duration::from_millis(10)) => {
                        // Check if still running
                        let is_running = *running_clone.read().await;
                        if !is_running {
                            break;
                        }
                    }
                }
            }
        });

        // Add a background task to check file access times as a fallback
        // This helps catch file access that the watcher might miss on Windows
        let running_clone = Arc::clone(&self.running);
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(200)); // Check every 200ms for faster response
            let mut last_access_times: HashMap<PathBuf, std::time::SystemTime> = HashMap::new();
            let mut virtual_files_cache: HashMap<PathBuf, bool> = HashMap::new();
            let mut cache_update_counter = 0;
            let now = std::time::SystemTime::now();
            
            loop {
                interval.tick().await;
                
                let is_running = *running_clone.read().await;
                if !is_running {
                    break;
                }

                // Update cache of virtual files every 2 seconds (every 10 iterations at 200ms)
                cache_update_counter += 1;
                if cache_update_counter >= 10 {
                    cache_update_counter = 0;
                    virtual_files_cache.clear();
                    
                    // Build cache of virtual files
                    if let Ok(folders) = Self::get_folders().await {
                        let home = home_dir();
                        if let Some(home) = home {
                            let home_str = home.to_string_lossy().to_string();
                            
                            for folder in folders {
                                let folder_path = folder.path.replace("~", &home_str);
                                let folder_path_buf = PathBuf::from(&folder_path);
                                
                                if let Ok(files) = Self::get_all_files_from_syncthing(&folder.id).await {
                                    for file in files {
                                        let full_path = folder_path_buf.join(&file.path);
                                        if full_path.exists() && Self::is_virtual_file(&full_path) {
                                            virtual_files_cache.insert(full_path, true);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                // Check all cached virtual files for access time changes
                let current_time = std::time::SystemTime::now();
                for full_path in virtual_files_cache.keys() {
                    if let Ok(metadata) = full_path.metadata() {
                        if let Ok(accessed) = metadata.accessed() {
                            let last_access = last_access_times.get(full_path);
                            
                            // Check if file was accessed recently (within last 1 second)
                            if let Ok(duration) = current_time.duration_since(accessed) {
                                // If file was accessed within the last second, it's a recent access
                                let duration_ms = duration.as_millis();
                                if duration_ms < 1000 {
                                    // Check if this is a new access (either no previous record, or access time changed)
                                    let should_trigger = if let Some(last) = last_access {
                                        // Access time changed
                                        if let Ok(change_duration) = accessed.duration_since(*last) {
                                            change_duration.as_millis() > 0
                                        } else {
                                            false
                                        }
                                    } else {
                                        // First time we're seeing this file - check if access is very recent (within 500ms)
                                        duration_ms < 500
                                    };
                                    
                                    if should_trigger {
                                        // File was accessed, spawn immediate sync in separate task
                                        let path_clone = full_path.clone();
                                        tokio::spawn(async move {
                                            vf_access_log!(ok, "File access detected via access time check: {:?} (accessed {}ms ago)", 
                                                path_clone, duration_ms);
                                            if let Err(e) = Self::handle_file_access(&path_clone).await {
                                                vf_access_log!(error, "Failed to handle file access: {}", e);
                                            }
                                        });
                                    }
                                }
                            }
                            
                            last_access_times.insert(full_path.clone(), accessed);
                        }
                    }
                }
                
                // Clean up old access times (older than 10 seconds)
                last_access_times.retain(|_, time| {
                    time.elapsed().map(|e| e.as_secs() < 10).unwrap_or(false)
                });
            }
        });

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
                    vf_log!(error, "Error updating virtual files: {}", e);
                }
            }
            vf_log!(ok, "Virtual file manager stopped");
        });
    }

    /// Stop the virtual file manager
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
        
        // Stop all watchers
        let mut watchers = self.watchers.write().await;
        watchers.clear();
        
        vf_log!(ok, "Stopping virtual file manager");
    }
    
    /// Start file system watchers for all Syncthing folders
    async fn start_file_watchers(
        watchers: Arc<RwLock<HashMap<String, notify::RecommendedWatcher>>>,
        tx: mpsc::Sender<PathBuf>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let folders = Self::get_folders().await?;
        let home = home_dir().ok_or("Failed to get home directory")?;
        let home_str = home.to_string_lossy().to_string();
        
        let mut watchers_guard = watchers.write().await;
        
        for folder in folders {
            let folder_path = folder.path.replace("~", &home_str);
            let path = PathBuf::from(&folder_path);
            
            if !path.exists() {
                continue;
            }
            
            let folder_id = folder.id.clone();
            let tx_clone = tx.clone();
            
            let mut watcher = notify::recommended_watcher(move |result: Result<Event, notify::Error>| {
                match result {
                    Ok(event) => {
                        // Log all events for debugging (only for file access logs)
                        vf_access_log!(debug, "File system event: {:?} for paths: {:?}", event.kind, event.paths);
                        
                        // Check for ANY file events that might indicate access
                        // On Windows, Access events might not always fire, so we check all events
                        // We're particularly interested in: Access, Modify, Create, Open
                        let is_relevant_event = matches!(event.kind, 
                            EventKind::Access(_) | 
                            EventKind::Modify(_) | 
                            EventKind::Create(_) |
                            EventKind::Any
                        );
                        
                        if is_relevant_event {
                            for path in &event.paths {
                                // Check if this is a virtual file
                                if path.exists() {
                                    // Double-check it's virtual before processing
                                    if Self::is_virtual_file(path) {
                                        vf_access_log!(ok, "Virtual file event detected: {:?} (event: {:?})", path, event.kind);
                                        
                                        // Send to async task for immediate processing
                                        if tx_clone.try_send(path.clone()).is_ok() {
                                            // Successfully queued
                                        } else {
                                            // Channel full, spawn directly
                                            let path_clone = path.clone();
                                            std::thread::spawn(move || {
                                                let rt = tokio::runtime::Runtime::new().unwrap();
                                                rt.block_on(async {
                                                    vf_access_log!(ok, "Processing virtual file access directly (channel full): {:?}", path_clone);
                                                    if let Err(e) = Self::handle_file_access(&path_clone).await {
                                                        vf_access_log!(error, "Failed to handle file access: {}", e);
                                                    }
                                                });
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        vf_access_log!(error, "File watcher error: {}", e);
                    }
                }
            })
            .map_err(|e| format!("Failed to create watcher: {}", e))?;
            
            watcher.watch(&path, RecursiveMode::Recursive)
                .map_err(|e| format!("Failed to watch path: {}", e))?;
            
            watchers_guard.insert(folder.id.clone(), watcher);
            vf_log!(ok, "Started file watcher for folder: {} ({})", folder.id, folder_path);
        }
        
        Ok(())
    }

    /// Update virtual files - creates virtual files for ALL files/folders that are NOT selected to sync
    async fn update_virtual_files() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let conn = database_connect()?;
        
        // Get selected items (these should NOT be virtual - they're meant to be synced)
        let selected_items = match get_selected_items(&conn)? {
            Some(items) => {
                vf_log!(debug, "Found {} selected items (these will NOT be virtual)", items.len());
                items
            },
            None => {
                vf_log!(debug, "No selected items found - all files will be virtual");
                Vec::new()
            },
        };

        // Create a set of selected paths for quick lookup: (folder_id, path)
        use std::collections::HashSet;
        let selected_paths: HashSet<(String, String)> = selected_items
            .iter()
            .map(|item| (item.root.clone(), item.path.clone()))
            .collect();

        // Get folders from Syncthing
        let folders = match Self::get_folders().await {
            Ok(f) => {
                vf_log!(debug, "Retrieved {} folders from Syncthing", f.len());
                f
            },
            Err(e) => {
                vf_log!(error, "Failed to get folders from Syncthing: {}", e);
                return Ok(()); // If we can't get folders, skip this update
            },
        };

        let home = home_dir().ok_or("Failed to get home directory")?;
        let home_str = home.to_string_lossy().to_string();

        let mut total_files = 0;
        let mut created_count = 0;
        let mut skipped_count = 0;
        let mut error_count = 0;

        // Process each folder
        for folder in folders {
            vf_log!(debug, "Processing folder: {} ({})", folder.label, folder.id);
            
            // Get all files/folders in this folder from Syncthing directly (recursively)
            match Self::get_all_files_from_syncthing(&folder.id).await {
                Ok(files) => {
                    vf_log!(debug, "Found {} items in folder {} (recursive)", files.len(), folder.id);
                    
                    let folder_path = folder.path.replace("~", &home_str);
                    
                    for file in files {
                        total_files += 1;
                        
                        // Check if this file/folder is selected to sync
                        let is_selected = selected_paths.contains(&(folder.id.clone(), file.path.clone()));
                        
                        if is_selected {
                            // This item is selected to sync, so it should NOT be virtual
                            // If it exists as virtual, we should remove the virtual marker
                            let full_path = PathBuf::from(&folder_path).join(&file.path);
                            if full_path.exists() && Self::is_virtual_file(&full_path) {
                                // Remove virtual marker since it's now selected to sync
                                if let Err(e) = Self::remove_virtual_marker(&full_path).await {
                                    vf_log!(warning, "Failed to remove virtual marker from {:?}: {}", full_path, e);
                                } else {
                                    vf_log!(debug, "Removed virtual marker from selected item: {:?}", full_path);
                                }
                            }
                            skipped_count += 1;
                            continue;
                        }
                        
                        // This file/folder is NOT selected, so it should be virtual
                        let full_path = PathBuf::from(&folder_path).join(&file.path);
                        
                        // Check if file already exists
                        if full_path.exists() {
                            // Check if it's already a virtual file
                            if Self::is_virtual_file(&full_path) {
                                // Already virtual, skip
                                skipped_count += 1;
                                continue;
                            } else {
                                // File exists but is real - this shouldn't happen for non-selected items
                                // but we'll skip it to avoid overwriting
                                vf_log!(debug, "File exists and is real (not virtual), skipping: {:?}", full_path);
                                skipped_count += 1;
                                continue;
                            }
                        }

                        // Create parent directories if needed
                        if let Some(parent) = full_path.parent() {
                            if let Err(e) = fs::create_dir_all(parent) {
                                vf_log!(error, "Failed to create parent directory {:?}: {}", parent, e);
                                error_count += 1;
                                continue;
                            }
                        }

                        // Create virtual file or directory
                        if file.is_file {
                            match Self::create_virtual_file(&full_path) {
                                Ok(_) => {
                                    created_count += 1;
                                    if created_count % 100 == 0 {
                                        vf_log!(ok, "Created {} virtual files so far...", created_count);
                                    }
                                },
                                Err(e) => {
                                    vf_log!(error, "Failed to create virtual file {:?}: {}", full_path, e);
                                    error_count += 1;
                                },
                            }
                        } else {
                            match fs::create_dir_all(&full_path) {
                                Ok(_) => {
                                    match Self::mark_as_virtual(&full_path) {
                                        Ok(_) => {
                                            created_count += 1;
                                        },
                                        Err(e) => {
                                            vf_log!(error, "Failed to mark directory as virtual {:?}: {}", full_path, e);
                                            error_count += 1;
                                        },
                                    }
                                },
                                Err(e) => {
                                    vf_log!(error, "Failed to create virtual directory {:?}: {}", full_path, e);
                                    error_count += 1;
                                },
                            }
                        }
                    }
                },
                Err(e) => {
                    vf_log!(error, "Failed to get files for folder {}: {}", folder.id, e);
                    error_count += 1;
                },
            }
        }

        if total_files > 0 {
            vf_log!(ok, "Virtual files update: {} total files, {} created, {} skipped, {} errors", 
                total_files, created_count, skipped_count, error_count);
        } else {
            vf_log!(debug, "No files found to process");
        }

        Ok(())
    }
    
    /// Get all files and folders from Syncthing database API (recursively)
    /// Uses the browse endpoint with a large depth to get all files
    async fn get_all_files_from_syncthing(folder_id: &str) -> Result<Vec<crate::structs::FolderFile>, Box<dyn std::error::Error + Send + Sync>> {
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
        
        // Use Syncthing's browse API to get all files recursively
        // levels=-1 means unlimited depth, prefix="" means from root
        let url = format!("{}/rest/db/browse?folder={}&levels=-1&prefix=", base_url, folder_id);
        vf_log!(debug, "Fetching all files from Syncthing: {}", url);
        
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
        
        // Get the raw response text first
        let response_text = response.text().await
            .map_err(|e| format!("Failed to read response text: {}", e))?;
        
        // Log a preview of the response for debugging
        if response_text.len() > 0 {
            let preview = response_text.chars().take(200).collect::<String>();
            vf_log!(debug, "Response preview (first 200 chars): {}", preview);
        } else {
            vf_log!(warning, "Empty response from Syncthing browse API");
            return Ok(Vec::new());
        }
        
        // Parse JSON - Syncthing browse API returns an array of tree structures
        let entries: Vec<serde_json::Value> = serde_json::from_str(&response_text)
            .map_err(|e| {
                vf_log!(error, "Failed to parse JSON. Response preview: {}", 
                    response_text.chars().take(500).collect::<String>());
                format!("Failed to parse Syncthing response: {}. Response length: {}", e, response_text.len())
            })?;
        
        // Recursively traverse the tree structure to collect all files
        let mut files = Vec::new();
        Self::traverse_browse_tree(&entries, "", folder_id, &mut files);
        
        vf_log!(debug, "Retrieved {} files from Syncthing for folder {}", files.len(), folder_id);
        Ok(files)
    }
    
    /// Recursively traverse the Syncthing browse tree structure
    /// Each entry has: name, type (string like "FILE_INFO_TYPE_DIRECTORY"), and optional children array
    fn traverse_browse_tree(
        entries: &[serde_json::Value],
        parent_path: &str,
        folder_id: &str,
        files: &mut Vec<crate::structs::FolderFile>,
    ) {
        for entry in entries {
            // Extract name
            let name = entry.get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            
            // Build full path
            let path = if parent_path.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", parent_path, name)
            };
            
            // Get file type from string (e.g., "FILE_INFO_TYPE_DIRECTORY", "FILE_INFO_TYPE_FILE")
            let type_str = entry.get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            
            let is_file = type_str.contains("FILE") && !type_str.contains("DIRECTORY");
            
            // Add this entry to the files list
            files.push(crate::structs::FolderFile {
                id: folder_id.to_string(),
                name: name.clone(),
                path: path.clone(),
                is_file,
            });
            
            // If it's a directory and has children, recurse
            if !is_file {
                if let Some(children) = entry.get("children").and_then(|v| v.as_array()) {
                    Self::traverse_browse_tree(children, &path, folder_id, files);
                }
            }
        }
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

        vf_log!(debug, "Created virtual file: {:?}", path);
        Ok(())
    }

    /// Get folders from Syncthing API directly
    /// Note: The virtual files manager runs in the background without a session, so it can't
    /// authenticate with the SSSS server. We get folder information directly from Syncthing.
    /// The SSSS server's indexed data is used for browsing files within folders (via the web UI).
    async fn get_folders() -> Result<Vec<crate::structs::Folder>, Box<dyn std::error::Error + Send + Sync>> {
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
        vf_log!(debug, "Fetching folders from Syncthing: {}", url);
        
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
        
        vf_log!(debug, "Retrieved {} folders from Syncthing", folders.len());
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
                vf_log!(debug, "Marked as virtual (OFFLINE): {:?}", path);
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
    /// This is called when a virtual file is detected as being accessed
    /// Adds the file and all parent folders to the sync list
    pub async fn handle_file_access(path: &Path) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if !Self::is_virtual_file(path) {
            return Ok(false);
        }
        
        vf_access_log!(ok, "Virtual file accessed: {:?}, adding to sync list", path);
        
        // Find the file in the database to get root and path
        let conn = database_connect()?;
        let selected_items = match get_selected_items(&conn)? {
            Some(items) => items,
            None => Vec::new(),
        };
        
        // Get folders to match the path
        let folders = match Self::get_folders().await {
            Ok(f) => f,
            Err(e) => {
                vf_access_log!(error, "Failed to get folders: {}", e);
                return Ok(false);
            },
        };
        
        let home = home_dir().ok_or("Failed to get home directory")?;
        let home_str = home.to_string_lossy().to_string();
        
        // Find which folder this file belongs to and get the relative path
        for folder in folders {
            let folder_path = folder.path.replace("~", &home_str);
            let folder_path_buf = PathBuf::from(&folder_path);
            
            if let Ok(relative_path) = path.strip_prefix(&folder_path_buf) {
                let relative_path_str = relative_path.to_string_lossy().replace("\\", "/");
                
                // Add the file and all parent folders to selected items
                let path_parts: Vec<&str> = relative_path_str.split('/').filter(|s| !s.is_empty()).collect();
                
                // Build paths for all parent folders and the file itself
                let mut paths_to_add = Vec::new();
                let mut current_path = String::new();
                
                // Add all parent folders (build cumulative paths)
                // All parts except the last are folders, the last part depends on whether path is a file
                for (idx, part) in path_parts.iter().enumerate() {
                    if !current_path.is_empty() {
                        current_path.push('/');
                    }
                    current_path.push_str(part);
                    
                    // Determine if this part is a file or folder
                    // - If it's the last part and the path is a file, it's a file
                    // - Otherwise, it's a folder
                    let is_file = path.is_file() && idx == path_parts.len() - 1;
                    
                    paths_to_add.push((current_path.clone(), part.to_string(), is_file));
                }
                
                // If the path has no parts (root level file/directory), add it
                if path_parts.is_empty() {
                    let name = path.file_name()
                        .or_else(|| path.file_stem())
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    paths_to_add.push((relative_path_str.clone(), name, path.is_file()));
                }
                
                // Add each path to selected items if not already there
                for (path_str, name, is_file) in paths_to_add {
                    let is_selected = selected_items.iter()
                        .any(|item| item.root == folder.id && item.path == path_str);
                    
                    if !is_selected {
                        let folder_file = crate::structs::FolderFile {
                            id: folder.id.clone(),
                            name: name.clone(),
                            path: path_str.clone(),
                            is_file,
                        };
                        
                        if create_selected_item(&conn, folder_file) {
                            vf_access_log!(ok, "Added {} to selected items: {}", 
                                if is_file { "file" } else { "folder" }, path_str);
                        }
                    }
                }
                
                // Trigger sync via HTTP endpoint for the file itself
                let client = config::create_http_client()
                    .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
                
                let sync_url = "http://127.0.0.1:8383/sync-file";
                let sync_request = serde_json::json!({
                    "root": folder.id,
                    "path": relative_path_str
                });
                
                match client.post(sync_url)
                    .header("Content-Type", "application/json")
                    .json(&sync_request)
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.status().is_success() {
                            vf_access_log!(ok, "Successfully triggered sync for: {:?}", path);
                        } else {
                            vf_access_log!(error, "Sync request returned error status {} for: {:?}", response.status(), path);
                        }
                        return Ok(true);
                    },
                    Err(e) => {
                        vf_access_log!(error, "Failed to trigger sync: {} for: {:?}", e, path);
                        return Ok(false);
                    },
                }
            }
        }
        
        vf_access_log!(warning, "Could not find folder for file: {:?}", path);
        Ok(false)
    }
}

