use std::{fs, path::Path, process::Command, sync::Arc, time::Duration};

use axum::{error_handling::HandleErrorLayer, extract::State, response::{Html, IntoResponse}, routing::{delete, get, post}, Json, Router};
use database::db::{create_selected_item, database_connect, delete_selected_item, get_selected_item, get_selected_items, get_site_setting, set_site_setting};
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
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)  // Skip certificate verification
        .cookie_store(true)
        .timeout(Duration::from_secs(30)) // Optional: Set a timeout
        .build().unwrap();
    let client = Arc::new(client);

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
        .layer(session_service)
        .with_state(client);
    // run our app with hyper, listening globally on port 8383
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8383").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn home_page(State(client): State<Arc<Client>>) -> impl IntoResponse {
    // println!("Guest: {guest:?}");
    // Get the html page
    let page = fs::read_to_string("./html/home.html").unwrap();
    // Check if user is logged in
    // Get the url for the website
    let conn = database_connect();
    let ss = get_site_setting(&conn, "ssss-url").unwrap();
    out::debug(SCRIPT, &format!("SiteSettings: {ss:?}"));

    let mut logged_in = "(Not Logged in)".to_string();

    match client.get(&format!("{}/check-status", ss.value)).send().await {
        Ok(response) => {
            // Get respones status
            if response.status() == StatusCode::OK {
                logged_in = "Logged In".to_string();
            } else if response.status() == StatusCode::IM_A_TEAPOT {
                logged_in = "Not Logged In".to_string();
            } else {
                logged_in = "Server Error".to_string();
            }
        },
        Err(err) => out::error(SCRIPT, &format!("Request failed: {:?}", err)),
    };

    let page = page.replace("<LogInStatus/>", &logged_in);
    let page = page.replace("<Folders/>", &create_html_folders().await);

    Html(page).into_response()
}

async fn items_page(State(client): State<Arc<Client>>) -> impl IntoResponse {
    // println!("Guest: {guest:?}");
    // Get the html page
    let page = fs::read_to_string("./html/items.html").unwrap();

    Html(page).into_response()
}

async fn login(State(client): State<Arc<Client>>) -> StatusCode {
    // Check if user is logged in
    // Get the url for the website
    let conn = database_connect();
    let ss = get_site_setting(&conn, "ssss-url").unwrap();
    out::debug(SCRIPT, &format!("SiteSettings: {ss:?}"));

    let conn = database_connect();
    let ss_user = get_site_setting(&conn, "ssss-user").unwrap();

    let conn = database_connect();
    let ss_pass = get_site_setting(&conn, "ssss-pass").unwrap();

    match client.post(&format!("{}/api/login", ss.value)).header("Content-Type", "application/json").json(&LoginUser{username: ss_user.value, password: ss_pass.value}).send().await {
        Ok(response) => {
            // Get respones status
            out::debug(SCRIPT, &format!("{:?}", response));
            response.status()
        },
        Err(_) => StatusCode::SERVICE_UNAVAILABLE,
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
    // println!("Payload: {payload:?}");
    let conn = database_connect();

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
    // println!("Payload: {payload:?}");
    let conn = database_connect();

    let res = create_selected_item(&conn, payload.clone());

    // Update .stignore file
    update_stignore(payload.id, false).await;

    if res {
        return StatusCode::OK;
    } else {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
}

async fn del_selected_item(
    Json(payload): Json<DeleteSelected>,
) -> StatusCode {
    println!("Payload: {payload:?}");
    let conn = database_connect();
    let item = get_selected_item(&conn, payload.id);
    let res = delete_selected_item(&conn, payload.id);

    if item.is_none() {
        return StatusCode::NOT_FOUND;
    }

    // Update .stignore file
    update_stignore(payload.root.clone(), true).await;

    let item = item.unwrap();
    delete_folder(item, payload.root).await;

    if res {
        return StatusCode::OK;
    } else {
        return StatusCode::INTERNAL_SERVER_ERROR;
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
    let conn = database_connect();
    let ss = get_site_setting(&conn, "ssss-url").unwrap();
    out::bright(SCRIPT, &format!("ssss-url: {ss:?}"));
    // Get the data
    match client.post(&format!("{}/api/ssss/get-items", ss.value)).json(&fs).send().await {
        Ok(response) => {
            // Get respones status
            let res = response.json::<Vec<FolderFile>>().await.unwrap_or_default();
            return res;
        },
        Err(err) => out::error(SCRIPT, &format!("Request failed: {:?}", err)),
    };
    return vec![];
}

async fn get_all_selected_items() -> impl IntoResponse {
    // println!("Payload: {payload:?}");
    let conn = database_connect();

    let res = get_selected_items(&conn);

    Json(res).into_response()
}

async fn update_stignore(id: String, delete: bool) {
    let conn = database_connect();
    // Update .stignore file
    let items = get_selected_items(&conn);
    let folders = get_folders().await.unwrap_or_default();

    let mut new_file = String::from("#DO NOT CHANGE MANUALY");

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
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)  // Skip certificate verification
        .cookie_store(true)
        .timeout(Duration::from_secs(30)) // Optional: Set a timeout
        .build().unwrap();
    let conn = database_connect();
    let ss = get_site_setting(&conn, "st-url").unwrap();
    let st_api_key = get_site_setting(&conn, "api-key").unwrap();
    let res = client.post(format!("https://{}/rest/db/scan?folder={id}", ss.value))
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
    let conn = database_connect();
    // Get the user
    let r = res.clone();
    for folder in r {
        html_folders = create_html(html_folders, folder.label, folder.id, folder.path, folder.filesystemType);
    }
    
    html_folders
}

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

fn create_html(mut html: String, label: String, id: String, path: String, fst: String) -> String {
    let onclick = format!(
        ""
    );
    html = format!(
        "{html}<a href=\"/items\" class=\"folder-item\"><div class=\"folder-name\">{}</div><div class=\"folder-detail\">ID: {}</div><div class=\"folder-detail\">Path: {}</div><div class=\"folder-detail\">File System: {}</div></a>",
        label,
        id,
        path,
        fst
    );
    html
}