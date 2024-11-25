use std::fs;
use std::io::BufRead;
use std::path::Path;
use std::fs::File;
use std::io::BufReader;
use std::time::Duration;
use axum::response::Html;
use axum::routing::delete;
use database::create_user::create_user;
use database::create_user::generate_salt;
use database::create_user::get_all_users;
use database::create_user::sha256_hash;
use database::db::database_connect;
use database::db::database_get_user;
use database::db::get_site_setting;
use database::db::set_site_setting;
use guest::GuestData;
use reqwest::Client;
use sha2::Sha256;
use sha2::Digest;
use structs::DeleteUser;
use structs::Folder;
use structs::FolderFile;
use structs::FolderSearch;
use structs::User;
use structs::UserUpdate;
use std::io::Read;
use tower::ServiceBuilder;
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer};
use dirs::home_dir;

mod guest;
mod database;
mod structs;
mod out;

use axum::{
    error_handling::HandleErrorLayer, BoxError, http::StatusCode, response::{Response, IntoResponse}, routing::{get, post}, Json, Router
};
use serde::{Deserialize, Serialize};

use crate::guest::Guest;

const ADMIN: u8 = 255; 

#[derive(Debug)]
struct Config {
    folder: String,
    path: String,
}

impl Config {
    fn new() -> Self {
        Config {
            folder: String::new(),
            path: String::new(),
        }
    }
}

const SCRIPT: &str = "MAIN";

#[tokio::main]
async fn main() {
    out::ok(SCRIPT, "Starting Syncthing Select Sync Server (SSSS)");
    // Get the url for the website
    let conn = database_connect();
    let ss = get_site_setting(&conn, "ssss-url").unwrap();

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


    // build our application with a route
    let app = Router::new()
        // `GET /` goes to `root`
        .route("/login", get(login_page))
        .route("/", get(home_page))
        .route("/ping", get(ping))
        .route("/check-status", get(check_status))
        .route("/api/logout", post(logout))
        .route("/api/login", post(login))
        .route("/api/update-password", post(update_password))
        .route("/api/update-user", post(update_user))
        .route("/api/delete-user", delete(delete_user))
        .route("/api/add-user", post(add_user))
        .route("/api/user-allowed-folders", get(get_user_allowed_folders))
        .route("/api/ssss/get-items", post(get_items_in_folder))
        .route("/api/update-site-settings", post(update_site_settings))
        .layer(session_service);

    // run our app with hyper, listening globally on port 8383
    let listener = tokio::net::TcpListener::bind(&ss.value).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn ping(mut guest: Guest) -> StatusCode {
    // println!("Guest: {guest:?}");
    return StatusCode::OK;
}

async fn check_status(mut guest: Guest) -> StatusCode {
    // println!("Guest: {guest:?}");
    if guest.guest_data.logged_in {
        return StatusCode::OK;
    } else {
        return StatusCode::IM_A_TEAPOT;
    }
}

#[derive(Deserialize, Debug)]
struct LoginUser {
    username: String,
    password: String
}

async fn login(
    mut guest: Guest,
    Json(payload): Json<LoginUser>,
) -> StatusCode {
    // println!("Payload: {payload:?}");
    out::ok(SCRIPT, "Logging in User!");
    
    let conn = database_connect();

    let found_user = database_get_user(&conn, payload.username);

    if found_user.is_none() {
        return StatusCode::IM_A_TEAPOT;
    }

    let user = found_user.unwrap();

    if sha256_hash(payload.password, user.salt) == user.password {
        // login
        guest.guest_data.username = user.username.clone();
        guest.guest_data.logged_in = true;
        guest.guest_data.role = user.role;
        Guest::update_session(&guest.session, &guest.guest_data);
        return StatusCode::OK;
    }
    
    return StatusCode::SERVICE_UNAVAILABLE;
}

async fn logout(
    mut guest: Guest
) -> StatusCode {
    guest.guest_data.logged_in = false;
    guest.guest_data.username = "".to_string();
    Guest::update_session(&guest.session, &guest.guest_data);
    StatusCode::ACCEPTED
}

async fn get_user_allowed_folders(
    mut guest: Guest
) -> impl IntoResponse {
    // Check if user exists
    if !guest.guest_data.logged_in {
        return (StatusCode::IM_A_TEAPOT, "User not logged in").into_response();
    }
    let conn = database_connect();
    let user = User::get_user(&conn, &guest.guest_data.username).unwrap();

    Json(user.allowed_folders).into_response()
}

async fn get_items_in_folder(
    guest: Guest,
    Json(payload): Json<FolderSearch>,
) -> impl IntoResponse {
    out::highlight(SCRIPT, &format!("Payload: {payload:?}"));
    // Check if user exists
    if !guest.guest_data.logged_in {
        return (StatusCode::IM_A_TEAPOT, "User not logged in").into_response();
    }

    let folders = get_folders().await;
    if folders.is_none() {
        return (StatusCode::SERVICE_UNAVAILABLE, "Failed to get folders").into_response();
    }

    let folders = folders.unwrap();

    // // println!("Folders: {folders:?}");

    for folder in folders {
        if folder.id == payload.id {
            // Get the files and folders in the directory
            let home = home_dir().unwrap().to_string_lossy().to_string();
            let r = format!("{}/", folder.path.replace("~", &home));
            let p = format!("{}/{}", folder.path.replace("~", &home), payload.path);
            // // println!("Path: {p}");
            let path = Path::new(&p);
            let files_and_folders = read_dir_recursive(path, r, payload.id.clone());
            return Json(files_and_folders).into_response();
        }
    }
    return (StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS, "UNAVAILABLE_FOR_LEGAL_REASONS").into_response();
}

fn read_dir_recursive(dir: &Path, replace: String, id: String) -> Vec<FolderFile> {
    let mut entries = Vec::new();

    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir {
            if let Ok(entry) = entry {
                let path = entry.path();
                let is_file = path.is_file();
                let name = path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Unknown".to_string());

                entries.push(FolderFile {
                    id: id.clone(),
                    name: name.clone(),
                    path: path.to_string_lossy().to_string().replace(&replace, ""),
                    is_file,
                });
            }
        }
    } else {
        // println!("Dir Error: {:?}", fs::read_dir(dir));
        out::error(SCRIPT, &format!("Dir Error: {:?}", fs::read_dir(dir)));
    }

    entries
}

