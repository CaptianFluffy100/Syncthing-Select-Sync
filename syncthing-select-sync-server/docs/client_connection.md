# **Cookie Management for Syncthing Select Sync**

In Syncthing Select Sync, the server creates a session cookie for the client when the session starts. Each time the client's session ends, a new session cookie is issued. To ensure the client can maintain persistent sessions, cookies must be enabled.

## **How Cookie Management Works**

1. **Session Cookie Creation:**  
   When the client connects, the server generates a session cookie. This cookie is used to maintain the session throughout interactions with the server.

2. **New Cookie on Session End:**  
   When the client's session ends (e.g., the client disconnects or logs out), the server generates a new cookie upon the next connection. This helps ensure that the client starts a fresh session each time.

3. **Client Cookie Requirements:**  
   To work properly with Syncthing Select Sync, the client **must have cookies enabled**. This allows the client to receive, store, and send session cookies with each request, ensuring uninterrupted session continuity.

## **Key Considerations**

- **Session Persistence:**  
   Cookies ensure that the session remains active across multiple interactions, so the client can continue syncing data without needing to reauthenticate after each request.

- **Automatic Cookie Handling:**  
   With cookies enabled, the client will automatically send the stored session cookie with each request, so the client doesn’t need to manually manage cookies.

- **Security:**  
   Ensure that cookies are transmitted over secure channels (e.g., `https://`) to protect sensitive session information.

- **Cookie Expiration:**  
   Session cookies may have an expiration time defined by the server. The client will automatically handle cookie expiration and renewal when needed.

## **Conclusion**

To use Syncthing Select Sync effectively, make sure your HTTP client has cookie management enabled. This ensures that session cookies are correctly stored and sent with each request, maintaining session continuity throughout the client-server interaction.

[BACK](main-docs.md)
