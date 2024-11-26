# Syncthing-Select-Sync
Select Sync for Syncthing.

#### ***This project is still in development.***


### Setup Client

After starting up the client you will need to change a few settings.  So first thing is open up your favorite browser and go to http://127.0.0.1:8383.

#### Site Settings
Click on "Site Settings".
    - **Syncthing API Token**: The API token for the local Syncthing API key.
    - **SSSS URL**: This is the Syncthing Select Sync Server URL.  `http(s)://<DOMAIN || IP:PORT>`.  For HTTPS use a reverse proxy with Cloud Flare (Or someone else).
    - **SSSS User**: Your username for the Syncthing Select Sync Server.
    - **SSSS Pass**: Your password for the Syncthing Select Sync Server.
    - **Syncthing URL**: Local ip and port: 127.0.0.1:8384.

After filling these feilds click `Save Site Settings`.  If you need to change somthing later on, just fill one of the fields and click `Save Site Settings`.  Only the feilds that have data will be saved.

#### Logging in
To log into the server with the client, click on the text next to `Site Settings`.
    - `(Not Logged in)` > Not logged in.
    - `Not Logged in` > Tried to log in, but could not because creds are wrong.
    - `Server Error` > Could not connect to server.
    - `Logged In` > Logged in.

#### Select Sync
To use the select sync, click on a item in the **Accessible Folders** Section.  This will take you to a new page called **Folder and File Explorer**.  Here you can select folders and files to sync from Syncthing.  This works by editing the .stignore file of the root folder.  Clicking on the Sync checkbox will update the .stignore and will start syncing the item.  Unchecking the box will update the .stignore file and then delete it on the client to save space.

### Setup Server

After starting up the server you will need to change a few settings.  So first thing is open up your favorite browser and go to `http://<IP OF SERVER>:8383`.  If you have SSL setup with the server then just go to the url.  You will be met with a login screen with the default login as `ADMIN` as both username and password.

#### Site Settings
Click on "Site Settings".
    - **Syncthing API Token**: The API token for the local Syncthing API key.
    - **SSSS URL**: Leave this alown because if is default of `0.0.0.0:8383`. You can change the port here, but leave the IP as `0.0.0.0` so you can access it anywhere on LAN or WAN
    - **Syncthing URL**: Local ip and port: `127.0.0.1:8384`.

After filling these feilds click `Save Site Settings`.  If you need to change somthing later on, just fill one of the fields and click `Save Site Settings`.  Only the feilds that have data will be saved.

#### Add User
Adding a new user.
    - **Username**: Username for a new user.
    - **Password**: New password for a user.
    - **Role**: `0-255`. `255` is `ADMIN`.
    - **Folders**: `[{'folder': 'FOLDER_ID'}]`.  This is a string of all files the user can access. (Can only be changed by a `ADMIN` user).

#### Edit User
Edit a user. It has the same data as the add user, and the `ADMIN` can update any user, but a regular user can update there own user.

#### ADMIN USER
One you create a new ADMIN user or your own user, change the ADMIN password, and change the role to 0, and change the folders to `[]`.  This way it makes this user worthless.