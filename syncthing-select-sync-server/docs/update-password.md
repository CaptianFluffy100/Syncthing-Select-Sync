# Update Password for Syncthing Select Sync

`/api/update-password`

The `update-password` endpoint allows logged-in users to change their account password. This process ensures secure and seamless password updates while maintaining session integrity.

## Requirements

- **Authentication:**  
  The user must be logged in to update their password. Requests from unauthenticated users will be rejected.

- **Request Payload:**  
  The client must send the new password in the request body as a JSON object. Example:  
  `{ "password": "your_new_password" }`

## Response Codes

### Successful Update

- **Status Code:** `200 OK`  
  If the new password is successfully updated, the server will respond with `200 OK`. The user can immediately use the new password for future logins.

### Error Responses

1. **User Not Logged In**  
   - **Status Code:** `418 I'm a teapot`  
     This status indicates that the user is not logged in. The client should prompt the user to log in before attempting to update the password.

2. **Internal Server Error**  
   - **Status Code:** `500 Internal Server Error`  
     If an unexpected issue occurs on the server (e.g., a database error), the update will fail, and the server will return `500 Internal Server Error`. The client should inform the user that the password update was unsuccessful and suggest retrying later.

## Error Handling

The client should handle responses as follows:

- **`418 I'm a teapot`:**  
  Inform the user that they must be logged in to update their password. Prompt them to log in and try again.

- **`500 Internal Server Error`:**  
  Notify the user that the password update failed due to a server issue. Suggest trying again later.

## Summary

The `update-password` endpoint provides a secure way for logged-in users to change their password. Clients should handle the following possible responses:

- **`200 OK`**: Password updated successfully.
- **`418 I'm a teapot`**: User is not logged in.
- **`500 Internal Server Error`**: Server error occurred during the update.

By following these guidelines, developers can implement reliable and user-friendly password update functionality.

[BACK](main-docs.md)
