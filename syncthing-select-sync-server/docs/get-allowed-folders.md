# User Allowed Folders API

## Endpoint: `/api/user-allowed-folders`

### Method
**GET**

### Description
This endpoint retrieves the list of allowed folders for the currently logged-in user. It requires the user to be authenticated.

### Responses
- **Status: IM_A_TEAPOT**  
  Returned when the user is not logged in.  

  **Message:**  
  `User not logged in`

- **Status: OK**  
  Returned when the user is logged in successfully. The response contains an array of allowed folders.

### Response Format
The response for a successful request is an array of folder objects:

```json
[
    {
        "folder": "example-folder-id"
    },
    {
        "folder": "another-folder-id"
    }
]
```

### Notes
- If the user is not logged in, no data will be returned, and an error message will be provided.

- Authentication is required to access this endpoint.  

[BACK](main-docs.md)
