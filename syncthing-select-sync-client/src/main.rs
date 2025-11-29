use std::{collections::HashMap, fs, path::Path, process::Command, sync::Arc};

use axum::{error_handling::HandleErrorLayer, extract::State, response::{Html, IntoResponse}, routing::{delete, get, post}, Json, Router};
use database::db::{create_selected_item, database_connect, delete_selected_item, get_selected_item, get_selected_item_by_path, get_selected_items, get_site_setting, set_site_setting, update_database, update_selected_item, update_sync_status};
use dirs::home_dir;
use http::StatusCode;
use reqwest::Client;
use serde::Deserialize;
use structs::{Folder, FolderFile, FolderFileSaved, FolderSearch, LoginUser};
use tower::{BoxError, ServiceBuilder};
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer};

mod out;
mod database;
mod structs;
mod guest;
mod error;
mod config;
mod virtual_files;
mod syncthing_config;

const SCRIPT: &str = "MAIN";
#[tokio::main]
async fn main() {
    out::ok(SCRIPT, "Starting Syncthing Select Sync Client (SSSC)");

    let session_store = MemoryStore::default();
    // let session_layer = SessionuserOnInactivity(Duration::seconds(60*60*24*30)));
    let session_service = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(|_: BoxError| async {
            StatusCode::BAD_REQUEST
        }))
        .layer(
            SessionManagerLayer::new(session_store)
                .with_secure(false)
                .with_expiry(Expiry::OnSessionEnd),
        );

    // Create a shared Reqwest client
    let client = config::create_shared_client()
        .unwrap_or_else(|e| {
            out::error(SCRIPT, &format!("Failed to create HTTP client: {}", e));
            std::process::exit(1);
        });

    // Auto-configure Syncthing API key if available
    let syncthing_config = syncthing_config::SyncthingConfig::new();
    syncthing_config.auto_configure_api_key();

    // Consolidate paths
    match database_connect() {
        Ok(mut db) => {
            match get_selected_items(&db) {
                Ok(Some(mut saved_files)) => {
                    let original_files = saved_files.clone();

                    // Consolidate paths
                    consolidate_paths(&mut saved_files);

                    // Update the database with the new paths and remove unnecessary entries
                    match update_database(&mut db, &saved_files, &original_files) {
                        Ok(_) => out::ok(SCRIPT, "Consolidation Passed"),
                        Err(e) => out::warning(SCRIPT, &format!("Consolidation Failed: {}", e)),
                    }
                }
                Ok(None) => {
                    // No items to consolidate, that's fine
                }
                Err(e) => out::error(SCRIPT, &format!("Failed to get selected items: {}", e)),
            }
        }
        Err(e) => {
            out::error(SCRIPT, &format!("Failed to initialize database: {}", e));
            std::process::exit(1);
        }
    }
    
    // Start virtual file manager
    let virtual_file_manager = virtual_files::VirtualFileManager::new();
    virtual_file_manager.start().await;
    out::ok(SCRIPT, "Virtual file manager started");

    // Start the axum end
    // build our application with a route
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/", get(home_page))
        .route("/items", get(items_page))
        .route("/login", get(login))
        .route("/update-site-settings", post(update_site_settings))
        .route("/add-selected-item", post(add_selected_item))
        .route("/del-selected-item", delete(del_selected_item))
        .route("/ssss/get-items", post(get_ssss_items))
        .route("/select-sync-items", get(get_all_selected_items))
        .route("/sync-file", post(sync_file))
        .route("/free-up-space", post(free_up_space))
        .route("/check-file-status", post(check_file_status))
        .layer(session_service)
        .with_state(client);
    // run our app with hyper, listening globally on port 8383
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8383").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn home_page(State(client): State<Arc<Client>>) -> impl IntoResponse {
    // Get the html page
    let page = match fs::read_to_string("./html/home.html") {
        Ok(p) => p,
        Err(e) => {
            out::error(SCRIPT, &format!("Failed to read home.html: {}", e));
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load page").into_response();
        }
    };

    // Check if user is logged in
    let logged_in = match database_connect() {
        Ok(conn) => {
            match get_site_setting(&conn, "ssss-url") {
                Some(ss) => {
    out::debug(SCRIPT, &format!("SiteSettings: {ss:?}"));
    match client.get(&format!("{}/check-status", ss.value)).send().await {
        Ok(response) => {
            if response.status() == StatusCode::OK {
                                "Logged In".to_string()
            } else if response.status() == StatusCode::IM_A_TEAPOT {
                                "Not Logged In".to_string()
            } else {
                                "Server Error".to_string()
                            }
                        },
                        Err(err) => {
                            out::error(SCRIPT, &format!("Request failed: {:?}", err));
                            "(Not Logged in)".to_string()
                        },
                    }
                },
                None => {
                    out::error(SCRIPT, "ssss-url not found in settings");
                    "(Not Logged in)".to_string()
                }
            }
        },
        Err(e) => {
            out::error(SCRIPT, &format!("Database connection failed: {}", e));
            "(Not Logged in)".to_string()
        }
    };

    let page = page.replace("<LogInStatus/>", &logged_in);
    let page = page.replace("<Folders/>", &create_html_folders().await);

    Html(page).into_response()
}

