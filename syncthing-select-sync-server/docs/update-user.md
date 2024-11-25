# Update User for Syncthing Select Sync

The `update-user` endpoint allows for modifying user details while maintaining restrictions on certain fields. This ensures controlled updates and adheres to the system's security and access policies.

## Endpoint

- **HTTP Method:** `POST`
- **Path:** `/api/update-user`

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

- **`id`**: Represents the user's unique identifier. This field is required but does not get updated by the endpoint.
- **`username`**: Represents the user's username. This field is required but cannot be changed.
- **`password`**: Represents the user's new password.
- **`salt`**: Represents the salt value used for hashing the password. This field is required but does not get updated by the endpoint.
- **`role`**: Represents the user's role. This field can only be changed by an admin user.
- **`allowed_folders`**: Represents the paths/folders the user is allowed to access. This field can only be changed by an admin user.

## Permissions

- **Non-admin Users:** Can update the `password` field only.
- **Admin Users:** Can update the `role` and `allowed_folders` fields in addition to the `password` field.

## Response Codes

- **`200 OK`**: The user was successfully updated.
- **`418 Im A Teapot`**: User is not logged in.
- **`500 Internal Server Error`**: Did not update user. Somthing went wrong, or user does not have permition to update user.

## Summary

The `update-user` endpoint is designed to securely update user details:

- **Unchangeable Fields:** `id`, `username`, `salt`
- **Admin-only Fields:** `role`, `allowed_folders`
- **Updatable by All Users:** `password`

By enforcing these rules, the system ensures user data integrity while allowing controlled updates. Developers should handle permissions and error responses appropriately in their implementations.

[BACK](main-docs.md)
