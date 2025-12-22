use actix_session::Session;
use actix_multipart::Multipart;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use futures_util::StreamExt;
use tokio::fs::File as TokioFile;
use tokio::io::AsyncWriteExt;
use sqlx::Row;
use uuid::Uuid;
use chrono::Utc;
use std::fs;

use crate::POOL_DATA;
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use password_hash::SaltString;
use rand_core::OsRng;

#[get("/manage_profile")]
async fn manage_profile_page(_req: HttpRequest, _session: Session) -> actix_web::Result<actix_files::NamedFile> {
    let f = actix_files::NamedFile::open_async("./templates/profile_manager.html").await?;
    Ok(f)
}

#[get("/api/profile")]
async fn api_get_profile(session: Session) -> impl Responder {
    let user_id = match session.get::<i32>("user_id").unwrap_or(None) {
        Some(id) => id,
        None => return HttpResponse::Unauthorized().json(serde_json::json!({"error":"not logged in"})),
    };

    if let Some(pool_data) = POOL_DATA.get() {
        let pool = pool_data.get_ref();
        let row = sqlx::query("SELECT id, full_name, email, role, verified FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await;

        match row {
            Ok(Some(r)) => {
                let full_name: String = r.get("full_name");
                let email: String = r.get("email");
                let role: Option<String> = r.get("role");
                let email_verified: bool = r.get::<bool, _>("verified");

                // check teacher verification
                let trow = sqlx::query("SELECT status, id_path, created_at FROM teacher_verifications WHERE user_id = $1 ORDER BY created_at DESC LIMIT 1")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await
                    .ok()
                    .flatten();

                let teacher_verification = if let Some(tr) = trow {
                    serde_json::json!({
                        "status": tr.get::<String, _>("status"),
                        "id_path": tr.try_get::<String, _>("id_path").unwrap_or_default(),
                    })
                } else {
                    serde_json::Value::Null
                };

                return HttpResponse::Ok().json(serde_json::json!({
                    "id": user_id,
                    "full_name": full_name,
                    "email": email,
                    "role": role.unwrap_or_else(|| "student".to_string()),
                    "email_verified": email_verified,
                    "teacher_verification": teacher_verification,
                    "view_as": session.get::<String>("view_as").unwrap_or(None)
                }));
            }
            Ok(None) => return HttpResponse::NotFound().json(serde_json::json!({"error":"user not found"})),
            Err(e) => return HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("db: {}", e)})),
        }
    }

    HttpResponse::InternalServerError().json(serde_json::json!({"error":"no db"}))
}

#[post("/api/change_name")]
async fn api_change_name(session: Session, params: web::Json<serde_json::Value>) -> impl Responder {
    let user_id = match session.get::<i32>("user_id").unwrap_or(None) {
        Some(id) => id,
        None => return HttpResponse::Unauthorized().json(serde_json::json!({"error":"not logged in"})),
    };
    let new_name = params.get("full_name").and_then(|v| v.as_str()).unwrap_or("").trim();
    if new_name.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({"error":"name required"}));
    }

    if let Some(pool_data) = POOL_DATA.get() {
        let pool = pool_data.get_ref();
        let res = sqlx::query("UPDATE users SET full_name = $1 WHERE id = $2")
            .bind(new_name)
            .bind(user_id)
            .execute(pool)
            .await;
        match res {
            Ok(_) => return HttpResponse::Ok().json(serde_json::json!({"ok":true})),
            Err(e) => return HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("db: {}", e)})),
        }
    }

    HttpResponse::InternalServerError().json(serde_json::json!({"error":"no db"}))
}

#[post("/api/change_password")]
async fn api_change_password(session: Session, params: web::Json<serde_json::Value>) -> impl Responder {
    let user_id = match session.get::<i32>("user_id").unwrap_or(None) {
        Some(id) => id,
        None => return HttpResponse::Unauthorized().json(serde_json::json!({"error":"not logged in"})),
    };
    let old = params.get("old_password").and_then(|v| v.as_str()).unwrap_or("");
    let new = params.get("new_password").and_then(|v| v.as_str()).unwrap_or("");
    if old.is_empty() || new.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({"error":"old and new required"}));
    }

    if let Some(pool_data) = POOL_DATA.get() {
        let pool = pool_data.get_ref();
        let row = sqlx::query("SELECT password_hash FROM users WHERE id = $1").bind(user_id).fetch_one(pool).await;
        match row {
            Ok(r) => {
                let hash: String = r.get("password_hash");
                let parsed = password_hash::PasswordHash::new(&hash).map_err(|e| e.to_string());
                if parsed.is_err() {
                    return HttpResponse::InternalServerError().json(serde_json::json!({"error":"invalid password hash stored"}));
                }
                let parsed = parsed.unwrap();
                if Argon2::default().verify_password(old.as_bytes(), &parsed).is_err() {
                    return HttpResponse::Unauthorized().json(serde_json::json!({"error":"old password mismatch"}));
                }

                // hash new
                let salt = SaltString::generate(&mut OsRng);
                let hashed = Argon2::default().hash_password(new.as_bytes(), &salt).map(|h| h.to_string()).map_err(|e| e.to_string());
                if hashed.is_err() { return HttpResponse::InternalServerError().json(serde_json::json!({"error":"hash failed"})); }
                let new_hash = hashed.unwrap();
                let upd = sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2").bind(new_hash).bind(user_id).execute(pool).await;
                match upd {
                    Ok(_) => return HttpResponse::Ok().json(serde_json::json!({"ok":true})),
                    Err(e) => return HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("db: {}", e)})),
                }
            }
            Err(e) => return HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("db: {}", e)})),
        }
    }

    HttpResponse::InternalServerError().json(serde_json::json!({"error":"no db"}))
}

