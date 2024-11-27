# Syncthing-Select-Sync

**Select Sync for Syncthing**

#### **_This project is currently in development. Expect Bugs._**

---

![SSSS Home page](/docs/Images/SSSC-Home.png
)

---

![SSSC Folder File page](/docs/Images/SSSC-Home.png)

---

## Table of Contents
1. [Client Setup](#client-setup)
   - [Site Settings](#site-settings)
   - [Logging In](#logging-in)
   - [Using Select Sync](#using-select-sync)
2. [Server Setup](#server-setup)
   - [Site Settings](#site-settings-1)
   - [Adding Users](#adding-users)
   - [Editing Users](#editing-users)
   - [Admin User Configuration](#admin-user-configuration)
3. [Docs](docs/main-docs.md)

---

## Client Setup

To configure the client, open your browser and navigate to [http://127.0.0.1:8383](http://127.0.0.1:8383).

### Site Settings
1. Click on **Site Settings**.
2. Fill out the following fields:
   - **Syncthing API Token**: API token for the local Syncthing instance.
   - **SSSS URL**: URL of the Syncthing Select Sync Server. Use the format: `http(s)://<DOMAIN || IP:PORT>`. For HTTPS, use a reverse proxy (e.g., Cloudflare).
   - **SSSS User**: Your username for the Syncthing Select Sync Server.
   - **SSSS Pass**: Your password for the Syncthing Select Sync Server.
   - **Syncthing URL**: Local IP and port (e.g., `127.0.0.1:8384`).

3. Click **Save Site Settings**.  
   - To update settings later, modify only the required fields and save again.

### Logging In
To log in, click the text next to **Site Settings** to check your status:
- **(Not Logged In)**: Not logged in yet.
- **Not Logged In**: Login attempt failed (incorrect credentials).
- **Server Error**: Unable to connect to the server.
- **Logged In**: Successfully logged in.

### Using Select Sync
1. Select an item in the **Accessible Folders** section to open the **Folder and File Explorer**.
2. Use the interface to sync files and folders:
   - **Sync Checkbox**: 
     - Checking: Adds the item to the `.stignore` file and starts syncing.
     - Unchecking: Updates the `.stignore` file and deletes the item locally to free up space.

---

## Server Setup

To configure the server, open your browser and navigate to `http://<SERVER_IP>:8383`. If SSL is set up, use the secure URL. By default, the login credentials are:

- **Username**: `ADMIN`
- **Password**: `ADMIN`

### Site Settings
1. Click on **Site Settings**.
2. Fill out the following fields:
   - **Syncthing API Token**: API token for the local Syncthing instance.
   - **SSSS URL**: Default is `0.0.0.0:8383`. Leave the IP as `0.0.0.0` to allow access from both LAN and WAN.
   - **Syncthing URL**: Local IP and port (e.g., `127.0.0.1:8384`).

3. Click **Save Site Settings**.  
   - To update settings later, modify only the required fields and save again.

### Adding Users
To add a user:
1. Enter the following details:
   - **Username**: The user's username.
   - **Password**: The user's password.
   - **Role**: A value between `0-255` (where `255` is `ADMIN`).
   - **Folders**: A JSON string of folders the user can access (e.g., `"[{'folder': 'FOLDER_ID'}]"`).
2. Click **Add User**.

### Editing Users
Admins can update any user, while regular users can update only their own information. Use the same fields as in **Adding Users**.

### Admin User Configuration
After creating a new admin user or setting up your personal account:
1. Change the default `ADMIN` user's password.
2. Set the `ADMIN` user's role to `0`.
3. Clear the `Folders` field by setting it to `[]`.  
   This ensures the default admin account is no longer usable, enhancing security.

---

### Notes
- Always secure your setup with HTTPS if exposing it to the internet.
- Keep API tokens and passwords confidential.
