use std::collections::HashSet;

use rusqlite::{params, Connection, OptionalExtension};

use crate::{out, structs::{FolderFile, FolderFileSaved, SiteSetting}, error::{AppError, AppResult}};

const SCRIPT: &str = "DB";

pub fn database_connect() -> AppResult<Connection> {
    out::ok(SCRIPT, "Connecting to Database SQLite");
    let conn = Connection::open("syncthing-select-sync.db")
        .map_err(|e| {
            out::error(SCRIPT, &format!("Failed to open database: {}", e));
            AppError::Database(e)
        })?;
    // Create all tables that DO NOT exist
    database_create_site_settings(&conn)?;
    database_create_seleted_items(&conn)?;
    Ok(conn)
}

//##### Selected Files/Folders #####\\
pub fn database_create_seleted_items(conn: &Connection) -> AppResult<()> {
    let exists = table_exists(conn, "selected_items")?;
    if !exists {
        // Ensure the table exists
        conn.execute(
            "CREATE TABLE IF NOT EXISTS selected_items (
                id INTEGER PRIMARY KEY,
                folder TEXT NOT NULL,
                path TEXT NOT NULL,
                is_file INTEGER NOT NULL,
                sync_status TEXT NOT NULL DEFAULT 'in_cloud'
            )",
            (),
        ).map_err(|e| {
            out::error(SCRIPT, &format!("Error creating selected_items table: {}", e));
            AppError::Database(e)
        })?;
    } else {
        // Migrate existing table to add sync_status column if it doesn't exist
        let mut stmt = conn.prepare("PRAGMA table_info(selected_items)").map_err(AppError::Database)?;
        let columns: Vec<String> = stmt.query_map([], |row| {
            Ok(row.get::<_, String>(1)?)
        }).map_err(AppError::Database)?
        .collect::<Result<Vec<_>, _>>().map_err(AppError::Database)?;
        
        if !columns.contains(&"sync_status".to_string()) {
            conn.execute(
                "ALTER TABLE selected_items ADD COLUMN sync_status TEXT NOT NULL DEFAULT 'in_cloud'",
                (),
            ).map_err(|e| {
                out::warning(SCRIPT, &format!("Error adding sync_status column (may already exist): {}", e));
                AppError::Database(e)
            })?;
            out::ok(SCRIPT, "Migrated selected_items table to include sync_status");
        }
    }
    Ok(())
}

pub fn create_selected_item(conn: &Connection, ff: FolderFile) -> bool {
    match conn.execute(
        "INSERT INTO selected_items(folder, path, is_file, sync_status) VALUES(?1, ?2, ?3, ?4)",
        params![&ff.id, &ff.path, ff.is_file, "in_cloud"],
    ) {
        Ok(rows_affected) => {
            out::warning(SCRIPT, &format!("Create selected Item Out: {rows_affected} rows affected"));
            rows_affected == 1
        }
        Err(e) => {
            out::error(SCRIPT, &format!("Error creating selected item: {}", e));
            false
        }
    }
}

pub fn delete_selected_item(conn: &Connection, id: i32) -> bool {
    match conn.execute(
        "DELETE FROM selected_items WHERE id = ?1",
        [id],
    ) {
        Ok(rows_deleted) => rows_deleted > 0,
        Err(e) => {
            out::error(SCRIPT, &format!("Error deleting selected item {}: {}", id, e));
            false
        }
    }
}

pub fn update_selected_item(conn: &Connection, id: i32, new_path: &str) -> bool {
    match conn.execute(
        "UPDATE selected_items SET path = ?1 WHERE id = ?2",
        params![new_path, id],
    ) {
        Ok(rows_affected) => {
            out::warning(SCRIPT, &format!("Update selected Item Out: {rows_affected} rows affected"));
            rows_affected == 1
        }
        Err(e) => {
            out::error(SCRIPT, &format!("Error updating selected item {}: {}", id, e));
            false
        }
    }
}

pub fn update_sync_status(conn: &Connection, id: i32, status: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE selected_items SET sync_status = ?1 WHERE id = ?2",
        params![status, id],
    ).map_err(|e| {
        out::error(SCRIPT, &format!("Error updating sync status for item {}: {}", id, e));
        AppError::Database(e)
    })?;
    Ok(())
}

pub fn get_selected_item_by_path(conn: &Connection, root: &str, path: &str) -> AppResult<Option<FolderFileSaved>> {
    let mut stmt = conn.prepare("SELECT id, folder, path, is_file, sync_status FROM selected_items WHERE folder = ?1 AND path = ?2")
        .map_err(AppError::Database)?;

    let item = stmt.query_row([root, path], |row| {
        let status_str: String = row.get(4).unwrap_or_else(|_| "in_cloud".to_string());
        Ok(FolderFileSaved {
            id: row.get(0)?,
            root: row.get(1)?,
            path: row.get(2)?,
            is_file: row.get(3)?,
            sync_status: crate::structs::SyncStatus::from_str(&status_str),
        })
    }).optional()
    .map_err(AppError::Database)?;

    Ok(item)
}


pub fn get_selected_items(conn: &Connection) -> AppResult<Option<Vec<FolderFileSaved>>> {
    let mut stmt = conn.prepare("SELECT id, folder, path, is_file, sync_status FROM selected_items")
        .map_err(AppError::Database)?;

    let items_iter = stmt.query_map([], |row| {
        let status_str: String = row.get(4).unwrap_or_else(|_| "in_cloud".to_string());
        Ok(FolderFileSaved {
            id: row.get(0)?,
            root: row.get(1)?,
            path: row.get(2)?,
            is_file: row.get(3)?,
            sync_status: crate::structs::SyncStatus::from_str(&status_str),
        })
    }).map_err(AppError::Database)?;

    let items: Result<Vec<FolderFileSaved>, _> = items_iter.collect();
    match items {
        Ok(items) if items.is_empty() => Ok(None),
        Ok(items) => Ok(Some(items)),
        Err(e) => {
            out::error(SCRIPT, &format!("Error collecting items: {}", e));
            Err(AppError::Database(e))
        }
    }
}