#[derive(Deserialize, Debug)]
struct NewPassword {
    password: String
}

async fn update_password(
    mut guest: Guest,
    Json(payload): Json<NewPassword>,
) -> StatusCode {
    // println!("Payload: {payload:?}");
    if !guest.guest_data.logged_in {
        return StatusCode::IM_A_TEAPOT;
    }
    
    let conn = database_connect();

    let res = User::update_password(&conn, guest.guest_data.username.clone(), payload.password.clone());
    // // println!("Res: {res:?}");
    if res {
        return StatusCode::OK;
    } else {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
}

async fn update_user(
    mut guest: Guest,
    Json(payload): Json<UserUpdate>,
) -> StatusCode {
    // println!("Payload: {payload:?}");
    if !guest.guest_data.logged_in {
        return StatusCode::IM_A_TEAPOT;
    }
    
    let conn = database_connect();

    let user = User {
        id: 0,
        username: payload.username,
        password: payload.password,
        salt: "".to_string(),
        role: payload.role,
        allowed_folders: serde_json::from_str(&payload.allowed_folders).unwrap()
    };

    let res = User::update_user(&conn, user, guest.clone());
    // // println!("Res: {res:?}");
    if res {
        return StatusCode::OK;
    } else {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
}

async fn delete_user(
    mut guest: Guest,
    Json(payload): Json<DeleteUser>,
) -> StatusCode {
    // println!("Payload: {payload:?}");
    if !guest.guest_data.logged_in {
        return StatusCode::IM_A_TEAPOT;
    }

    if guest.guest_data.role != ADMIN {
        return StatusCode::FORBIDDEN;
    }
    
    let conn = database_connect();

    let res = User::delete_user(&conn, &payload.username);
    // // println!("Res: {res:?}");
    if res {
        return StatusCode::OK;
    } else {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
}

async fn add_user(
    mut guest: Guest,
    Json(payload): Json<UserUpdate>,
) -> StatusCode {
    // println!("Payload: {payload:?}");
    if !guest.guest_data.logged_in {
        return StatusCode::IM_A_TEAPOT;
    }

    if guest.guest_data.role != ADMIN {
        return StatusCode::FORBIDDEN;
    }
    
    let conn = database_connect();

    let mut user = User {
        id: 0,
        username: payload.username,
        password: payload.password,
        salt: "".to_string(),
        role: payload.role,
        allowed_folders: serde_json::from_str(&payload.allowed_folders).unwrap()
    };

    user.salt = generate_salt();
    user.password = sha256_hash(user.password, user.salt.clone());

    let res = create_user(&conn, user);
    // // println!("Res: {res:?}");
    if res {
        return StatusCode::OK;
    } else {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
}

async fn login_page(
    mut guest: Guest,
) -> impl IntoResponse {
    // Get the html page
    let page = fs::read_to_string("./html/login.html").unwrap();
    Html(page).into_response()
}

async fn home_page(
    mut guest: Guest,
) -> impl IntoResponse {
    // println!("Guest: {guest:?}");
    // Get the html page
    if guest.guest_data.logged_in {
        // get all folders
        let page = fs::read_to_string("./html/home.html").unwrap();
        let page = page.replace("<Folders/>", &create_html_folders(guest.guest_data.clone()).await);
        let page = page.replace("<Users/>", &create_html_users(guest.guest_data.clone()).await);
        let page = page.replace("<UsersData/>", &create_json_users(guest.guest_data.clone()).await);

        Html(page).into_response()
    } else {
        let page = fs::read_to_string("./html/login.html").unwrap();
        Html(page).into_response()
    }
}

//##### SET SITE SETTINGS #####\\
#[derive(Deserialize, Debug)]
struct SetSiteSettings {
    api_token: String,
    st_url: String,
    ssss_url: String
}

async fn update_site_settings(
    mut guest: Guest,
    Json(payload): Json<SetSiteSettings>,
) -> StatusCode {
    // println!("Payload: {payload:?}");
    if !guest.guest_data.logged_in {
        return StatusCode::IM_A_TEAPOT;
    }

    if guest.guest_data.role != ADMIN {
        return StatusCode::FORBIDDEN;
    }
    
    let conn = database_connect();

    // Update each field in site settings
    if !payload.api_token.is_empty() {
        let _ = set_site_setting(&conn, "api-key", &payload.api_token);
    }

    if !payload.st_url.is_empty() {
        let _ = set_site_setting(&conn, "st-url", &payload.st_url);
    }

    if !payload.ssss_url.is_empty() {
        let _ = set_site_setting(&conn, "ssss-url", &payload.ssss_url);
    }

    StatusCode::OK
}

//##### GET FOLDERS HTML #####\\
async fn get_folders() -> Option<Vec<Folder>> {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)  // Skip certificate verification
        .cookie_store(true)
        .timeout(Duration::from_secs(30)) // Optional: Set a timeout
        .build().unwrap();
    let conn = database_connect();
    let ss = get_site_setting(&conn, "st-url").unwrap();
    let st_api_key = get_site_setting(&conn, "api-key").unwrap();
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

async fn create_html_folders(guest: GuestData) -> String {
    let folders = get_folders().await;
    let mut html_folders = "".to_string();
    let res = folders.unwrap_or_default();
    let conn = database_connect();
    // Get the user
    let user = User::get_user(&conn, &guest.username);
    if user.is_none() {
        return "".to_string();
    }
    let user = user.unwrap();

    if guest.role == ADMIN {
        let r = res.clone();
        for folder in r {
            html_folders = create_html(html_folders, folder.label, folder.id, folder.path, folder.filesystemType);
        }
    } else {
        for allowed_folder in user.allowed_folders {
            let r = res.clone();
            for folder in r {
                if folder.id == allowed_folder.folder {
                    html_folders = create_html(html_folders, folder.label, folder.id, folder.path, folder.filesystemType);
                }
            }
        }
    }
    
    html_folders
}

fn create_html(mut html: String, label: String, id: String, path: String, fst: String) -> String {
    html = format!(
        "{html}<div class=\"folder-item\"><div class=\"folder-name\">{}</div><div class=\"folder-detail\">ID: {}</div><div class=\"folder-detail\">Path: {}</div><div class=\"folder-detail\">File System: {}</div></div>",
        label,
        id,
        path,
        fst
    );
    html
}

//##### GET USERS HTML #####\\
async fn get_users(guest: GuestData) -> Option<Vec<User>> {
    let conn = database_connect();
    let users = get_all_users(&conn);
    if guest.role == ADMIN {
        return Some(users.clone());
    }

    for user in users {
        if user.username == guest.username {
            return Some(vec![user]);
        }
    }
    None
}

async fn create_html_users(guest: GuestData) -> String {
    let users = get_users(guest).await;
    let mut html_users = "".to_string();
    let res = users.unwrap_or_default();
    for user in res {
        html_users = format!(
            "{html_users}<tr data-id=\"user-{}\">
                        <td>{}</td>
                        <td>********</td>
                        <td>{}</td>
                        <td>{}</td>
                    </tr>",
                user.id,
                user.username,
                user.role,
                user.allowed_folders.len()
        );
    }
    html_users
}

//##### GET USERS JSON #####\\
async fn create_json_users(guest: GuestData) -> String {
    let users = get_users(guest).await;
    let res = users.unwrap_or_default();
    let json = serde_json::to_string(&res).unwrap();
    json
}
