# Get Items API

## Endpoint: `/api/ssss/get-items`

### Method
**POST**

### Description
Retrieve the list of items (files and folders) from a specified folder ID and path. This API requires the user to be authenticated.

### Payload
```json
{
    "id": "FOLDER_ID",
    "path": "subfolder"  // Can be empty
}
```

The request payload must include the following fields:
- **id**: The ID of the folder to retrieve items from.  
- **path**: The relative path within the folder (use an empty string for the root directory).

### Responses
```json
[
    {
        "id": "FOLDER_ID",
        "name": "example_file.txt",
        "path": "subfolder/example_file.txt",
        "is_file": true
    },
    {   
        "id": "FOLDER_ID",
        "name": "example_folder",
        "path": "subfolder/example_folder",
        "is_file": false
    }
]
```

#### Success
**Status: OK**  
Returns an array of items in the specified folder. Each item includes:
- **id**: The id of the root folder.
- **name**: The name of the item.  
- **path**: The relative path of the item.  
- **is_file**: A boolean indicating whether the item is a file or folder.

#### Error Responses
- **Status: IM_A_TEAPOT**  
  **Message:** User not logged in.  

- **Status: SERVICE_UNAVAILABLE**  
  **Message:** Server Error.  

- **Status: UNAVAILABLE_FOR_LEGAL_REASONS**  
  **Message:** No Syncthing folders.  

### Notes
- Authentication is required for this endpoint.
- Ensure the `id` field corresponds to a valid folder.
- If the `path` field is invalid, no items will be returned.

[BACK](main-docs.md)
