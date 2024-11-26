# Add User for Syncthing Select Sync

The `add-user` endpoint allows for adding a new user to the system. This action is restricted to admin users and requires a valid user object in the request body.

## Endpoint

- **HTTP Method:** `POST`
- **Path:** `/api/add-user`

## Request Structure

The request must include a JSON object with the following fields:

```json
{
    "id": <NUMBER_NOT_NEEDED>,
    "username": "<STRING_NEEDED>",
    "password": "<LEAVE_AS_HASH_OR_PUT_NEW_PASSWORD>",
    "salt": "<STRING_NOT_NEEDED>",
    "role": 0-255,  // 255 is defualt ADMIN
    "allowed_folders": "<JSON_STRING_NEEDED>"
}
```

```json
// allowed_folders string
[
    {
        "folder": "<STRING_FOLDER_ID>"
    }
]
```

## Field Requirements

The request must include a JSON body with the following fields:

- **`username`**: Represents the new user's username. This field is required.
- **`password`**: Represents the new user's password. Must be hashed using SHA256 before being sent. This field is required.
- **`role`**: Represents the user's role. This field is required.
- **`allowed_folders`**: Represents the paths the user is allowed to access. This field is required.

The server will automatically generate a unique `id` for the user and a `salt` for hashing the password.

## Permissions

- **Not Logged In**: The client must be logged in to perform this action. If not logged in, the response will be `IM_A_TEAPOT`.
- **Admin Users Only**: Only users with admin privileges can add new users. If the client is not an admin, the response will be `FORBIDDEN`.

## Response Codes

- **`200 OK`**: The user was successfully added to the system.
- **`403 Forbidden`**: The client does not have admin privileges and is not authorized to add new users.
- **`418 I'm a teapot`**: The client is not logged in and cannot add users.
- **`500 Internal Server Error`**: An unexpected error occurred on the server while processing the user addition.

## Summary

The `add-user` endpoint enables admin users to add new users to the system. 

- **Required Fields:** `username`, `password`, `role`, `allowed_folders`
- **Automatically Generated Fields:** `id`, `salt`
- **Restricted Access:** Admin-only action, and the client must be logged in.

This endpoint ensures that only authorized users can create new accounts while handling errors appropriately to maintain system integrity.

[BACK](main-docs.md)
