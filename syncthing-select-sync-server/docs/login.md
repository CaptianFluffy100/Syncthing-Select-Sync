# Login Process for Syncthing Select Sync

`/api/login`

The login process requires a **username** and **password**. Based on the provided credentials and server availability, the server will return different HTTP status codes to indicate success or failure.

## Requirements
- **METHOD:**
   `POST`
- **Request Payload:**  
  The client must send the login information in the request body as a JSON object. Example:  
  `{ "username": "your_username", "password": "your_password" }`

## **Login Request**

To initiate the login process, the client must send a **POST** request to the login endpoint with the **username** and **password** in the request body.

### **Successful Login**

- **Status Code:** `200 OK`
- **Description:** If the username and password match the server's records, the server will respond with a `200 OK` status, indicating a successful login and the start of a session.

### **Failed Login**

If the login attempt fails, the server will return an appropriate error status code:

1. **IM_A_TEAPOT**  
   - **Status Code:** `418 I'm a teapot`
   - **Description:** This status code indicates that the provided **username** does not exist on the server. The client should check if the username is correct and try again.

2. **SERVICE_UNAVAILABLE**  
   - **Status Code:** `503 Service Unavailable`
   - **Description:** This status code is returned when the server cannot process the login request because a required file is missing or unavailable. It indicates that the server is temporarily unable to fulfill the request, often due to missing resources.

## **Error Handling**

When handling failed login attempts, the client should be prepared to:

- **Handle `418 I'm a teapot`:**  
   Inform the user that the username or password is not found/correct and prompt them to verify the username and try again.

- **Handle `503 Service Unavailable`:**  
   Inform the user that there is a temporary issue with the server (such as a missing file), and suggest they try logging in again later.

## **Summary**

The login process in Syncthing Select Sync involves sending the username and password for authentication. The client should expect the following possible responses:

- **`200 OK`**: Successful login.
- **`418 I'm a teapot`**: User does not exist.
- **`503 Service Unavailable`**: Server unable to process the login due to missing files.

By handling these responses, the client can provide a better user experience during the login process.

[BACK](main-docs.md)
