use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use dirs::home_dir;
use chrono::{NaiveTime, Local, Datelike};
use crate::{out, database::db::{database_connect, get_site_setting}, structs::Folder};

const SCRIPT: &str = "FILE_INDEXER";

#[derive(Debug, Clone)]
pub struct IndexedFile {
    pub id: i32,
    pub folder_id: String,
    pub path: String,
    pub name: String,
    pub is_file: bool,
    pub parent_path: String,
    pub last_modified: i64,
}

pub struct FileIndexer {
    watchers: Arc<RwLock<HashMap<String, notify::RecommendedWatcher>>>,
    running: Arc<RwLock<bool>>,
}

impl FileIndexer {
    pub fn new() -> Self {
        Self {
            watchers: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Initialize the file indexer - create database table and start services
    pub async fn initialize(&self) -> Result<(), String> {
        let conn = database_connect();
        Self::create_indexed_files_table(&conn)
            .map_err(|e| format!("Failed to create table: {}", e))?;
        
        // Start file system watchers
        self.start_watchers().await
            .map_err(|e| format!("Failed to start watchers: {}", e))?;
        
        // Start scheduled indexing
        self.start_scheduled_indexing().await;
        
        Ok(())
    }

    /// Create the indexed_files table in the database
    fn create_indexed_files_table(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
        let exists = Self::table_exists(conn, "indexed_files")?;
        
        if !exists {
            conn.execute(
                "CREATE TABLE IF NOT EXISTS indexed_files (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    folder_id TEXT NOT NULL,
                    path TEXT NOT NULL,
                    name TEXT NOT NULL,
                    is_file INTEGER NOT NULL,
                    parent_path TEXT NOT NULL,
                    last_modified INTEGER NOT NULL,
                    UNIQUE(folder_id, path)
                )",
                (),
            )?;
            
            // Create index for faster lookups
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_folder_path ON indexed_files(folder_id, parent_path)",
                (),
            )?;
            
            out::ok(SCRIPT, "Created indexed_files table");
        }
        
        Ok(())
    }

    fn table_exists(conn: &rusqlite::Connection, table_name: &str) -> Result<bool, rusqlite::Error> {
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1")?;
        let exists: i32 = stmt.query_row([table_name], |row| row.get(0))?;
        Ok(exists > 0)
    }

    /// Index all files in a folder
    pub async fn index_folder(&self, folder_id: &str, folder_path: &str) -> Result<usize, String> {
        let home = home_dir().ok_or_else(|| "Failed to get home directory".to_string())?;
        let home_str = home.to_string_lossy().to_string();
        let full_path = folder_path.replace("~", &home_str);
        let path = Path::new(&full_path);

        if !path.exists() {
            out::warning(SCRIPT, &format!("Folder path does not exist: {}", full_path));
            return Ok(0);
        }

        out::ok(SCRIPT, &format!("Indexing folder: {} ({})", folder_id, full_path));
        
        let mut conn = database_connect();
        
        // Start a transaction for better performance
        let tx = conn.transaction()
            .map_err(|e| format!("Failed to start transaction: {}", e))?;
        
        // Clear existing entries for this folder
        tx.execute("DELETE FROM indexed_files WHERE folder_id = ?1", [folder_id])
            .map_err(|e| format!("Failed to delete existing entries: {}", e))?;
        
        // Recursively index files
        let count = Self::index_directory_recursive(&tx, folder_id, path, &full_path)
            .map_err(|e| format!("Failed to index directory: {}", e))?;
        
        tx.commit()
            .map_err(|e| format!("Failed to commit transaction: {}", e))?;
        
        out::ok(SCRIPT, &format!("Indexed {} items in folder {}", count, folder_id));
        Ok(count)
    }

