use rusqlite::{params, Connection, OptionalExtension};

use crate::{database::create_user::{create_user, generate_salt, sha256_hash}, out, structs::{SiteSetting, User}};

const SCRIPT: &str = "DB";

pub fn database_connect() -> Connection {
    out::ok(SCRIPT, "Connecting to Database SQLite");
    let conn = Connection::open("syncthing-select-sync.db").unwrap();
    // Create all tables that DO NOT exist
    database_create_users(&conn);
    database_create_site_settings(&conn);
    conn
}

pub fn database_create_users(conn: &Connection) {
    let out = table_exists(&conn, "users");
    // println!("Check if table exists: {out:?}");
    if !out {
        // Ensure the table exists
        let out = conn.execute(
            "CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY,
                username TEXT NOT NULL UNIQUE,
                password TEXT NOT NULL,
                salt TEXT NOT NULL,
                role INTEGER NOT NULL,
                allowed_folders TEXT NOT NULL
            )",
            (),
        );
        // println!("Out create table: {out:?}");
        match out {
            Ok(0) => {
                // println!("Table exists or was created.");
                let hash = generate_salt();
                let hashed_pass = sha256_hash("ADMIN".to_string(), hash.clone());
                let res = create_user(&conn, User {
                    id: 0,
                    username: "ADMIN".to_string(),
                    password: hashed_pass,
                    salt: hash,
                    role: 255,
                    allowed_folders: vec![],
                });
                if res {
                    out::ok(SCRIPT, "ADMIN user created");
                } else {
                    out::error(SCRIPT, "Failed to Create ADMIN user");
                }
            },
            Err(e) => out::error(SCRIPT, &format!("Error creating table: {}", e)),
            _ => out::warning(SCRIPT, &format!("Unexpected result: {out:?}")),
        }
    }
    
}

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
                // Add ST api setting url
                create_site_setting(&conn, SiteSetting {id: 0, key: "st-url".to_string(), value: "127.0.0.1:8384".to_string()});
                // Add SSSS api url
                create_site_setting(&conn, SiteSetting {id: 0, key: "ssss-url".to_string(), value: "0.0.0.0:8383".to_string()});
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

pub fn database_get_user(conn: &Connection, username: String) -> Option<User> {
    let user = User::get_user(&conn, &username);
    user
}

fn table_exists(conn: &Connection, table_name: &str) -> bool {
    let mut stmt = conn.prepare("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1").unwrap();
    let exists: i32 = stmt.query_row([table_name], |row| row.get(0)).unwrap();
    exists > 0
}
