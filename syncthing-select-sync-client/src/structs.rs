use std::io::Error;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderFileSaved {
    pub id: i32,
    pub root: String,
    pub path: String,
    pub is_file: bool
}

#[derive(Deserialize, Debug, Clone)]
pub struct Folder {
    pub id: String,
    pub label: String,
    pub filesystemType: String,
    pub path: String,
    // Add other fields based on your JSON structure
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LoginUser {
    pub username: String,
    pub password: String
}