    fn index_directory_recursive(
        tx: &rusqlite::Transaction,
        folder_id: &str,
        dir: &Path,
        base_path: &str,
    ) -> Result<usize, rusqlite::Error> {
        let mut count = 0;
        
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                out::warning(SCRIPT, &format!("Failed to read directory {:?}: {}", dir, e));
                return Ok(0);
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    out::warning(SCRIPT, &format!("Failed to read directory entry: {}", e));
                    continue;
                }
            };

            let path = entry.path();
            let is_file = path.is_file();
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
                .to_string();

            // Calculate relative path
            let relative_path = path.to_string_lossy().replace(base_path, "").trim_start_matches('/').to_string();
            let parent_path = if let Some(parent) = path.parent() {
                parent.to_string_lossy().replace(base_path, "").trim_start_matches('/').to_string()
            } else {
                String::new()
            };

            // Get last modified time
            let last_modified = path.metadata()
                .and_then(|m| m.modified())
                .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64)
                .unwrap_or(0);

            // Insert into database
            match tx.execute(
                "INSERT INTO indexed_files (folder_id, path, name, is_file, parent_path, last_modified) 
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(folder_id, path) DO UPDATE SET 
                    name = ?3, is_file = ?4, parent_path = ?5, last_modified = ?6",
                rusqlite::params![folder_id, relative_path, name, is_file as i32, parent_path, last_modified],
            ) {
                Ok(_) => {
                    count += 1;
                }
                Err(e) => {
                    out::error(SCRIPT, &format!("Failed to insert indexed file: {}", e));
                }
            }

            // Recursively index subdirectories
            if !is_file {
                if let Ok(sub_count) = Self::index_directory_recursive(tx, folder_id, &path, base_path) {
                    count += sub_count;
                }
            }
        }

        Ok(count)
    }

    /// Get indexed files for a folder and path
    pub fn get_indexed_files(folder_id: &str, parent_path: &str) -> Result<Vec<crate::structs::FolderFile>, rusqlite::Error> {
        let conn = database_connect();
        let mut stmt = conn.prepare(
            "SELECT path, name, is_file FROM indexed_files 
             WHERE folder_id = ?1 AND parent_path = ?2 
             ORDER BY is_file, name"
        )?;

        let files: Result<Vec<_>, _> = stmt.query_map([folder_id, parent_path], |row| {
            Ok(crate::structs::FolderFile {
                id: folder_id.to_string(),
                name: row.get(1)?,
                path: row.get(0)?,
                is_file: row.get(2)?,
            })
        })?.collect();

        files.map_err(|e| e.into())
    }

    /// Start file system watchers for all folders
    async fn start_watchers(&self) -> Result<(), String> {
        let folders = Self::get_folders().await?;
        let home = home_dir().ok_or_else(|| "Failed to get home directory".to_string())?;
        let home_str = home.to_string_lossy().to_string();

        let mut watchers = self.watchers.write().await;

        for folder in folders {
            let folder_path = folder.path.replace("~", &home_str);
            let path = PathBuf::from(&folder_path);

            if !path.exists() {
                continue;
            }

            let folder_id = folder.id.clone();
            let mut watcher = notify::recommended_watcher(move |result: Result<Event, notify::Error>| {
                match result {
                    Ok(event) => {
                        if let EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) = event.kind {
                            out::debug(SCRIPT, &format!("File system event detected for folder {}: {:?}", folder_id, event));
                            // Trigger incremental update for this folder
                            // This will be handled by the scheduled indexing or a separate update task
                        }
                    }
                    Err(e) => {
                        out::error(SCRIPT, &format!("File watcher error: {}", e));
                    }
                }
            })
            .map_err(|e| format!("Failed to create watcher: {}", e))?;

            watcher.watch(&path, RecursiveMode::Recursive)
                .map_err(|e| format!("Failed to watch path: {}", e))?;
            watchers.insert(folder.id.clone(), watcher);
            out::ok(SCRIPT, &format!("Started file watcher for folder: {} ({})", folder.id, folder_path));
        }

        Ok(())
    }

    /// Get folders from Syncthing API
    async fn get_folders() -> Result<Vec<crate::structs::Folder>, String> {
        use reqwest::Client;
        use std::time::Duration;

        let client = Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let conn = database_connect();
        let ss = get_site_setting(&conn, "st-url")
            .ok_or_else(|| "st-url not found in settings".to_string())?;
        let st_api_key = get_site_setting(&conn, "api-key")
            .ok_or_else(|| "api-key not found in settings".to_string())?;

        let response = client
            .get(format!("https://{}/rest/config/folders", ss.value))
            .header("X-API-Key", st_api_key.value)
            .send()
            .await
            .map_err(|e| format!("Failed to send request: {}", e))?;

        let folders: Vec<crate::structs::Folder> = response.json().await
            .map_err(|e| format!("Failed to parse response: {}", e))?;
        Ok(folders)
    }

    /// Start scheduled indexing based on configuration
    async fn start_scheduled_indexing(&self) {
        let running = Arc::clone(&self.running);
        let indexer = Arc::new(self.clone());

        tokio::spawn(async move {
            let mut running_lock = running.write().await;
            *running_lock = true;
            drop(running_lock);

            loop {
                let is_running = *running.read().await;
                if !is_running {
                    break;
                }

                // Check if it's time to index
                if Self::should_index_now().await {
                    out::ok(SCRIPT, "Starting scheduled indexing");
                    
                    match Self::get_folders().await {
                        Ok(folders) => {
                            let home = home_dir().unwrap().to_string_lossy().to_string();
                            for folder in folders {
                                let folder_path = folder.path.replace("~", &home);
                                if let Err(e) = indexer.index_folder(&folder.id, &folder_path).await {
                                    out::error(SCRIPT, &format!("Failed to index folder {}: {}", folder.id, e));
                                }
                            }
                        }
                        Err(e) => {
                            out::error(SCRIPT, &format!("Failed to get folders for indexing: {}", e));
                        }
                    }
                }

                // Check every minute
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            }
        });
    }

    /// Check if indexing should run now based on schedule configuration
    async fn should_index_now() -> bool {
        let conn = database_connect();
        
        // Get indexing schedule from settings
        let schedule_time = get_site_setting(&conn, "index-schedule-time")
            .map(|s| s.value)
            .unwrap_or_else(|| "00:00".to_string()); // Default to midnight
        
        let schedule_days = get_site_setting(&conn, "index-schedule-days")
            .map(|s| s.value)
            .unwrap_or_else(|| "0,1,2,3,4,5,6".to_string()); // Default to all days (0=Sunday, 6=Saturday)

        // Parse time
        let schedule_time = match NaiveTime::parse_from_str(&schedule_time, "%H:%M") {
            Ok(t) => t,
            Err(_) => {
                out::warning(SCRIPT, &format!("Invalid schedule time format: {}", schedule_time));
                return false;
            }
        };

        // Parse days
        let schedule_days: Vec<u32> = schedule_days
            .split(',')
            .filter_map(|d| d.trim().parse().ok())
            .collect();

        if schedule_days.is_empty() {
            return false;
        }

        let now = Local::now();
        let current_time = now.time();
        let current_weekday = Datelike::weekday(&now).num_days_from_sunday();

        // Check if current day is in schedule
        if !schedule_days.contains(&current_weekday) {
            return false;
        }

        // Check if current time matches schedule time (within 1 minute window)
        let time_diff = (current_time - schedule_time).num_seconds().abs();
        time_diff < 60
    }

    /// Manually trigger indexing for all folders
    pub async fn trigger_indexing(&self) -> Result<(), String> {
        let folders = Self::get_folders().await?;
        let home = home_dir().ok_or_else(|| "Failed to get home directory".to_string())?;
        let home_str = home.to_string_lossy().to_string();

        for folder in folders {
            let folder_path = folder.path.replace("~", &home_str);
            self.index_folder(&folder.id, &folder_path).await
                .map_err(|e| format!("Failed to index folder {}: {}", folder.id, e))?;
        }

        Ok(())
    }
}

impl Clone for FileIndexer {
    fn clone(&self) -> Self {
        Self {
            watchers: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::clone(&self.running),
        }
    }
}