// Accept multipart upload for teacher ID
#[post("/api/upload_id")]
async fn api_upload_id(session: Session, mut payload: Multipart) -> impl Responder {
    let user_id = match session.get::<i32>("user_id").unwrap_or(None) {
        Some(id) => id,
        None => return HttpResponse::Unauthorized().json(serde_json::json!({"error":"not logged in"})),
    };

    // ensure uploads dir
    let upload_dir = std::path::Path::new("uploads/teacher_ids");
    if !upload_dir.exists() { let _ = fs::create_dir_all(upload_dir); }

    // create teacher_verifications table if not exists
    if let Some(pool_data) = POOL_DATA.get() {
        let pool = pool_data.get_ref();
        let _ = sqlx::query("CREATE TABLE IF NOT EXISTS teacher_verifications (id SERIAL PRIMARY KEY, user_id INTEGER NOT NULL, status VARCHAR(32) NOT NULL, id_path TEXT, created_at TIMESTAMP DEFAULT now())").execute(pool).await;

        // process multipart - use tokio async file writes
        while let Some(field_res) = payload.next().await {
            let mut field = match field_res {
                Ok(f) => f,
                Err(e) => { eprintln!("multipart field error: {}", e); continue; }
            };
            let filename = {
                let default = format!("upload-{}.dat", Uuid::new_v4());
                match field.headers().get("content-disposition") {
                    Some(hv) => match hv.to_str() {
                        Ok(s) => {
                            if let Some(pos) = s.find("filename=") {
                                let mut rest = &s[pos + "filename=".len()..];
                                // trim possible surrounding quotes
                                rest = rest.trim();
                                rest = rest.trim_matches(|c| c == '"' || c == '\'');
                                if !rest.is_empty() { rest.to_string() } else { default }
                            } else { default }
                        }
                        Err(_) => default,
                    },
                    None => default,
                }
            };
            let ts = Utc::now().timestamp();
            let filepath = upload_dir.join(format!("{}_{}", ts, sanitize_filename::sanitize(&filename)));

            // create file asynchronously
            match TokioFile::create(&filepath).await {
                Ok(mut f) => {
                    while let Some(chunk_res) = field.next().await {
                        match chunk_res {
                            Ok(chunk) => {
                                if let Err(e) = f.write_all(&chunk).await {
                                    eprintln!("write error: {}", e);
                                }
                            }
                            Err(e) => {
                                eprintln!("chunk read error: {}", e);
                                continue;
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("failed to create file {}: {}", filepath.display(), e);
                    continue;
                }
            }

            // insert or update verification record
            let _ = sqlx::query("INSERT INTO teacher_verifications (user_id, status, id_path, created_at) VALUES ($1, $2, $3, now())")
                .bind(user_id)
                .bind("pending")
                .bind(filepath.to_string_lossy().to_string())
                .execute(pool)
                .await;
        }

        return HttpResponse::Ok().json(serde_json::json!({"ok":true}));
    }

    HttpResponse::InternalServerError().json(serde_json::json!({"error":"no db"}))
}

#[post("/api/request_email_verification")]
async fn api_request_email_verification(session: Session) -> impl Responder {
    let user_id = match session.get::<i32>("user_id").unwrap_or(None) {
        Some(id) => id,
        None => return HttpResponse::Unauthorized().json(serde_json::json!({"error":"not logged in"})),
    };

    if let Some(pool_data) = POOL_DATA.get() {
        let pool = pool_data.get_ref();
        let token = Uuid::new_v4().to_string();
        let upd = sqlx::query("UPDATE users SET verification_token = $1, verified = false WHERE id = $2").bind(&token).bind(user_id).execute(pool).await;
        match upd {
            Ok(_) => {
                // TODO: send email via services::email::send_verification_email
                eprintln!("Email verification token for user {}: {}", user_id, token);
                return HttpResponse::Ok().json(serde_json::json!({"ok":true}));
            }
            Err(e) => return HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("db: {}", e)})),
        }
    }
    HttpResponse::InternalServerError().json(serde_json::json!({"error":"no db"}))
}

#[post("/api/view_as")]
async fn api_view_as(session: Session, params: web::Json<serde_json::Value>) -> impl Responder {
    let user_id = match session.get::<i32>("user_id").unwrap_or(None) {
        Some(id) => id,
        None => return HttpResponse::Unauthorized().json(serde_json::json!({"error":"not logged in"})),
    };
    let view_as = params.get("view_as").and_then(|v| v.as_str()).unwrap_or("");
    if view_as != "teacher" && view_as != "student" { return HttpResponse::BadRequest().json(serde_json::json!({"error":"invalid role"})); }

    // if save flag is true, update DB
    let save = params.get("save").and_then(|v| v.as_bool()).unwrap_or(false);
    if save {
        if let Some(pool_data) = POOL_DATA.get() {
            let pool = pool_data.get_ref();
            let upd = sqlx::query("UPDATE users SET role = $1 WHERE id = $2").bind(view_as).bind(user_id).execute(pool).await;
            match upd {
                Ok(_) => return HttpResponse::Ok().json(serde_json::json!({"ok":true})),
                Err(e) => return HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("db: {}", e)})),
            }
        }
        return HttpResponse::InternalServerError().json(serde_json::json!({"error":"no db"}));
    }

    // otherwise store temporary view in session
    let _ = session.insert("view_as", view_as.to_string());
    HttpResponse::Ok().json(serde_json::json!({"ok":true}))
}

pub fn init(cfg: &mut web::ServiceConfig) {
    cfg.service(manage_profile_page)
        .service(api_get_profile)
        .service(api_change_name)
        .service(api_change_password)
        .service(api_upload_id)
        .service(api_request_email_verification)
        .service(api_view_as);
}
