# Update Site Settings

#### `/api/update-site-settings`

**Method:**  
`POST`

---

## Description  
This endpoint allows updating the site settings. It requires the user to be logged in and have admin privileges.

---

## Request Payload  
The request must include the following fields:  

- **api_token**: The API token for authentication with external services.  
- **st_url**: The Syncthing service URL.  
- **ssss_url**: The Syncthing Select Sync service URL.

##### JSON Payload for `/api/update-site-settings`

The payload must be structured as follows:

| Field       | Type   | Description                                         |
|-------------|--------|-----------------------------------------------------|
| `api_token` | String | The API token for authentication with external services. |
| `st_url`    | String | The URL of the Syncthing service.                   |
| `ssss_url`  | String | The URL of the Syncthing Select Sync service.       |

## Example  
```json
{
    "api_token": "example-token",
    "st_url": "https://syncthing.example.com",
    "ssss_url": "https://ssss.example.com"
}
```


---

## Responses  
- **IM_A_TEAPOT**: The user is not logged in.  
- **FORBIDDEN**: The user does not have admin privileges.  
- **OK**: The settings were successfully updated.  

---

## Notes  
- This endpoint requires admin access.  
- Ensure all fields are provided in the request payload.  

[BACK](main-docs.md)
