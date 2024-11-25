use std::io::Error;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::{database::create_user::{generate_salt, sha256_hash}, guest::Guest, ADMIN};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub password: String,   // Hashed with SHA256
    pub salt: String,
    pub role: u8,
    pub allowed_folders: Vec<FolderId>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUpdate {
    pub id: i32,
    pub username: String,
    pub password: String,   // Hashed with SHA256
    pub salt: String,
    pub role: u8,
    pub allowed_folders: String
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteUser {
    pub username: String
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderId {
    pub folder: String
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderSearch {
    pub id: String,
    pub path: String
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteSetting {
    pub id: i32,
    pub key: String,
    pub value: String
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderFile {
    pub id: String,
    pub name: String,
    pub path: String,
    pub is_file: bool
}

impl User {
    pub fn delete_user(conn: &Connection, username: &str) -> bool {
        match conn.execute(
            "DELETE FROM users WHERE username = ?1",
            [username],
        ) {
            Ok(rows_deleted) => rows_deleted > 0,
            Err(_) => false,
        }
    }

    pub fn get_user(conn: &Connection, username: &str) -> Option<User> {
        let mut stmt = conn.prepare(
            "SELECT id, username, password, salt, role, allowed_folders
             FROM users WHERE username = ?1",
        ).unwrap();
        
        let user = stmt.query_row([username], |row| {
            Ok(User {
                id: row.get(0).unwrap(),
                username: row.get(1).unwrap(),
                password: row.get(2).unwrap(),
                salt: row.get(3).unwrap(),
                role: row.get(4).unwrap(),
                allowed_folders: serde_json::from_str::<Vec<FolderId>>(&row.get::<_, String>(5).unwrap()).unwrap(),
            })
        }).optional().unwrap(); // Use `optional` to handle cases where no user is found

        user
    }

    pub fn update_password(conn: &Connection, username: String, new_password: String) -> bool {
        // Generate a new random salt
        let salt: String = generate_salt();

        // Hash the new password with the new salt
        let hashed_password = sha256_hash(new_password, salt.clone());

        // Update the password and salt in the database
        let out = conn.execute(
            "UPDATE users SET password = ?1, salt = ?2 WHERE username = ?3",
            [hashed_password, salt, username],
        );

        // println!("Out Update Pass: {out:?}");
        true
    }

    pub fn update_user(conn: &Connection, mut updated_user: User, guest: Guest) -> bool {
        if guest.guest_data.role != ADMIN {
            return false;
        }

        // get the user before it is updated
        let user_before_update = User::get_user(&conn, &updated_user.username.clone());

        if user_before_update.is_none() {
            return false;
        }

        let ubu = user_before_update.unwrap();

        updated_user.salt = ubu.salt.clone();

        if ubu.password != updated_user.password {
            // Update salt and SHA256
            let salt = generate_salt();
            updated_user.password = sha256_hash(updated_user.password.clone(), salt.clone());
            updated_user.salt = salt;
        }

        // Execute the update query
        let result = conn.execute(
            "UPDATE users 
             SET password = ?1, salt = ?2, role = ?3, allowed_folders = ?4 
             WHERE username = ?5",
            params![
                updated_user.password,
                updated_user.salt,
                updated_user.role,
                serde_json::to_string(&updated_user.allowed_folders).unwrap(),
                updated_user.username.clone(),
            ],
        );

        match result {
            Ok(rows_updated) => rows_updated > 0,
            Err(_) => false,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Folder {
    pub id: String,
    pub label: String,
    pub filesystemType: String,
    pub path: String,
    // Add other fields based on your JSON structure
}