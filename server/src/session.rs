use errors;
use redis::{Commands, Connection};
use redis_conn::RedisConn;
use rocket::Request;
use rocket::http::{Cookie, Cookies, Status};
use rocket::outcome::IntoOutcome;
use rocket::request::{FromRequest, Outcome};
use serde_json;

fn get_redis_key(session_id: i32) -> String {
  format!("session_{}", session_id)
}

#[derive(Debug)]
pub struct SessionToken {
  id: i32,
}

impl SessionToken {
  pub fn get(self, conn: &Connection) -> errors::Result<Session> {
    let serialized: String = conn.get(get_redis_key(self.id))?;
    let session: Session = serde_json::from_str(&serialized)?;
    Ok(session)
  }
}

impl<'a, 'r> FromRequest<'a, 'r> for SessionToken {
  type Error = ();

  fn from_request(request: &'a Request) -> Outcome<Self, ()> {
    request.cookies()
      .get_private("session")
      .ok_or(())
      .and_then(|c| c.value().parse::<i32>().map_err(|_| ()))
      .map(|id| SessionToken { id: id })
      .into_outcome(Status::BadRequest)
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
  pub id: i32,
  pub user_id: i32,
}

impl Session {
  pub fn persist(conn: &Connection, user_id: i32) -> errors::Result<Session> {
    let id: i32 = conn.incr("session_id", 1)?;

    let session = Session {
      id: id,
      user_id: user_id,
    };
    let serialized: String = serde_json::to_string(&session)?;

    let expiry = 15 * 50;

    let _: () = conn.set_ex(format!("session_{}", id), serialized, expiry)?;
    Ok(session)
  }

  pub fn add_cookie(&self, cookies: &mut Cookies) {
    cookies.add_private(Cookie::new("session", self.id.to_string()));
  }

  pub fn remove_cookie(cookies: &mut Cookies) {
    cookies.remove_private(Cookie::named("session"));
  }

  pub fn extend(&self, conn: &Connection) -> errors::Result<()> {
    let key = get_redis_key(self.id);
    let expiry = 15 * 50;
    let _: () = conn.expire(key, expiry)?;
    Ok(())
  }
}

impl<'a, 'r> FromRequest<'a, 'r> for Session {
  type Error = ();

  fn from_request(request: &'a Request) -> Outcome<Self, ()> {
    let conn = RedisConn::from_request(request)?;
    let token = SessionToken::from_request(request)?;

    token.get(&conn)
      .map_err(|_| ())
      .into_outcome(Status::InternalServerError)
  }
}
