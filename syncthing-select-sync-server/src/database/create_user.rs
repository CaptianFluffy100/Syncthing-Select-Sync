use rand::{distributions::Alphanumeric, Rng};
use rusqlite::{params, Connection};
use sha2::{Sha256, Digest};

use crate::structs::{FolderId, User};

pub fn create_user(conn: &Connection, user: User) -> bool {
    let out = conn.execute(
        "INSERT INTO users(username, password, salt, role, allowed_folders) VALUES(?1, ?2, ?3, ?4, ?5)",
        params![&user.username, &user.password, &user.salt, user.role, &serde_json::to_string(&user.allowed_folders).unwrap()],
    );

    // println!("Create user Out: {out:?}");

    match out {
        Ok(rows_affected) => {
            // If one row was affected, it means the user was inserted
            rows_affected == 1
        }
        Err(_) => false, // If there's an error, return false
    }
}

pub fn get_all_users(conn: &Connection) -> Vec<User> {
    let mut stmt = conn.prepare("SELECT id, username, password, salt, role, allowed_folders FROM users").unwrap();

    let user_iter = stmt.query_map([], |row| {
        let allowed_folders: String = row.get(5)?; // Assuming allowed_folders is stored as a JSON string in the DB
        // println!("Allowed Folders: {allowed_folders:?}");
        let allowed_folders: Vec<FolderId> = serde_json::from_str(&allowed_folders).unwrap_or_default(); // Deserialize
        
        Ok(User {
            id: row.get(0)?,
            username: row.get(1)?,
            password: row.get(2)?,
            salt: row.get(3)?,
            role: row.get(4)?,
            allowed_folders: allowed_folders,
        })
    }).unwrap();

    let users: Vec<User> = user_iter.collect::<Result<_, _>>().unwrap(); // Collect all users into a Vec<User>

    users
}

pub fn generate_salt() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect()
}

pub fn sha256_hash(pass: String, salt: String) -> String {
    let mut hasher = Sha256::new(); // Create a SHA256 hasher
    hasher.update(salt.as_bytes()); // Add the salt to the hasher
    hasher.update(pass.as_bytes()); // Add the password to the hasher

    let result = hasher.finalize(); // Finalize the hash
    base64::encode(result) // Return the hash as a Base64 string
}