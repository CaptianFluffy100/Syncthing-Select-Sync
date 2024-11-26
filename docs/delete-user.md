# Delete User for Syncthing Select Sync

The `delete-user` endpoint allows for removing a user from the system. This action is restricted to admin users to prevent unauthorized deletions.

## Endpoint

- **HTTP Method:** `DELETE`
- **Path:** `/api/delete-user`

## Field Requirements

- **`username`**: Represents the username of the user to be deleted. This field is required.

## Permissions

- **Admin Users Only:** Only users with admin privileges are authorized to use this endpoint.

## Response Codes

- **`200 OK`**: The user was successfully deleted.
- **`403 Forbidden`**: The client does not have admin privileges and is not authorized to delete users.
- **`418 Im A teapot`**: User not logged in.
- **`500 Internal Server Error`**: Somthing went wrong and user is not deleted.

## Summary

The `delete-user` endpoint is a secure method for managing user accounts. 

- **Required Field:** `username`
- **Restricted Access:** Admin-only action

By limiting access to admin users and validating inputs, this endpoint ensures that user deletions are performed securely and responsibly. Developers should handle error responses appropriately to provide clear feedback to the client.

[BACK](main-docs.md)
