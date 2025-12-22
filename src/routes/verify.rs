use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;
use sqlx::PgPool;
use sqlx::Row;

#[derive(Deserialize)]
pub struct VerifyQuery {
    pub token: String,
}

// You pass the DB pool via web::Data
pub async fn verify_email(
    pool: web::Data<PgPool>,
    query: web::Query<VerifyQuery>,
) -> impl Responder {
    // 1. Look up the user by token (runtime query to avoid sqlx compile-time macros requirement)
    let result = sqlx::query(
        "SELECT id, email, verified FROM users WHERE verification_token = $1",
    )
    .bind(query.token.as_str())
    .fetch_optional(pool.get_ref())
    .await;

    match result {
        Ok(Some(row)) => {
            let id: i32 = row.get("id");
            let verified: Option<bool> = row.get("verified");

            if verified.unwrap_or(false) {
                return HttpResponse::Ok()
                    .content_type("text/html")
                    .body("<h3>Email already verified!</h3>");
            }

            // 2. Mark the user as verified
            let update = sqlx::query(
                "UPDATE users SET verified = true, verification_token = NULL WHERE id = $1",
            )
            .bind(id)
            .execute(pool.get_ref())
            .await;

            match update {
                Ok(_) => HttpResponse::Ok()
                    .content_type("text/html")
                    .body("<h3>Email successfully verified!</h3>"),
                Err(e) => {
                    eprintln!("DB update error: {:?}", e);
                    HttpResponse::InternalServerError().body("Error verifying email")
                }
            }
        }
        Ok(None) => {
            HttpResponse::BadRequest().body("<h3>Invalid or expired verification token</h3>")
        }
        Err(e) => {
            eprintln!("DB query error: {:?}", e);
            HttpResponse::InternalServerError().body("Database error")
        }
    }
}
