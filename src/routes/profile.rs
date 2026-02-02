use actix_session::Session;
use actix_multipart::Multipart;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use actix_web::http::header::HeaderValue;
use actix_files::NamedFile;
use futures_util::StreamExt;
use tokio::fs::File as TokioFile;
use tokio::io::AsyncWriteExt;
use sqlx::Row;
use uuid::Uuid;
use chrono::{Utc, NaiveDate};
use std::fs;
use std::path::PathBuf;

use crate::POOL_DATA;
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use password_hash::SaltString;
use rand_core::OsRng;
use sanitize_filename::sanitize;

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
        let row = sqlx::query("SELECT id, full_name, email, role, verified, birthday, avatar_path FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await;

        match row {
            Ok(Some(r)) => {
                let full_name: String = r.get("full_name");
                let email: String = r.get("email");
                let role: Option<String> = r.get("role");
                let email_verified: bool = r.get::<bool, _>("verified");
                let birthday: Option<chrono::NaiveDate> = r.try_get("birthday").ok();
                let avatar_path: Option<String> = r.try_get("avatar_path").ok();

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
                    "birthday": birthday.map(|d| d.to_string()),
                    "avatar_path": avatar_path,
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

// Serve current user's avatar securely (auth required)
#[get("/api/avatar")]
async fn api_avatar(session: Session, req: HttpRequest) -> actix_web::Result<HttpResponse> {
    let user_id = session.get::<i32>("user_id").unwrap_or(None).ok_or_else(|| actix_web::error::ErrorUnauthorized("not logged in"))?;
    let default_path = PathBuf::from("static/images/default-avatar.svg");

    if let Some(pool_data) = POOL_DATA.get() {
        let pool = pool_data.get_ref();
        if let Ok(Some(row)) = sqlx::query("SELECT avatar_path FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await
        {
            let path: Option<String> = row.try_get("avatar_path").ok();
            if let Some(p) = path {
                let normalized = p.replace('\\', "/");
                let fs_path = PathBuf::from(&normalized);
                if fs_path.exists() {
                    let file = NamedFile::open_async(fs_path).await?;
                    let mut resp = file.use_last_modified(false).into_response(&req);
                    resp.headers_mut().insert(actix_web::http::header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
                    return Ok(resp);
                }
            }
        }
    }
    let file = NamedFile::open_async(default_path).await?;
    let mut resp = file.use_last_modified(false).into_response(&req);
    resp.headers_mut().insert(actix_web::http::header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(resp)
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

// Update basic profile fields (full_name, birthday)
#[post("/api/update_profile")]
async fn api_update_profile(session: Session, params: web::Json<serde_json::Value>) -> impl Responder {
    let user_id = match session.get::<i32>("user_id").unwrap_or(None) {
        Some(id) => id,
        None => return HttpResponse::Unauthorized().json(serde_json::json!({"error":"not logged in"})),
    };

    let new_name = params.get("full_name").and_then(|v| v.as_str()).map(|s| s.trim()).unwrap_or("");
    let birthday_str = params.get("birthday").and_then(|v| v.as_str()).map(|s| s.trim()).unwrap_or("");

    let name_opt = if new_name.is_empty() { None } else { Some(new_name.to_string()) };
    let birthday_opt = if birthday_str.is_empty() {
        None
    } else {
        match NaiveDate::parse_from_str(birthday_str, "%Y-%m-%d") {
            Ok(d) => Some(d),
            Err(_) => return HttpResponse::BadRequest().json(serde_json::json!({"error":"invalid birthday format (use YYYY-MM-DD)"})),
        }
    };

    if name_opt.is_none() && birthday_opt.is_none() {
        return HttpResponse::BadRequest().json(serde_json::json!({"error":"nothing to update"}));
    }

    if let Some(pool_data) = POOL_DATA.get() {
        let res = sqlx::query(
            "UPDATE users SET full_name = COALESCE($1, full_name), birthday = COALESCE($2, birthday) WHERE id = $3",
        )
        .bind(name_opt)
        .bind(birthday_opt)
        .bind(user_id)
        .execute(pool_data.get_ref())
        .await;

        match res {
            Ok(_) => HttpResponse::Ok().json(serde_json::json!({"ok":true})),
            Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("db: {}", e)})),
        }
    } else {
        HttpResponse::InternalServerError().json(serde_json::json!({"error":"no db"}))
    }
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
    let upload_dir = std::path::Path::new("private/teacher_ids");
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

// Upload avatar photo
#[post("/api/upload_avatar")]
async fn api_upload_avatar(session: Session, mut payload: Multipart) -> impl Responder {
    let user_id = match session.get::<i32>("user_id").unwrap_or(None) {
        Some(id) => id,
        None => return HttpResponse::Unauthorized().json(serde_json::json!({"error":"not logged in"})),
    };

    let upload_dir = std::path::Path::new("uploads/profile_pics");
    if !upload_dir.exists() {
        let _ = fs::create_dir_all(upload_dir);
    }

    if let Some(pool_data) = POOL_DATA.get() {
        let pool = pool_data.get_ref();

        let mut saved_path: Option<String> = None;

        while let Some(field_res) = payload.next().await {
            let mut field = match field_res {
                Ok(f) => f,
                Err(e) => { eprintln!("multipart field error: {}", e); continue; }
            };

            // Require image content type
            let ct = field.content_type();
            if !ct.type_().as_str().eq_ignore_ascii_case("image") {
                return HttpResponse::BadRequest().json(serde_json::json!({"error":"only image uploads allowed"}));
            }

            let ext = ct.subtype().as_str();
            let safe_ext = if ext.is_empty() { "dat" } else { ext };

            // basic size guard (~5MB)
            let mut total_bytes: usize = 0;
            let ts = Utc::now().timestamp();
            let fname = format!("avatar-{}-{}-{}.{}", user_id, ts, Uuid::new_v4(), safe_ext);
            let filepath = upload_dir.join(&fname);

            match TokioFile::create(&filepath).await {
                Ok(mut f) => {
                    while let Some(chunk_res) = field.next().await {
                        match chunk_res {
                            Ok(chunk) => {
                                total_bytes += chunk.len();
                                if total_bytes > 5 * 1024 * 1024 { // 5MB limit
                                    eprintln!("avatar upload too large");
                                    let _ = tokio::fs::remove_file(&filepath).await;
                                    return HttpResponse::BadRequest().json(serde_json::json!({"error":"file too large"}));
                                }
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

            saved_path = Some(filepath.to_string_lossy().to_string());
            break; // handle one file only
        }

        if let Some(path) = saved_path {
            let res = sqlx::query("UPDATE users SET avatar_path = $1 WHERE id = $2")
                .bind(&path)
                .bind(user_id)
                .execute(pool)
                .await;
            match res {
                Ok(_) => return HttpResponse::Ok().json(serde_json::json!({"ok":true, "avatar_path": path})),
                Err(e) => return HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("db: {}", e)})),
            }
        }
        return HttpResponse::BadRequest().json(serde_json::json!({"error":"no file uploaded"}));
    }

    HttpResponse::InternalServerError().json(serde_json::json!({"error":"no db"}))
}

// Submit teacher verification with KYC details and front/back IDs
#[post("/api/teacher_verify_submit")]
async fn api_teacher_verify_submit(session: Session, mut payload: Multipart) -> impl Responder {
    let user_id = match session.get::<i32>("user_id").unwrap_or(None) {
        Some(id) => id,
        None => return HttpResponse::Unauthorized().json(serde_json::json!({"error":"not logged in"})),
    };

    // ensure uploads dir
    let upload_dir = std::path::Path::new("uploads/teacher_ids");
    if !upload_dir.exists() { let _ = fs::create_dir_all(upload_dir); }

    let mut full_name: Option<String> = None;
    let mut dob: Option<NaiveDate> = None;
    let mut gender: Option<String> = None;
    let mut address: Option<String> = None;
    let mut front_path: Option<String> = None;
    let mut back_path: Option<String> = None;

    while let Some(field_res) = payload.next().await {
        let mut field = match field_res {
            Ok(f) => f,
            Err(e) => { eprintln!("multipart field error: {}", e); continue; }
        };

            let name = field.name().to_string();

        // handle text fields streamed as a single chunk
        if name == "full_name" || name == "gender" || name == "address" || name == "dob" {
            let mut data = Vec::new();
            while let Some(chunk_res) = field.next().await {
                match chunk_res {
                    Ok(chunk) => data.extend_from_slice(&chunk),
                    Err(e) => { eprintln!("chunk read error: {}", e); continue; }
                }
            }
            let s = String::from_utf8_lossy(&data).trim().to_string();
            match name.as_str() {
                "full_name" => full_name = Some(s),
                "gender" => gender = Some(s),
                "address" => address = Some(s),
                "dob" => {
                    if s.is_empty() { dob = None; }
                    else {
                        match NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
                            Ok(d) => dob = Some(d),
                            Err(_) => return HttpResponse::BadRequest().json(serde_json::json!({"error":"invalid dob format"})),
                        }
                    }
                }
                _ => {}
            }
            continue;
        }

        // file fields: front_id, back_id
        if name == "front_id" || name == "back_id" {
            let ct = field.content_type();
            if !ct.type_().as_str().eq_ignore_ascii_case("image") {
                return HttpResponse::BadRequest().json(serde_json::json!({"error":"only image uploads allowed"}));
            }
            let ts = Utc::now().timestamp();
            let fname = format!("{}-{}-{}.dat", name, user_id, ts);
            let filepath = upload_dir.join(&fname);
            let mut total_bytes: usize = 0;

            match TokioFile::create(&filepath).await {
                Ok(mut f) => {
                    while let Some(chunk_res) = field.next().await {
                        match chunk_res {
                            Ok(chunk) => {
                                total_bytes += chunk.len();
                                if total_bytes > 25 * 1024 * 1024 { // 25MB cap
                                    let _ = tokio::fs::remove_file(&filepath).await;
                                    return HttpResponse::BadRequest().json(serde_json::json!({"error":"file too large (max 25MB)"}));
                                }
                                if let Err(e) = f.write_all(&chunk).await {
                                    eprintln!("write error: {}", e);
                                }
                            }
                            Err(e) => { eprintln!("chunk read error: {}", e); continue; }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("failed to create file {}: {}", filepath.display(), e);
                    continue;
                }
            }

            let rel_path = filepath.to_string_lossy().to_string();
            if name == "front_id" { front_path = Some(rel_path.clone()); }
            if name == "back_id" { back_path = Some(rel_path); }
        }
    }

    // validations
    if full_name.as_deref().unwrap_or("").is_empty() || dob.is_none() || gender.as_deref().unwrap_or("").is_empty() || address.as_deref().unwrap_or("").is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({"error":"all fields are required"}));
    }
    if front_path.is_none() || back_path.is_none() {
        return HttpResponse::BadRequest().json(serde_json::json!({"error":"front and back ID required"}));
    }

    if let Some(pool_data) = POOL_DATA.get() {
        let pool = pool_data.get_ref();
        let res = sqlx::query("INSERT INTO teacher_verifications (user_id, status, id_path, full_name, dob, gender, address, front_id_path, back_id_path, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, now())")
            .bind(user_id)
            .bind("pending")
            .bind(front_path.as_ref().unwrap())
            .bind(full_name.as_ref().unwrap())
            .bind(dob.unwrap())
            .bind(gender.as_ref().unwrap())
            .bind(address.as_ref().unwrap())
            .bind(front_path.as_ref().unwrap())
            .bind(back_path.as_ref().unwrap())
            .execute(pool)
            .await;

        match res {
            Ok(_) => return HttpResponse::Ok().json(serde_json::json!({"ok":true})),
            Err(e) => return HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("db: {}", e)})),
        }
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

// Allow user to submit a support/request ticket to admins with optional attachment
#[post("/api/support_request")]
async fn api_support_request(session: Session, mut payload: Multipart) -> impl Responder {
    let user_id = match session.get::<i32>("user_id").unwrap_or(None) {
        Some(id) => id,
        None => return HttpResponse::Unauthorized().json(serde_json::json!({"error":"not logged in"})),
    };

    let role = session.get::<String>("role").unwrap_or(None).unwrap_or_else(|| "student".to_string());

    let pool_data = match POOL_DATA.get() {
        Some(p) => p,
        None => return HttpResponse::InternalServerError().json(serde_json::json!({"error":"no db"})),
    };

    // derive latest kyc status
    let kyc_status = match sqlx::query("SELECT status FROM teacher_verifications WHERE user_id = $1 ORDER BY updated_at DESC NULLS LAST, created_at DESC LIMIT 1")
        .bind(user_id)
        .fetch_optional(pool_data.get_ref())
        .await
    {
        Ok(Some(r)) => r.try_get::<String,_>("status").unwrap_or_else(|_| "unverified".to_string()),
        _ => "unverified".to_string(),
    };

    let mut body_text: Option<String> = None;
    let mut attachment_path: Option<String> = None;
    let upload_dir = std::path::Path::new("uploads/support_requests");
    if !upload_dir.exists() { let _ = fs::create_dir_all(upload_dir); }

    while let Some(field_res) = payload.next().await {
        let mut field = match field_res {
            Ok(f) => f,
            Err(e) => return HttpResponse::BadRequest().json(serde_json::json!({"error": format!("multipart error: {}", e)})),
        };
        let name = field.name().to_string();
        if name == "attachment" {
            let mut total: u64 = 0;
            let original = field.content_disposition().get_filename().map(|v| sanitize(v));
            let safe_name = original.unwrap_or_else(|| format!("attach-{}.bin", Uuid::new_v4()));
            let full_path = upload_dir.join(format!("{}-{}", Uuid::new_v4(), safe_name));
            let mut f = match TokioFile::create(&full_path).await {
                Ok(file) => file,
                Err(e) => return HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("file create: {}", e)})),
            };
            while let Some(chunk) = field.next().await {
                let data = match chunk {
                    Ok(d) => d,
                    Err(e) => return HttpResponse::BadRequest().json(serde_json::json!({"error": format!("upload chunk: {}", e)})),
                };
                total += data.len() as u64;
                if total > 25 * 1024 * 1024 {
                    let _ = fs::remove_file(&full_path);
                    return HttpResponse::BadRequest().json(serde_json::json!({"error": "attachment too large (max 25MB)"}));
                }
                if let Err(e) = f.write_all(&data).await {
                    return HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("write failed: {}", e)}));
                }
            }
            attachment_path = Some(full_path.to_string_lossy().to_string());
        } else {
            let mut bytes = Vec::new();
            while let Some(chunk) = field.next().await {
                let data = match chunk {
                    Ok(d) => d,
                    Err(e) => return HttpResponse::BadRequest().json(serde_json::json!({"error": format!("field read: {}", e)})),
                };
                bytes.extend_from_slice(&data);
            }
            let text = String::from_utf8(bytes).unwrap_or_default();
            if name == "body" || name == "message" {
                body_text = Some(text.trim().to_string());
            }
        }
    }

    let body_val = match body_text {
        Some(b) if !b.is_empty() => b,
        _ => return HttpResponse::BadRequest().json(serde_json::json!({"error": "message required"})),
    };

    let row = sqlx::query("INSERT INTO support_requests (user_id, role, kyc_status, body, attachment_path) VALUES ($1, $2, $3, $4, $5) RETURNING id")
        .bind(user_id)
        .bind(&role)
        .bind(&kyc_status)
        .bind(&body_val)
        .bind(&attachment_path)
        .fetch_one(pool_data.get_ref())
        .await;

    match row {
        Ok(r) => {
            let id: i32 = r.get("id");
            HttpResponse::Ok().json(serde_json::json!({"ok": true, "id": id}))
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({"error": format!("db: {}", e)})),
    }
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
        .service(api_avatar)
    .service(api_update_profile)
        .service(api_change_password)
        .service(api_upload_id)
    .service(api_upload_avatar)
        .service(api_teacher_verify_submit)
        .service(api_request_email_verification)
        .service(api_support_request)
        .service(api_view_as);
}
