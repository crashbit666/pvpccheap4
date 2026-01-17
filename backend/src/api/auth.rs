use crate::{db::DbPool, models::User, schema::users, services::auth};
use actix_web::{HttpResponse, Responder, post, web};
use diesel::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

#[post("/register")]
pub async fn register(pool: web::Data<DbPool>, item: web::Json<AuthRequest>) -> impl Responder {
    let mut conn = pool.get().expect("Couldn't get db connection");

    // Hash password
    let hashed = match auth::hash_password(&item.password) {
        Ok(h) => h,
        Err(_) => return HttpResponse::InternalServerError().body("Error hashing password"),
    };

    // Insert user
    let new_user = diesel::insert_into(users::table)
        .values((
            users::username.eq(&item.username),
            users::password_hash.eq(&hashed),
        ))
        .get_result::<User>(&mut conn);

    match new_user {
        Ok(user) => {
            HttpResponse::Ok().json(serde_json::json!({"id": user.id, "username": user.username}))
        }
        Err(_) => HttpResponse::Conflict().body("Username already exists"),
    }
}

#[post("/login")]
pub async fn login(pool: web::Data<DbPool>, item: web::Json<AuthRequest>) -> impl Responder {
    let mut conn = pool.get().expect("Couldn't get db connection");

    // Find user
    let user_result = users::table
        .filter(users::username.eq(&item.username))
        .first::<User>(&mut conn);

    match user_result {
        Ok(user) => {
            // Verify password
            match auth::verify_password(&item.password, &user.password_hash) {
                Ok(true) => {
                    // Generate JWT
                    match auth::create_jwt(user.id) {
                        Ok(token) => HttpResponse::Ok().json(serde_json::json!({"token": token})),
                        Err(_) => HttpResponse::InternalServerError().body("Error creating token"),
                    }
                }
                _ => HttpResponse::Unauthorized().body("Invalid credentials"),
            }
        }
        Err(_) => HttpResponse::Unauthorized().body("Invalid credentials"),
    }
}
