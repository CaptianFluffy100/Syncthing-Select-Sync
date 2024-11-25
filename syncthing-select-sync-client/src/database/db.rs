use rusqlite::{params, Connection, OptionalExtension};

use crate::{out, structs::{FolderFile, FolderFileSaved, SiteSetting}};

const SCRIPT: &str = "DB";

pub fn database_connect() -> Connection {
    out::ok(SCRIPT, "Connecting to Database SQLite");
    let conn = Connection::open("syncthing-select-sync.db").unwrap();
    // Create all tables that DO NOT exist
    database_create_site_settings(&conn);
    database_create_seleted_items(&conn);
    conn
}

//##### Selected Files/Folders #####\\
pub fn database_create_seleted_items(conn: &Connection) {
    let out = table_exists(&conn, "selected_items");
    // println!("Check if table exists: {out:?}");
    if !out {
        // Ensure the table exists
        let out = conn.execute(
            "CREATE TABLE IF NOT EXISTS selected_items (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL,
                is_file INTEGER NOT NULL
            )",
            (),
        );

        match out {
            Ok(0) => {
                // println!("Table exists or was created.");
            },
            Err(e) => out::error(SCRIPT, &format!("Error creating table: {}", e)),
            _ => out::warning(SCRIPT, &format!("Unexpected result: {out:?}")),
        }
    }
}

pub fn create_selected_item(conn: &Connection, ff: FolderFile) -> bool {
    let out = conn.execute(
        "INSERT INTO selected_items(folder, path, is_file) VALUES(?1, ?2, ?3)",
        params![&ff.id, &ff.path, ff.is_file],
    );

    // println!("Create ss Out: {out:?}");
    out::warning(SCRIPT, &format!("Create selected Item Out: {out:?}"));

    match out {
        Ok(rows_affected) => {
            // If one row was affected, it means the user was inserted
            rows_affected == 1
        }
        Err(_) => false, // If there's an error, return false
    }
}

pub fn delete_selected_item(conn: &Connection, id: i32) -> bool {
    match conn.execute(
        "DELETE FROM selected_items WHERE id = ?1",
        [id],
    ) {
        Ok(rows_deleted) => rows_deleted > 0,
        Err(_) => false,
    }
}

pub fn get_selected_items(conn: &Connection) -> Vec<FolderFileSaved> {
    let mut stmt = conn.prepare("SELECT id, folder, path, is_file FROM selected_items").unwrap();

    let items_iter = stmt.query_map([], |row| {        
        Ok(FolderFileSaved {
            id: row.get(0)?,
            root: row.get(1)?,
            path: row.get(2)?,
            is_file: row.get(3)?
        })
    }).unwrap();

    let items: Vec<FolderFileSaved> = items_iter.collect::<Result<_, _>>().unwrap(); // Collect all users into a Vec<User>

    items
}

pub fn get_selected_item(conn: &Connection, id: i32) -> Option<FolderFileSaved> {
    let mut stmt = conn.prepare("SELECT id, folder, path, is_file FROM selected_items WHERE id = ?1").unwrap();

    let item = stmt.query_row([id], |row| {        
        Ok(FolderFileSaved {
            id: row.get(0)?,
            root: row.get(1)?,
            path: row.get(2)?,
            is_file: row.get(3)?
        })
    }).optional().unwrap();

    item
}

//##### Site Settings #####\\
pub fn database_create_site_settings(conn: &Connection) {
    let out = table_exists(&conn, "site_settings");
    // println!("Check if table exists: {out:?}");
    if !out {
        // Ensure the table exists
        let out = conn.execute(
            "CREATE TABLE IF NOT EXISTS site_settings (
                id INTEGER PRIMARY KEY,
                key TEXT NOT NULL,
                value TEXT NOT NULL
            )",
            (),
        );

        match out {
            Ok(0) => {
                // println!("Table exists or was created.");
                // Add api token setting
                create_site_setting(&conn, SiteSetting {id: 0, key: "api-key".to_string(), value: "NULL".to_string()});
                // Add SSSS api url
                create_site_setting(&conn, SiteSetting {id: 0, key: "ssss-url".to_string(), value: "127.0.0.1:8383".to_string()});
                // Add SSSS api user
                create_site_setting(&conn, SiteSetting {id: 0, key: "ssss-user".to_string(), value: "NULL".to_string()});
                // Add SSSS api pass
                create_site_setting(&conn, SiteSetting {id: 0, key: "ssss-pass".to_string(), value: "NULL".to_string()});
                // Add Syncthing URL
                create_site_setting(&conn, SiteSetting {id: 0, key: "st-url".to_string(), value: "NULL".to_string()});
            },
            Err(e) => out::error(SCRIPT, &format!("Error creating table: {}", e)),
            _ => out::warning(SCRIPT, &format!("Unexpected result: {out:?}")),
        }
    }
}

pub fn create_site_setting(conn: &Connection, ss: SiteSetting) -> bool {
    let out = conn.execute(
        "INSERT INTO site_settings(key, value) VALUES(?1, ?2)",
        params![&ss.key, &ss.value],
    );

    // println!("Create ss Out: {out:?}");
    out::warning(SCRIPT, &format!("Create ss Out: {out:?}"));

    match out {
        Ok(rows_affected) => {
            // If one row was affected, it means the user was inserted
            rows_affected == 1
        }
        Err(_) => false, // If there's an error, return false
    }
}

pub fn get_site_setting(conn: &Connection, key: &str) -> Option<SiteSetting> {
    let mut stmt = conn.prepare(
        "SELECT id, key, value
         FROM site_settings WHERE key = ?1",
    ).unwrap();
    
    let user = stmt.query_row([key], |row| {
        Ok(SiteSetting {
            id: row.get(0).unwrap(),
            key: row.get(1).unwrap(),
            value: row.get(2).unwrap(),
        })
    }).optional().unwrap(); // Use `optional` to handle cases where no user is found

    user
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

fn table_exists(conn: &Connection, table_name: &str) -> bool {
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1").unwrap();
    let exists: i32 = stmt.query_row([table_name], |row| row.get(0)).unwrap();
    exists > 0
}
