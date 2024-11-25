use serde::{Deserialize, Serialize};
// use axum_session::{Session, SessionConfig, SessionNullPool, SessionLayer, SessionSqlitePool, SessionStore};
// use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use tower_sessions::Session;
use uuid::Uuid;
use http::{request::Parts, StatusCode};
use axum::async_trait;
use axum::extract::FromRequestParts;
// use axum_core::extract::FromRequestParts;
// use tower_sessions::tower_sessions_core::session::Session;
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuestData {
  pub id: Uuid,
  pub username: String,
  pub logged_in: bool,
  pub role: u8,
}
impl Default for GuestData {
  fn default() -> Self {
    Self {
      id: Uuid::new_v4(),
      username: "".to_string(),
      logged_in: false,
      role: 0,
    }
  }
}
#[derive(Clone, Debug)]
pub struct Guest {
  pub session: Session,
  pub guest_data: GuestData,
}
impl Guest {
  const GUEST_DATA_KEY: &'static str = "guest_data";
  pub fn _id(&self) -> Uuid {
    self.guest_data.id
  }
  pub fn _username(&self) -> String {
    self.guest_data.username.clone()
  }
  pub fn logout(session: &Session) -> bool {
    let value: Result<Option<GuestData>, _> = session.remove(Self::GUEST_DATA_KEY);
    if let Ok(Some(_)) = value {
      true
    } else {
      false
    }
  }
  pub fn update_session(session: &Session, guest_data: &GuestData) {
    session
      .insert(Self::GUEST_DATA_KEY, guest_data.clone())
      .expect("infallible")
  }
}
#[async_trait]
impl<S> FromRequestParts<S> for Guest
where
  S: Send + Sync,
{
  type Rejection = (StatusCode, &'static str);
  async fn from_request_parts(req: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
    let session = Session::from_request_parts(req, state).await?;
    let guest_data: GuestData = session
      .get(Self::GUEST_DATA_KEY)
      .expect("infallible")
      .unwrap_or_default();
    Self::update_session(&session, &guest_data);
    Ok(Self {
      session,
      guest_data,
    })
  }
}