async fn items_page(State(_client): State<Arc<Client>>) -> impl IntoResponse {
    // Get the html page
    match fs::read_to_string("./html/items.html") {
        Ok(page) => Html(page).into_response(),
        Err(e) => {
            out::error(SCRIPT, &format!("Failed to read items.html: {}", e));
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load page").into_response()
        }
    }
}

async fn login(State(client): State<Arc<Client>>) -> StatusCode {
    // Get the database connection and site settings
    let conn = match database_connect() {
        Ok(c) => c,
        Err(e) => {
            out::error(SCRIPT, &format!("Database connection failed: {}", e));
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let ss = match get_site_setting(&conn, "ssss-url") {
        Some(s) => s,
        None => {
            out::error(SCRIPT, "ssss-url not found in settings");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };
    out::debug(SCRIPT, &format!("SiteSettings: {ss:?}"));

    let ss_user = match get_site_setting(&conn, "ssss-user") {
        Some(s) => s,
        None => {
            out::error(SCRIPT, "ssss-user not found in settings");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let ss_pass = match get_site_setting(&conn, "ssss-pass") {
        Some(s) => s,
        None => {
            out::error(SCRIPT, "ssss-pass not found in settings");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    match client.post(&format!("{}/api/login", ss.value))
        .header("Content-Type", "application/json")
        .json(&LoginUser{username: ss_user.value, password: ss_pass.value})
        .send()
        .await
    {
        Ok(response) => {
            out::debug(SCRIPT, &format!("Login response: {:?}", response.status()));
            response.status()
        },
        Err(e) => {
            out::error(SCRIPT, &format!("Login request failed: {}", e));
            StatusCode::SERVICE_UNAVAILABLE
        },
    }
}

//##### SET SITE SETTINGS #####\\
#[derive(Deserialize, Debug)]
struct SetSiteSettings {
    api_token: String,
    ssss_url: String,
    ssss_pass: String,
    ssss_user: String,
    st_url: String,
}

async fn update_site_settings(
    Json(payload): Json<SetSiteSettings>,
) -> StatusCode {
    let conn = match database_connect() {
        Ok(c) => c,
        Err(e) => {
            out::error(SCRIPT, &format!("Database connection failed: {}", e));
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    // Update each field in site settings
    if !payload.api_token.is_empty() {
        let _ = set_site_setting(&conn, "api-key", &payload.api_token);
    }

    if !payload.ssss_url.is_empty() {
        let _ = set_site_setting(&conn, "ssss-url", &payload.ssss_url);
    }

    if !payload.ssss_user.is_empty() {
        let _ = set_site_setting(&conn, "ssss-user", &payload.ssss_user);
    }

    if !payload.ssss_pass.is_empty() {
        let _ = set_site_setting(&conn, "ssss-pass", &payload.ssss_pass);
    }

    if !payload.st_url.is_empty() {
        let _ = set_site_setting(&conn, "st-url", &payload.st_url);
    }

    StatusCode::OK
}

//##### SELECTED ITEMS #####\\
#[derive(Deserialize, Debug)]
struct DeleteSelected {
    id: i32,
    root: String
}

async fn add_selected_item(
    Json(payload): Json<FolderFile>,
) -> StatusCode {
    println!("Payload: {payload:?}");
    let conn = match database_connect() {
        Ok(c) => c,
        Err(e) => {
            out::error(SCRIPT, &format!("Database connection failed: {}", e));
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let saved_files = match get_selected_items(&conn) {
        Ok(Some(files)) => files,
        Ok(None) => Vec::new(),
        Err(e) => {
            out::error(SCRIPT, &format!("Failed to get selected items: {}", e));
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    // if saved_files.is_none() {
    //     // return StatusCode::INTERNAL_SERVER_ERROR;
    // }
    // let saved_files = saved_files.unwrap_or_default();

    let mut is_child = false;
    let mut parent_id = None;

    // Check if the new item is a child or parent
    for file in &saved_files {
        if file.root == payload.id {
            if is_child_path(&file.path, &payload.path) {
                // If it's a child, ignore it
                is_child = true;
                break;
            } else if is_child_path(&payload.path, &file.path) {
                // If it's a parent, store the ID to update later
                parent_id = Some(file.id);
            }
        }
    }

    println!("is_child: {is_child:?}");

    if is_child {
        // Ignore adding this item
        return StatusCode::OK;
    }

    let success = if let Some(parent_id) = parent_id {
        // Update the existing entry to use the parent path
        update_selected_item(&conn, parent_id, &payload.path)
    } else {
        // If neither, create a new entry
        create_selected_item(&conn, payload.clone())
    };

    // Update .stignore file
    update_stignore(payload.id, false).await;

    if success {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

async fn del_selected_item(
    Json(payload): Json<DeleteSelected>,
) -> StatusCode {
    println!("Payload: {payload:?}");
    let conn = match database_connect() {
        Ok(c) => c,
        Err(e) => {
            out::error(SCRIPT, &format!("Database connection failed: {}", e));
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };
    
    let item = match get_selected_item(&conn, payload.id) {
        Ok(Some(item)) => item,
        Ok(None) => return StatusCode::NOT_FOUND,
        Err(e) => {
            out::error(SCRIPT, &format!("Failed to get selected item: {}", e));
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };
    
    let res = delete_selected_item(&conn, payload.id);

    // Update .stignore file
    update_stignore(payload.root.clone(), true).await;

    delete_folder(item, payload.root).await;

    if res {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

async fn delete_folder(ffs: FolderFileSaved, id: String) {
    let folders = get_folders().await.unwrap_or_default();
    for folder in folders {
        if folder.id == id {
            let home = home_dir().unwrap().to_string_lossy().to_string();
            let r = format!("{}/{}", folder.path.replace("~", &home), ffs.path);
            let path = Path::new(&r);
            out::highlight(SCRIPT, &format!("Delete Path: {path:?}"));
            if ffs.is_file {
                // Delete file
                let out = fs::remove_file(path);
                out::highlight(SCRIPT, &format!("Delete File Out: {out:?}"));
            } else {
                // Delete folder
                let out = fs::remove_dir_all(path);
                out::highlight(SCRIPT, &format!("Delete Folder Out: {out:?}"));
            }
        }
    }
}

async fn get_ssss_items(
    State(client): State<Arc<Client>>,
    Json(payload): Json<FolderSearch>,
) -> impl IntoResponse {
    // println!("Payload: {payload:?}");
    out::bright(SCRIPT, &format!("Payload: {payload:?}"));
    if payload.id.is_empty() {
        // Get the folders and sent them
        let folders = get_folders().await;
        if folders.is_none() {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        let folders = folders.unwrap();
        let mut ff = vec![];
        for folder in folders {
            ff.push(FolderFile{id: folder.id, name: folder.label, path: "".to_string(), is_file: false});
        }
        return Json(ff).into_response();
    }

    Json(get_items_in_ssss_folder(client, payload).await).into_response()
}

async fn get_items_in_ssss_folder(client: Arc<Client>, fs: FolderSearch) -> Vec<FolderFile> {
    let conn = match database_connect() {
        Ok(c) => c,
        Err(e) => {
            out::error(SCRIPT, &format!("Database connection failed: {}", e));
            return vec![];
        }
    };
    
    let ss = match get_site_setting(&conn, "ssss-url") {
        Some(s) => s,
        None => {
            out::error(SCRIPT, "ssss-url not found in settings");
            return vec![];
        }
    };
    out::bright(SCRIPT, &format!("ssss-url: {ss:?}"));
    // Get the data
    match client.post(&format!("{}/api/ssss/get-items", ss.value)).json(&fs).send().await {
        Ok(response) => {
            // Get respones status
            let mut res = response.json::<Vec<FolderFile>>().await.unwrap_or_default();
            // Sort the result: folders first, files last
            res.sort_by(|a, b| {
                // If both are folders or both are files, sort by name
                if a.is_file == b.is_file {
                    a.name.cmp(&b.name)
                } else {
                    // Folders (is_file = false) come first, so compare `is_file`
                    a.is_file.cmp(&b.is_file)
                }
            });
            return res;
        },
        Err(err) => out::error(SCRIPT, &format!("Request failed: {:?}", err)),
    };
    return vec![];
}

async fn get_all_selected_items() -> impl IntoResponse {
    let conn = match database_connect() {
        Ok(c) => c,
        Err(e) => {
            out::error(SCRIPT, &format!("Database connection failed: {}", e));
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    match get_selected_items(&conn) {
        Ok(Some(items)) => Json(items).into_response(),
        Ok(None) => Json(Vec::<FolderFileSaved>::new()).into_response(),
        Err(e) => {
            out::error(SCRIPT, &format!("Failed to get selected items: {}", e));
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

#[derive(Deserialize, Debug)]
struct SyncFileRequest {
    root: String,
    path: String,
}

async fn sync_file(
    State(_client): State<Arc<Client>>,
    Json(payload): Json<SyncFileRequest>,
) -> impl IntoResponse {
    let conn = match database_connect() {
        Ok(c) => c,
        Err(e) => {
            out::error(SCRIPT, &format!("Database connection failed: {}", e));
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    // Get the item from database
    let item = match get_selected_item_by_path(&conn, &payload.root, &payload.path) {
        Ok(Some(item)) => item,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, "Item not found in sync list").into_response();
        }
        Err(e) => {
            out::error(SCRIPT, &format!("Database error: {}", e));
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    // Update status to syncing
    if let Err(e) = update_sync_status(&conn, item.id, "syncing") {
        out::error(SCRIPT, &format!("Failed to update sync status: {}", e));
    }

    // Get folder path
    let folders = match get_folders().await {
        Some(f) => f,
        None => {
            let _ = update_sync_status(&conn, item.id, "in_cloud");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get folders").into_response();
        }
    };

    let folder = match folders.iter().find(|f| f.id == payload.root) {
        Some(f) => f,
        None => {
            let _ = update_sync_status(&conn, item.id, "in_cloud");
            return (StatusCode::NOT_FOUND, "Folder not found").into_response();
        }
    };

    let home = match home_dir() {
        Some(h) => h.to_string_lossy().to_string(),
        None => {
            let _ = update_sync_status(&conn, item.id, "in_cloud");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get home directory").into_response();
        }
    };

    let full_path = format!("{}/{}", folder.path.replace("~", &home), payload.path);
    let file_path = Path::new(&full_path);

    // Check if file already exists
    if file_path.exists() {
        // Check if it's a virtual file
        if virtual_files::VirtualFileManager::is_virtual_file(file_path) {
            // It's a virtual file, we need to actually sync it
            out::ok(SCRIPT, &format!("Virtual file accessed, syncing: {}", full_path));
            // Continue to sync the file below
        } else {
            // File already synced
            if let Err(e) = update_sync_status(&conn, item.id, "synced") {
                out::error(SCRIPT, &format!("Failed to update sync status: {}", e));
            }
            return (StatusCode::OK, "File already synced").into_response();
        }
    }

    // Ensure parent directory exists
    if let Some(parent) = file_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            out::error(SCRIPT, &format!("Failed to create parent directory: {}", e));
            let _ = update_sync_status(&conn, item.id, "in_cloud");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create directory").into_response();
        }
    }

    // For now, we'll just create an empty file or placeholder
    // In a real implementation, you'd download from the server
    // For files, create empty file; for folders, create directory
    let result = if item.is_file {
        fs::File::create(file_path).map(|_| ())
    } else {
        fs::create_dir_all(file_path).map(|_| ())
    };

    match result {
        Ok(_) => {
            // Remove virtual file marker if it exists
            if let Err(e) = virtual_files::VirtualFileManager::remove_virtual_marker(file_path).await {
                out::warning(SCRIPT, &format!("Failed to remove virtual marker: {}", e));
            }
            
            // Update .stignore to include this file
            update_stignore(payload.root.clone(), false).await;
            
            // Update status to synced
            if let Err(e) = update_sync_status(&conn, item.id, "synced") {
                out::error(SCRIPT, &format!("Failed to update sync status: {}", e));
            }
            
            out::ok(SCRIPT, &format!("File synced: {}", full_path));
            (StatusCode::OK, "File synced successfully").into_response()
        }
        Err(e) => {
            out::error(SCRIPT, &format!("Failed to sync file: {}", e));
            let _ = update_sync_status(&conn, item.id, "in_cloud");
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to sync file").into_response()
        }
    }
}

#[derive(Deserialize, Debug)]
struct FreeSpaceRequest {
    root: String,
    path: String,
}

async fn free_up_space(
    Json(payload): Json<FreeSpaceRequest>,
) -> impl IntoResponse {
    let conn = match database_connect() {
        Ok(c) => c,
        Err(e) => {
            out::error(SCRIPT, &format!("Database connection failed: {}", e));
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    // Get the item from database
    let item = match get_selected_item_by_path(&conn, &payload.root, &payload.path) {
        Ok(Some(item)) => item,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, "Item not found in sync list").into_response();
        }
        Err(e) => {
            out::error(SCRIPT, &format!("Database error: {}", e));
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    // Get folder path
    let folders = match get_folders().await {
        Some(f) => f,
        None => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get folders").into_response();
        }
    };

    let folder = match folders.iter().find(|f| f.id == payload.root) {
        Some(f) => f,
        None => {
            return (StatusCode::NOT_FOUND, "Folder not found").into_response();
        }
    };

    let home = match home_dir() {
        Some(h) => h.to_string_lossy().to_string(),
        None => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get home directory").into_response();
        }
    };

    let full_path = format!("{}/{}", folder.path.replace("~", &home), payload.path);
    let file_path = Path::new(&full_path);

    // Delete the file/folder
    let result = if item.is_file {
        fs::remove_file(file_path)
    } else {
        fs::remove_dir_all(file_path)
    };

    match result {
        Ok(_) => {
            // Update status back to in_cloud
            if let Err(e) = update_sync_status(&conn, item.id, "in_cloud") {
                out::error(SCRIPT, &format!("Failed to update sync status: {}", e));
            }
            
            out::ok(SCRIPT, &format!("Freed up space: {}", full_path));
            (StatusCode::OK, "Space freed successfully").into_response()
        }
        Err(e) => {
            out::error(SCRIPT, &format!("Failed to free up space: {}", e));
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to free up space").into_response()
        }
    }
}

async fn check_file_status(
    Json(payload): Json<SyncFileRequest>,
) -> impl IntoResponse {
    let conn = match database_connect() {
        Ok(c) => c,
        Err(e) => {
            out::error(SCRIPT, &format!("Database connection failed: {}", e));
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    // Get the item from database
    let item = match get_selected_item_by_path(&conn, &payload.root, &payload.path) {
        Ok(Some(item)) => item,
        Ok(None) => {
            return Json(serde_json::json!({
                "exists": false,
                "in_sync_list": false,
                "status": "not_in_list"
            })).into_response();
        }
        Err(e) => {
            out::error(SCRIPT, &format!("Database error: {}", e));
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    // Check if file exists locally
    let folders = match get_folders().await {
        Some(f) => f,
        None => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get folders").into_response();
        }
    };

    let folder = match folders.iter().find(|f| f.id == payload.root) {
        Some(f) => f,
        None => {
            return (StatusCode::NOT_FOUND, "Folder not found").into_response();
        }
    };

    let home = match home_dir() {
        Some(h) => h.to_string_lossy().to_string(),
        None => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get home directory").into_response();
        }
    };

    let full_path = format!("{}/{}", folder.path.replace("~", &home), payload.path);
    let file_path = Path::new(&full_path);
    let exists = file_path.exists();

    Json(serde_json::json!({
        "exists": exists,
        "in_sync_list": true,
        "status": item.sync_status.as_str(),
        "is_file": item.is_file
    })).into_response()
}

async fn update_stignore(id: String, _delete: bool) {
    let conn = match database_connect() {
        Ok(c) => c,
        Err(e) => {
            out::error(SCRIPT, &format!("Database connection failed: {}", e));
            return;
        }
    };
    
    // Update .stignore file
    let items = match get_selected_items(&conn) {
        Ok(Some(items)) => items,
        Ok(None) => return,
        Err(e) => {
            out::error(SCRIPT, &format!("Failed to get selected items: {}", e));
        return;
    }
    };
    let folders = get_folders().await.unwrap_or_default();

    out::secret(SCRIPT, &format!("Folders: {:#?}", folders));

    let mut new_file = String::from("// DO NOT CHANGE MANUALY");

    for item in items {
        if item.root == id {
            new_file = format!("{new_file}\n!{}", item.path);
        }
    }

    new_file = format!("{new_file}\n\n*");
    
    // Get the file
    for folder in folders {
        if folder.id == id {
            // Get the file here and edit it
            let home = home_dir().unwrap().to_string_lossy().to_string();
            let r = format!("{}/.stignore", folder.path.replace("~", &home));
            let path = Path::new(&r);
            out::highlight(SCRIPT, &format!("{path:?}"));

            if cfg!(target_os = "windows") {
                unhide_file_windows(path.to_str().unwrap());
            }

            let _ = fs::write(path, &new_file);
        }
    }

    // Rescan the folder
    let client = match config::create_http_client() {
        Ok(c) => c,
        Err(e) => {
            out::error(SCRIPT, &format!("Failed to create HTTP client: {}", e));
            return;
        }
    };
    
    let conn = match database_connect() {
        Ok(c) => c,
        Err(e) => {
            out::error(SCRIPT, &format!("Database connection failed: {}", e));
            return;
        }
    };
    
    let ss = match get_site_setting(&conn, "st-url") {
        Some(s) => s,
        None => {
            out::error(SCRIPT, "st-url not found in settings");
            return;
        }
    };
    
    let st_api_key = match get_site_setting(&conn, "api-key") {
        Some(s) => s,
        None => {
            out::error(SCRIPT, "api-key not found in settings");
            return;
        }
    };
    let _res = client.post(format!("https://{}/rest/db/scan?folder={id}", ss.value))
    .header("X-API-Key", st_api_key.value)
    .send().await;
}

fn unhide_file_windows(file_path: &str) {
    let output = Command::new("cmd")
        .args(&["/C", "attrib", "-H", file_path])
        .output();

    if output.is_err() {
        out::error(SCRIPT, "CMD Failed");
    }

    let output = output.unwrap();

    if !output.status.success() {
        out::error(SCRIPT, "Could not edit file .stignore");
    }
}

//##### GET FOLDERS HTML #####\\
async fn create_html_folders() -> String {
    let folders = get_folders().await;
    let mut html_folders = "".to_string();
    let res = folders.unwrap_or_default();
    // Get the user
    let r = res.clone();
    for folder in r {
        html_folders = create_html(html_folders, folder.label, folder.id, folder.path, folder.filesystemType);
    }
    
    html_folders
}

async fn get_folders() -> Option<Vec<Folder>> {
    let client = match config::create_http_client() {
        Ok(c) => c,
        Err(e) => {
            out::error(SCRIPT, &format!("Failed to create HTTP client: {}", e));
            return None;
        }
    };
    
    let conn = match database_connect() {
        Ok(c) => c,
        Err(e) => {
            out::error(SCRIPT, &format!("Database connection failed: {}", e));
            return None;
        }
    };
    
    let ss = match get_site_setting(&conn, "st-url") {
        Some(s) => s,
        None => {
            out::error(SCRIPT, "st-url not found in settings");
            return None;
        }
    };
    
    let st_api_key = match get_site_setting(&conn, "api-key") {
        Some(s) => s,
        None => {
            out::error(SCRIPT, "api-key not found in settings");
            return None;
        }
    };
    let res = client.get(format!("https://{}/rest/config/folders", ss.value))
    .header("X-API-Key", st_api_key.value)
    .send().await;
    // println!("RES: {res:?}");
    if res.is_err() {
        return None;
    }
    let res = res.unwrap().json::<Vec<Folder>>().await;
    // println!("RES: {res:?}");
    if res.is_err() {
        return None;
    }
    Some(res.unwrap())
}

fn create_html(mut html: String, label: String, id: String, path: String, fst: String) -> String {
    html = format!(
        "{html}<a href=\"/items\" class=\"folder-item\"><div class=\"folder-name\">{}</div><div class=\"folder-detail\">ID: {}</div><div class=\"folder-detail\">Path: {}</div><div class=\"folder-detail\">File System: {}</div></a>",
        label,
        id,
        path,
        fst
    );
    html
}

/// Determines if `child_path` is a subpath of `parent_path`
fn is_child_path(parent_path: &str, child_path: &str) -> bool {
    child_path.starts_with(parent_path) && child_path.len() > parent_path.len()
}

/// Updates the vector by replacing child paths with their respective parent paths.
fn consolidate_paths(folder_files: &mut Vec<FolderFileSaved>) {
    let mut parent_map: HashMap<String, String> = HashMap::new();

    // First, find parent-child relationships
    for i in 0..folder_files.len() {
        for j in 0..folder_files.len() {
            if i != j && is_child_path(&folder_files[j].path, &folder_files[i].path) {
                parent_map.insert(folder_files[i].path.clone(), folder_files[j].path.clone());
            }
        }
    }

    // Replace child paths with their parent paths
    for file in folder_files.iter_mut() {
        if let Some(new_parent) = parent_map.get(&file.path) {
            file.path = new_parent.clone();
        }
    }

    // Remove duplicates that now have the same path
    folder_files.sort_by(|a, b| a.path.cmp(&b.path));
    folder_files.dedup_by(|a, b| a.path == b.path);
}

/// Checks if a given `FolderFile` has a parent in the `FolderFileSaved` list.
/// If it's a child, its path is updated to match the parent's.
fn update_folder_file(folder_file: &mut FolderFile, folder_files: &Vec<FolderFileSaved>) {
    for saved in folder_files {
        if is_child_path(&saved.path, &folder_file.path) {
            folder_file.path = saved.path.clone();
            break;
        }
    }
}