pub fn get_selected_item(conn: &Connection, id: i32) -> AppResult<Option<FolderFileSaved>> {
    let mut stmt = conn.prepare("SELECT id, folder, path, is_file, sync_status FROM selected_items WHERE id = ?1")
        .map_err(AppError::Database)?;

    let item = stmt.query_row([id], |row| {
        let status_str: String = row.get(4).unwrap_or_else(|_| "in_cloud".to_string());
        Ok(FolderFileSaved {
            id: row.get(0)?,
            root: row.get(1)?,
            path: row.get(2)?,
            is_file: row.get(3)?,
            sync_status: crate::structs::SyncStatus::from_str(&status_str),
        })
    }).optional()
    .map_err(AppError::Database)?;

    Ok(item)
}

//##### Site Settings #####\\
pub fn database_create_site_settings(conn: &Connection) -> AppResult<()> {
    let exists = table_exists(conn, "site_settings")?;
    if !exists {
        // Ensure the table exists
        conn.execute(
            "CREATE TABLE IF NOT EXISTS site_settings (
                id INTEGER PRIMARY KEY,
                key TEXT NOT NULL,
                value TEXT NOT NULL
            )",
            (),
        ).map_err(|e| {
            out::error(SCRIPT, &format!("Error creating site_settings table: {}", e));
            AppError::Database(e)
        })?;

        // Add default settings
        let default_settings = vec![
            ("api-key", "NULL"),
            ("ssss-url", "127.0.0.1:8383"),
            ("ssss-user", "NULL"),
            ("ssss-pass", "NULL"),
            ("st-url", "NULL"),
        ];

        for (key, value) in default_settings {
            create_site_setting(conn, SiteSetting {
                id: 0,
                key: key.to_string(),
                value: value.to_string(),
            })?;
        }
    }
    Ok(())
}

pub fn create_site_setting(conn: &Connection, ss: SiteSetting) -> AppResult<()> {
    conn.execute(
        "INSERT INTO site_settings(key, value) VALUES(?1, ?2)",
        params![&ss.key, &ss.value],
    ).map_err(|e| {
        out::error(SCRIPT, &format!("Error creating site setting {}: {}", ss.key, e));
        AppError::Database(e)
    })?;
    Ok(())
}

pub fn get_site_setting(conn: &Connection, key: &str) -> Option<SiteSetting> {
    let mut stmt = match conn.prepare(
        "SELECT id, key, value
         FROM site_settings WHERE key = ?1",
    ) {
        Ok(s) => s,
        Err(e) => {
            out::error(SCRIPT, &format!("Error preparing query for site setting {}: {}", key, e));
            return None;
        }
    };
    
    match stmt.query_row([key], |row| {
        Ok(SiteSetting {
            id: row.get(0)?,
            key: row.get(1)?,
            value: row.get(2)?,
        })
    }).optional() {
        Ok(Some(setting)) => Some(setting),
        Ok(None) => None,
        Err(e) => {
            out::error(SCRIPT, &format!("Error querying site setting {}: {}", key, e));
            None
        }
    }
}

pub fn set_site_setting(conn: &Connection, key: &str, value: &str) -> bool {
    // Update the Value in the database
    let out = conn.execute(
        "UPDATE site_settings SET value = ?1 WHERE key = ?2",
        [value, key],
    );

    out::info(SCRIPT, &format!("Site Setting Updated: {out:?}"));
    true
}

pub fn update_database(conn: &mut Connection, updated_items: &[FolderFileSaved], original_items: &[FolderFileSaved]) -> AppResult<()> {
    let tx = conn.transaction()
        .map_err(|e| {
            out::error(SCRIPT, &format!("Failed to start transaction: {}", e));
            AppError::Database(e)
        })?;

    let original_ids: HashSet<i32> = original_items.iter().map(|f| f.id).collect();
    let updated_ids: HashSet<i32> = updated_items.iter().map(|f| f.id).collect();

    // Update existing records
    for item in updated_items {
        tx.execute(
            "UPDATE selected_items SET path = ?, sync_status = ? WHERE id = ?",
            params![item.path, item.sync_status.as_str(), item.id],
        ).map_err(|e| {
            out::error(SCRIPT, &format!("Failed to update item {}: {}", item.id, e));
            AppError::Database(e)
        })?;
    }

    // Remove entries that no longer exist in the updated list
    let to_delete: Vec<i32> = original_ids.difference(&updated_ids).cloned().collect();
    for id in to_delete {
        tx.execute("DELETE FROM selected_items WHERE id = ?", params![id])
            .map_err(|e| {
                out::error(SCRIPT, &format!("Failed to delete item {}: {}", id, e));
                AppError::Database(e)
            })?;
    }

    tx.commit().map_err(|e| {
        out::error(SCRIPT, &format!("Failed to commit transaction: {}", e));
        AppError::Database(e)
    })?;

    Ok(())
}

fn table_exists(conn: &Connection, table_name: &str) -> AppResult<bool> {
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1")
        .map_err(AppError::Database)?;
    let exists: i32 = stmt.query_row([table_name], |row| row.get(0))
        .map_err(AppError::Database)?;
    Ok(exists > 0)
}
