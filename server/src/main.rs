#![feature(custom_derive, decl_macro, plugin)]
#![plugin(rocket_codegen)]
#![recursion_limit="128"]

extern crate bcrypt;
extern crate chrono;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_codegen;
extern crate dotenv;
#[macro_use]
extern crate error_chain;
extern crate jsonwebtoken as jwt;
extern crate rocket;
extern crate rocket_contrib;
extern crate r2d2_diesel;
extern crate r2d2;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

mod auth;
mod conn;
mod db;
mod errors;
mod schema;
mod models;

use auth::Session;
use conn::DbConn;
use dotenv::dotenv;
use rocket::http::Cookies;
use rocket::request::Form;
use rocket::response::Redirect;
use rocket::response::status::Created;
use rocket_contrib::Json;
use std::convert::From;

// register
#[derive(FromForm)]
struct RegisterRequest {
  email: String,
  username: String,
  password: String,
}

#[post("/register", data = "<form>")]
fn register(mut cookies: Cookies,
            conn: DbConn,
            form: Form<RegisterRequest>)
            -> errors::Result<Redirect> {
  let form: RegisterRequest = form.into_inner();

  let existing_users = db::find_users_with_email(&*conn, &form.email)?;
  if !existing_users.is_empty() {
    bail!(errors::ErrorKind::EmailAlreadyRegistered(form.email));
  }

  let user = db::create_user(&*conn, &form.email, &form.username, &form.password)?;
  auth::add_user_cookie(&mut cookies, &user);
  Ok(Redirect::to("/"))
}

// login
#[derive(Deserialize)]
struct LoginRequest {
  username: String,
  password: String,
}

#[derive(Serialize)]
struct LoginResponse {
  id: i32,
  email: String,
  username: String
}

#[post("/login", format="application/json", data = "<form>")]
fn login(mut cookies: Cookies, conn: DbConn, form: Json<LoginRequest>) -> errors::Result<Json<LoginResponse>> {
  let user = db::find_user(&*conn, &form.username, &form.password)?;
  auth::add_user_cookie(&mut cookies, &user);
  Ok(Json(LoginResponse {
    id: user.id,
    email: user.email,
    username: user.username
  }))
}

// logout
#[post("/logout")]
fn logout(mut cookies: Cookies) -> Redirect {
  auth::remove_user_cookie(&mut cookies);
  Redirect::to("/")
}

// stuff
#[derive(Debug, Serialize)]
struct Routine {
  id: usize,
  name: String,
  exercises: Vec<Exercise>,
}

impl From<(models::Routine, Vec<models::Exercise>)> for Routine {
  fn from((model, exercise_models): (models::Routine, Vec<models::Exercise>)) -> Self {
    let mut exercises = Vec::new();
    for exercise_model in exercise_models {
      exercises.push(Exercise::from(exercise_model))
    }

    Routine {
      id: model.id as usize,
      name: model.name,
      exercises: exercises,
    }
  }
}

#[derive(Debug, Serialize)]
struct Exercise {
  id: usize,
  name: String,
  sets: usize,
  reps: usize,
}

impl From<models::Exercise> for Exercise {
  fn from(model: models::Exercise) -> Exercise {
    Exercise {
      id: model.id as usize,
      name: model.name,
      sets: model.sets as usize,
      reps: model.reps as usize,
    }
  }
}

#[get("/routines", format = "application/json")]
fn list_routines(conn: DbConn) -> errors::Result<Json<Vec<Routine>>> {
  let routines = db::find_routines(&*conn)?;

  let mut result = Vec::new();
  for model in routines {
    result.push(Routine::from(model))
  }
  Ok(Json(result))
}

#[derive(Debug, Serialize, Deserialize)]
struct NewWorkout {
  routine_id: i32,
}

#[post("/workouts", format = "application/json", data = "<new_workout>")]
fn start_workout(session: Session,
                 conn: DbConn,
                 new_workout: Json<NewWorkout>)
                 -> errors::Result<Created<()>> {
  let workout = db::create_workout(&*conn, session.user_id, new_workout.routine_id)?;
  Ok(Created(format!("/workouts/{}", workout.id), None))
}

fn run() -> errors::Result<()> {

  dotenv()?;

  let pool = conn::pool()?;

  rocket::ignite()
    .manage(pool)
    .mount("/api", routes![register, login, logout, list_routines])
    .mount("/api/my", routes![start_workout])
    .launch();

  Ok(())
}

quick_main!(run);
