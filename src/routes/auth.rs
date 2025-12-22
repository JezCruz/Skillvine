use crate::services::{email::send_verification_email, token::generate_verification_token};
use actix_web::{web, Error, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;

#[derive(Deserialize)]
pub struct RegisterForm {
    pub email: String,
}

pub async fn register_user(
    form: web::Json<RegisterForm>,
    db: web::Data<PgPool>,
) -> Result<HttpResponse, Error> {
    let token = generate_verification_token();

    // Insert user with verification token (use runtime query to avoid compile-time SQLx macros)
    let result = sqlx::query(
        "INSERT INTO users (email, verification_token, verified) VALUES ($1, $2, false)",
    )
    .bind(&form.email)
    .bind(&token)
    .execute(db.get_ref())
    .await;

    if let Err(e) = result {
        eprintln!("Database insert error: {:?}", e);
        return Ok(HttpResponse::InternalServerError().body("Database error"));
    }

    // Send verification email
    if let Err(err) = send_verification_email(&form.email, &token) {
        eprintln!("Error sending email: {:?}", err);
        return Ok(HttpResponse::InternalServerError().body("Failed to send email"));
    }

    Ok(HttpResponse::Ok().body("Verification email sent!"))
}
