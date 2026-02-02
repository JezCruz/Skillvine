use actix_files::NamedFile;
use actix_session::Session;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use actix_multipart::Multipart;
use futures_util::StreamExt;
use tokio::fs::File as TokioFile;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;
use std::fs;
use std::path::PathBuf;
use sanitize_filename::sanitize;
use chrono::{NaiveDate, NaiveDateTime, DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use sqlx::Row;
use rand::{distributions::Alphanumeric, Rng};
use rand_core::OsRng;
use argon2::Argon2;
use password_hash::{SaltString, PasswordHasher};

use crate::POOL_DATA;
use crate::routes::notifications::{broadcast_notification, NotificationEvent};

fn ensure_admin(session: &Session) -> Result<i32, HttpResponse> {
    let user_id = session
        .get::<i32>("user_id")
        .unwrap_or(None)
        .ok_or_else(|| HttpResponse::Unauthorized().json(json!({"error": "not logged in"})))?;
    let role = session
        .get::<String>("role")
        .unwrap_or(None)
        .unwrap_or_default();
    // If impersonating, allow admin actions and return original admin id when stored
    if let Some(original) = session.get::<i32>("admin_original_user_id").unwrap_or(None) {
        return Ok(original);
    }
    if role != "admin" {
        return Err(HttpResponse::Forbidden().json(json!({"error": "admin only"})));
    }
    Ok(user_id)
}

#[get("/admin")]
async fn admin_portal(session: Session, req: HttpRequest) -> actix_web::Result<HttpResponse> {
    if let Err(resp) = ensure_admin(&session) {
        return Ok(resp);
    }
    let file = NamedFile::open_async("./templates/admin_dashboard.html").await?;
    Ok(file.into_response(&req))
}

#[derive(Deserialize)]
struct KycQuery {
    status: Option<String>,
    search: Option<String>,
}

#[get("/api/admin/kyc_requests")]
async fn list_kyc(query: web::Query<KycQuery>, session: Session) -> impl Responder {
    if let Err(resp) = ensure_admin(&session) { return resp; }

    let pool_data = match POOL_DATA.get() {
        Some(p) => p,
        None => return HttpResponse::InternalServerError().json(json!({"error": "no db"})),
    };

    let mut sql = "SELECT tv.id, tv.status, tv.created_at, tv.full_name, tv.dob, tv.gender, tv.address, tv.admin_note, u.email, u.full_name AS user_name, tv.front_id_path, tv.back_id_path FROM teacher_verifications tv JOIN users u ON tv.user_id = u.id WHERE 1=1".to_string();
    let mut binds: Vec<String> = Vec::new();
    if let Some(s) = &query.status {
        sql.push_str(" AND tv.status = $1");
        binds.push(s.to_lowercase());
    }
    let mut search_clause = String::new();
    if let Some(search) = &query.search {
        if binds.is_empty() { sql.push_str(" AND (u.email ILIKE $1 OR u.full_name ILIKE $1)"); }
        else { sql.push_str(" AND (u.email ILIKE $2 OR u.full_name ILIKE $2)"); }
        search_clause = format!("%{}%", search);
    }
    sql.push_str(" ORDER BY tv.created_at DESC");

    let rows = match (binds.len(), search_clause.is_empty()) {
        (0, true) => sqlx::query(&sql).fetch_all(pool_data.get_ref()).await,
        (1, true) => sqlx::query(&sql).bind(&binds[0]).fetch_all(pool_data.get_ref()).await,
        (1, false) => sqlx::query(&sql).bind(&binds[0]).bind(&search_clause).fetch_all(pool_data.get_ref()).await,
        (0, false) => sqlx::query(&sql).bind(&search_clause).fetch_all(pool_data.get_ref()).await,
        _ => sqlx::query(&sql).bind(&binds[0]).bind(&search_clause).fetch_all(pool_data.get_ref()).await,
    };

    match rows {
        Ok(list) => {
            let mapped: Vec<serde_json::Value> = list
                .into_iter()
                .map(|r| {
                    let submitted_at = r
                        .try_get::<DateTime<Utc>, _>("created_at")
                        .map(|d| d.to_rfc3339())
                        .or_else(|_| r.try_get::<NaiveDateTime, _>("created_at").map(|d| DateTime::<Utc>::from_naive_utc_and_offset(d, Utc).to_rfc3339()))
                        .unwrap_or_default();
                    json!({
                        "id": r.get::<i32,_>("id"),
                        "status": r.get::<String,_>("status"),
                        "submitted_at": submitted_at,
                        "full_name": r.try_get::<String,_>("full_name").unwrap_or_default(),
                        "dob": r.try_get::<NaiveDate,_>("dob").ok().map(|d| d.to_string()),
                        "gender": r.try_get::<String,_>("gender").unwrap_or_default(),
                        "address": r.try_get::<String,_>("address").unwrap_or_default(),
                        "user_email": r.try_get::<String,_>("email").unwrap_or_default(),
                        "user_name": r.try_get::<String,_>("user_name").unwrap_or_default(),
                        "admin_note": r.try_get::<String,_>("admin_note").unwrap_or_default(),
                        "front_url": format!("/api/admin/kyc_requests/{}/file/front", r.get::<i32,_>("id")),
                        "back_url": format!("/api/admin/kyc_requests/{}/file/back", r.get::<i32,_>("id")),
                    })
                })
                .collect();
            HttpResponse::Ok().json(json!({"items": mapped}))
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)})),
    }
}

#[derive(Deserialize)]
struct DecisionPayload {
    status: String,
    note: Option<String>,
}

#[post("/api/admin/kyc_requests/{id}/decision")]
async fn decide_kyc(path: web::Path<i32>, payload: web::Json<DecisionPayload>, session: Session) -> impl Responder {
    let admin_id = match ensure_admin(&session) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let status = payload.status.trim().to_lowercase();
    if status != "approved" && status != "rejected" && status != "pending" {
        return HttpResponse::BadRequest().json(json!({"error": "invalid status"}));
    }

    let pool_data = match POOL_DATA.get() {
        Some(p) => p,
        None => return HttpResponse::InternalServerError().json(json!({"error": "no db"})),
    };

    let user_row = sqlx::query("SELECT user_id FROM teacher_verifications WHERE id = $1")
        .bind(*path)
        .fetch_optional(pool_data.get_ref())
        .await;

    let user_id: i32 = match user_row {
        Ok(Some(r)) => r.get::<i32, _>("user_id"),
        Ok(None) => return HttpResponse::NotFound().json(json!({"error": "verification not found"})),
        Err(e) => return HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)})),
    };

    let res = sqlx::query("UPDATE teacher_verifications SET status = $1, admin_note = $2, reviewed_by = $3, updated_at = now() WHERE id = $4")
        .bind(&status)
        .bind(payload.note.as_deref().unwrap_or(""))
        .bind(admin_id)
        .bind(*path)
        .execute(pool_data.get_ref())
        .await;

    if let Err(e) = res {
        return HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)}));
    }

    let approved = status == "approved";
    if let Err(e) = sqlx::query("UPDATE users SET kyc_verified = $1, kyc_verified_at = CASE WHEN $1 THEN now() ELSE NULL END WHERE id = $2")
        .bind(approved)
        .bind(user_id)
        .execute(pool_data.get_ref())
        .await
    {
        return HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)}));
    }

    HttpResponse::Ok().json(json!({"ok": true, "status": status, "kyc_verified": approved}))
}

#[derive(Deserialize)]
struct BulkDecisionPayload {
    ids: Vec<i32>,
    status: String,
    note: Option<String>,
}

#[post("/api/admin/kyc_requests/bulk_decision")]
async fn bulk_decide(payload: web::Json<BulkDecisionPayload>, session: Session) -> impl Responder {
    let admin_id = match ensure_admin(&session) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    if payload.ids.is_empty() { return HttpResponse::BadRequest().json(json!({"error": "no ids provided"})); }
    let status = payload.status.trim().to_lowercase();
    if status != "approved" && status != "rejected" && status != "pending" {
        return HttpResponse::BadRequest().json(json!({"error": "invalid status"}));
    }

    let pool_data = match POOL_DATA.get() {
        Some(p) => p,
        None => return HttpResponse::InternalServerError().json(json!({"error": "no db"})),
    };

    let res = sqlx::query("UPDATE teacher_verifications SET status = $1, admin_note = $2, reviewed_by = $3, updated_at = now() WHERE id = ANY($4)")
        .bind(&status)
        .bind(payload.note.as_deref().unwrap_or(""))
        .bind(admin_id)
        .bind(&payload.ids)
        .execute(pool_data.get_ref())
        .await;

    if let Err(e) = res {
        return HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)}));
    }

    let approved = status == "approved";
    if let Err(e) = sqlx::query("UPDATE users SET kyc_verified = $1, kyc_verified_at = CASE WHEN $1 THEN now() ELSE NULL END WHERE id IN (SELECT user_id FROM teacher_verifications WHERE id = ANY($2))")
        .bind(approved)
        .bind(&payload.ids)
        .execute(pool_data.get_ref())
        .await
    {
        return HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)}));
    }

    HttpResponse::Ok().json(json!({"ok": true, "status": status, "count": payload.ids.len(), "kyc_verified": approved}))
}

#[get("/api/admin/kyc_requests/{id}/file/{side}")]
async fn download_kyc_file(path: web::Path<(i32, String)>, session: Session) -> actix_web::Result<NamedFile> {
    if ensure_admin(&session).is_err() {
        return Err(actix_web::error::ErrorUnauthorized("unauthorized"));
    }

    let (id, side) = path.into_inner();
    let pool_data = POOL_DATA.get().ok_or_else(|| actix_web::error::ErrorInternalServerError("no db"))?;
    let row = sqlx::query("SELECT front_id_path, back_id_path FROM teacher_verifications WHERE id = $1")
        .bind(id)
        .fetch_optional(pool_data.get_ref())
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;
    if let Some(r) = row {
        let path_str: Option<String> = match side.as_str() {
            "front" => r.try_get("front_id_path").ok(),
            "back" => r.try_get("back_id_path").ok(),
            _ => None,
        };
        if let Some(p) = path_str {
            let fs_path = std::path::PathBuf::from(p);
            if fs_path.exists() {
                return Ok(NamedFile::open_async(fs_path).await?);
            }
        }
    }
    Err(actix_web::error::ErrorNotFound("file not found"))
}

#[get("/api/admin/kyc_export")]
async fn export_kyc(session: Session) -> impl Responder {
    if let Err(resp) = ensure_admin(&session) { return resp; }
    let pool_data = match POOL_DATA.get() {
        Some(p) => p,
        None => return HttpResponse::InternalServerError().json(json!({"error": "no db"})),
    };
    let rows = sqlx::query("SELECT tv.id, tv.status, tv.created_at, tv.full_name, tv.dob, tv.gender, tv.address, u.email, u.full_name AS user_name FROM teacher_verifications tv JOIN users u ON tv.user_id = u.id ORDER BY tv.created_at DESC")
        .fetch_all(pool_data.get_ref())
        .await;
    match rows {
        Ok(list) => {
            let mut wtr = String::from("id,status,submitted_at,full_name,dob,gender,address,email,user_name\n");
            for r in list {
                let dob = r.try_get::<NaiveDate,_>("dob").ok().map(|d| d.to_string()).unwrap_or_default();
                wtr.push_str(&format!("{},{},{},{},{},{},{},{},{}\n",
                    r.get::<i32,_>("id"),
                    r.get::<String,_>("status"),
                    r.try_get::<NaiveDateTime,_>("created_at").ok().unwrap_or_else(|| NaiveDateTime::from_timestamp_opt(0,0).unwrap()),
                    r.try_get::<String,_>("full_name").unwrap_or_default(),
                    dob,
                    r.try_get::<String,_>("gender").unwrap_or_default(),
                    r.try_get::<String,_>("address").unwrap_or_default(),
                    r.try_get::<String,_>("email").unwrap_or_default(),
                    r.try_get::<String,_>("user_name").unwrap_or_default()
                ));
            }
            HttpResponse::Ok()
                .append_header(("Content-Type", "text/csv"))
                .append_header(("Content-Disposition", "attachment; filename=kyc_export.csv"))
                .body(wtr)
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)})),
    }
}

#[get("/api/admin/users")]
async fn list_users(session: Session) -> impl Responder {
    if let Err(resp) = ensure_admin(&session) { return resp; }
    let pool_data = match POOL_DATA.get() {
        Some(p) => p,
        None => return HttpResponse::InternalServerError().json(json!({"error": "no db"})),
    };
    let rows = sqlx::query("SELECT id, full_name, email, role, active, verified, kyc_verified, kyc_verified_at, created_at FROM users ORDER BY created_at DESC")
        .fetch_all(pool_data.get_ref())
        .await;
    match rows {
        Ok(list) => {
            let mapped: Vec<serde_json::Value> = list.into_iter().map(|r| {
                json!({
                    "id": r.get::<i32,_>("id"),
                    "full_name": r.get::<String,_>("full_name"),
                    "email": r.get::<String,_>("email"),
                    "role": r.try_get::<String,_>("role").unwrap_or_else(|_| "student".to_string()),
                    "active": r.try_get::<bool,_>("active").unwrap_or(true),
                    "verified": r.try_get::<bool,_>("verified").unwrap_or(false),
                    "kyc_verified": r.try_get::<bool,_>("kyc_verified").unwrap_or(false),
                    "kyc_verified_at": r.try_get::<NaiveDateTime,_>("kyc_verified_at").ok().map(|d| d.to_string()),
                    "created_at": r
                        .try_get::<DateTime<Utc>, _>("created_at")
                        .map(|d| d.to_rfc3339())
                        .or_else(|_| r.try_get::<NaiveDateTime, _>("created_at").map(|d| DateTime::<Utc>::from_naive_utc_and_offset(d, Utc).to_rfc3339()))
                        .unwrap_or_default(),
                })
            }).collect();
            HttpResponse::Ok().json(json!({"items": mapped}))
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)})),
    }
}

#[derive(Deserialize)]
struct RolePayload { role: String }

#[post("/api/admin/users/{id}/role")]
async fn update_role(path: web::Path<i32>, payload: web::Json<RolePayload>, session: Session) -> impl Responder {
    if let Err(resp) = ensure_admin(&session) { return resp; }
    let role = payload.role.trim().to_lowercase();
    if role != "admin" && role != "teacher" && role != "student" {
        return HttpResponse::BadRequest().json(json!({"error": "invalid role"}));
    }
    let pool_data = match POOL_DATA.get() { Some(p)=>p, None=>return HttpResponse::InternalServerError().json(json!({"error":"no db"}))};
    let res = sqlx::query("UPDATE users SET role = $1 WHERE id = $2")
        .bind(&role)
        .bind(*path)
        .execute(pool_data.get_ref())
        .await;
    match res {
        Ok(_) => HttpResponse::Ok().json(json!({"ok": true, "role": role})),
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)})),
    }
}

#[derive(Deserialize)]
struct ActivePayload { active: bool }

#[post("/api/admin/users/{id}/active")]
async fn update_active(path: web::Path<i32>, payload: web::Json<ActivePayload>, session: Session) -> impl Responder {
    if let Err(resp) = ensure_admin(&session) { return resp; }
    let pool_data = match POOL_DATA.get() { Some(p)=>p, None=>return HttpResponse::InternalServerError().json(json!({"error":"no db"}))};
    let res = sqlx::query("UPDATE users SET active = $1 WHERE id = $2")
        .bind(payload.active)
        .bind(*path)
        .execute(pool_data.get_ref())
        .await;
    match res {
        Ok(_) => HttpResponse::Ok().json(json!({"ok": true, "active": payload.active})),
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)})),
    }
}

#[post("/api/admin/users/{id}/reset_password")]
async fn reset_password(path: web::Path<i32>, session: Session) -> impl Responder {
    if let Err(resp) = ensure_admin(&session) { return resp; }
    let pool_data = match POOL_DATA.get() { Some(p)=>p, None=>return HttpResponse::InternalServerError().json(json!({"error":"no db"}))};
    let new_pw: String = rand::thread_rng().sample_iter(&Alphanumeric).take(12).map(char::from).collect();
    let salt = SaltString::generate(&mut OsRng);
    let hash = match Argon2::default().hash_password(new_pw.as_bytes(), &salt) {
        Ok(h) => h.to_string(),
        Err(e) => return HttpResponse::InternalServerError().json(json!({"error": e.to_string()})),
    };
    let res = sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
        .bind(hash)
        .bind(*path)
        .execute(pool_data.get_ref())
        .await;
    match res {
        Ok(_) => HttpResponse::Ok().json(json!({"ok": true, "temporary_password": new_pw})),
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)})),
    }
}

#[derive(Deserialize)]
struct SearchUsersQuery { q: Option<String> }

#[get("/api/admin/users/search")]
async fn search_users(query: web::Query<SearchUsersQuery>, session: Session) -> impl Responder {
    if let Err(resp) = ensure_admin(&session) { return resp; }
    let pool_data = match POOL_DATA.get() {
        Some(p) => p,
        None => return HttpResponse::InternalServerError().json(json!({"error": "no db"})),
    };

    let pattern = query.q.as_deref().unwrap_or("").trim();
    let like = format!("%{}%", pattern);

    let base_sql = "SELECT u.id, u.full_name, u.email, u.role, u.kyc_verified,
        COALESCE(tv.status, 'unverified') AS kyc_status
        FROM users u
        LEFT JOIN LATERAL (
            SELECT status FROM teacher_verifications tv
            WHERE tv.user_id = u.id
            ORDER BY updated_at DESC NULLS LAST, created_at DESC
            LIMIT 1
        ) tv ON TRUE";

    let rows = if pattern.is_empty() {
        sqlx::query(&format!("{} ORDER BY u.created_at DESC LIMIT 20", base_sql))
            .fetch_all(pool_data.get_ref())
            .await
    } else {
        sqlx::query(&format!("{} WHERE u.full_name ILIKE $1 OR u.email ILIKE $1 ORDER BY u.created_at DESC LIMIT 20", base_sql))
            .bind(&like)
            .fetch_all(pool_data.get_ref())
            .await
    };

    match rows {
        Ok(list) => {
            let mapped: Vec<serde_json::Value> = list
                .into_iter()
                .map(|r| json!({
                    "id": r.get::<i32,_>("id"),
                    "full_name": r.get::<String,_>("full_name"),
                    "email": r.get::<String,_>("email"),
                    "role": r.try_get::<String,_>("role").unwrap_or_else(|_| "student".to_string()),
                    "kyc_status": r.try_get::<String,_>("kyc_status").ok(),
                    "kyc_verified": r.try_get::<bool,_>("kyc_verified").unwrap_or(false),
                }))
                .collect();
            HttpResponse::Ok().json(json!({"items": mapped}))
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)})),
    }
}

#[derive(Deserialize)]
struct SupportQuery { status: Option<String> }

#[get("/api/admin/support_requests")]
async fn list_support_requests(query: web::Query<SupportQuery>, session: Session) -> impl Responder {
    if let Err(resp) = ensure_admin(&session) { return resp; }
    let pool_data = match POOL_DATA.get() {
        Some(p) => p,
        None => return HttpResponse::InternalServerError().json(json!({"error": "no db"})),
    };

    let status = query.status.clone().unwrap_or_else(|| "new".to_string());
    let rows = sqlx::query("SELECT sr.id, sr.role, sr.kyc_status, sr.body, sr.attachment_path, sr.status, sr.created_at, u.full_name, u.email FROM support_requests sr JOIN users u ON sr.user_id = u.id WHERE sr.status = $1 ORDER BY sr.created_at DESC")
        .bind(&status)
        .fetch_all(pool_data.get_ref())
        .await;

    match rows {
        Ok(list) => {
            let mapped: Vec<serde_json::Value> = list.into_iter().map(|r| {
                let created_at = r
                    .try_get::<DateTime<Utc>, _>("created_at")
                    .map(|d| d.to_rfc3339())
                    .or_else(|_| r.try_get::<NaiveDateTime, _>("created_at").map(|d| DateTime::<Utc>::from_naive_utc_and_offset(d, Utc).to_rfc3339()))
                    .unwrap_or_default();
                json!({
                    "id": r.get::<i32,_>("id"),
                    "role": r.get::<String,_>("role"),
                    "kyc_status": r.get::<String,_>("kyc_status"),
                    "body": r.get::<String,_>("body"),
                    "status": r.get::<String,_>("status"),
                    "created_at": created_at,
                    "user_name": r.get::<String,_>("full_name"),
                    "user_email": r.get::<String,_>("email"),
                    "attachment_url": r.try_get::<String,_>("attachment_path").ok().map(|_| format!("/api/admin/support_requests/{}/attachment", r.get::<i32,_>("id"))),
                })
            }).collect();
            HttpResponse::Ok().json(json!({"items": mapped}))
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)})),
    }
}

#[post("/api/admin/support_requests/{id}/status")]
async fn update_support_status(path: web::Path<i32>, payload: web::Json<serde_json::Value>, session: Session) -> impl Responder {
    if let Err(resp) = ensure_admin(&session) { return resp; }
    let status = payload.get("status").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
    if status != "new" && status != "done" { return HttpResponse::BadRequest().json(json!({"error":"invalid status"})); }

    let pool_data = match POOL_DATA.get() { Some(p)=>p, None=>return HttpResponse::InternalServerError().json(json!({"error":"no db"}))};
    let res = sqlx::query("UPDATE support_requests SET status = $1, updated_at = now() WHERE id = $2")
        .bind(&status)
        .bind(*path)
        .execute(pool_data.get_ref())
        .await;
    match res {
        Ok(_) => HttpResponse::Ok().json(json!({"ok": true, "status": status})),
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)})),
    }
}

#[get("/api/admin/support_requests/{id}/attachment")]
async fn support_attachment(path: web::Path<i32>, session: Session, req: HttpRequest) -> actix_web::Result<HttpResponse> {
    if ensure_admin(&session).is_err() {
        return Err(actix_web::error::ErrorUnauthorized("unauthorized"));
    }

    let pool_data = POOL_DATA.get().ok_or_else(|| actix_web::error::ErrorInternalServerError("no db"))?;
    let row = sqlx::query("SELECT attachment_path FROM support_requests WHERE id = $1")
        .bind(*path)
        .fetch_optional(pool_data.get_ref())
        .await
        .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    if let Some(r) = row {
        let path_str: Option<String> = r.try_get("attachment_path").ok();
        if let Some(p) = path_str {
            let fs_path = PathBuf::from(&p);
            if fs_path.exists() {
                let file = NamedFile::open_async(fs_path).await?;
                return Ok(file.into_response(&req));
            }
        }
    }
    Err(actix_web::error::ErrorNotFound("file not found"))
}

#[post("/api/admin/notifications")]
async fn admin_send_notification(session: Session, mut payload: Multipart) -> impl Responder {
    let admin_id = match ensure_admin(&session) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let pool_data = match POOL_DATA.get() {
        Some(p) => p,
        None => return HttpResponse::InternalServerError().json(json!({"error": "no db"})),
    };

    let mut target_user: Option<i32> = None;
    let mut title: Option<String> = None;
    let mut body: Option<String> = None;
    let mut attachment_path: Option<String> = None;

    // Ensure uploads dir exists
    let upload_dir = std::path::Path::new("uploads/notifications");
    if !upload_dir.exists() {
        let _ = fs::create_dir_all(upload_dir);
    }

    while let Some(field_res) = payload.next().await {
        let mut field = match field_res {
            Ok(f) => f,
            Err(e) => return HttpResponse::BadRequest().json(json!({"error": format!("multipart error: {}", e)})),
        };
        let name = field.name().to_string();
        if name == "attachment" {
            let original = field
                .content_disposition()
                .get_filename()
                .map(|v| sanitize(v));
            let safe_name = original.unwrap_or_else(|| format!("file-{}.bin", Uuid::new_v4()));
            let full_path = upload_dir.join(format!("{}-{}", Uuid::new_v4(), safe_name));
            let mut f = match TokioFile::create(&full_path).await {
                Ok(file) => file,
                Err(e) => return HttpResponse::InternalServerError().json(json!({"error": format!("file create: {}", e)})),
            };
            while let Some(chunk) = field.next().await {
                let data = match chunk {
                    Ok(d) => d,
                    Err(e) => return HttpResponse::BadRequest().json(json!({"error": format!("upload chunk: {}", e)})),
                };
                if let Err(e) = f.write_all(&data).await {
                    return HttpResponse::InternalServerError().json(json!({"error": format!("write failed: {}", e)}));
                }
            }
            attachment_path = Some(full_path.to_string_lossy().to_string());
        } else {
            let mut bytes = Vec::new();
            while let Some(chunk) = field.next().await {
                let data = match chunk {
                    Ok(d) => d,
                    Err(e) => return HttpResponse::BadRequest().json(json!({"error": format!("field read: {}", e)})),
                };
                bytes.extend_from_slice(&data);
            }
            let text = String::from_utf8(bytes).unwrap_or_default();
            match name.as_str() {
                "user_id" => {
                    target_user = text.trim().parse::<i32>().ok();
                }
                "title" => title = Some(text.trim().to_string()),
                "body" => body = Some(text.trim().to_string()),
                _ => {}
            }
        }
    }

    let user_id = match target_user {
        Some(id) => id,
        None => return HttpResponse::BadRequest().json(json!({"error": "user_id required"})),
    };
    let title = match title {
        Some(t) if !t.is_empty() => t,
        _ => return HttpResponse::BadRequest().json(json!({"error": "title required"})),
    };
    let body_val = match body {
        Some(b) if !b.is_empty() => b,
        _ => return HttpResponse::BadRequest().json(json!({"error": "body required"})),
    };

    let row = sqlx::query("INSERT INTO notifications (user_id, sender_id, title, body, attachment_path) VALUES ($1, $2, $3, $4, $5) RETURNING id, created_at")
        .bind(user_id)
        .bind(admin_id)
        .bind(&title)
        .bind(&body_val)
        .bind(&attachment_path)
        .fetch_one(pool_data.get_ref())
        .await;

    match row {
        Ok(r) => {
            let notif_id: i32 = r.get("id");
            let created_at = r
                .try_get::<DateTime<Utc>, _>("created_at")
                .unwrap_or_else(|_| {
                    let naive: NaiveDateTime = r.get("created_at");
                    DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc)
                });
            let attachment_url = attachment_path.as_ref().map(|_| format!("/api/notifications/{}/attachment", notif_id));
            broadcast_notification(NotificationEvent {
                id: notif_id,
                user_id,
                title: title.clone(),
                body: body_val.clone(),
                attachment_url,
                created_at: created_at.to_rfc3339(),
                read: false,
            });
            HttpResponse::Ok().json(json!({"ok": true, "id": notif_id}))
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)})),
    }
}

#[get("/api/admin/notifications")]
async fn admin_list_notifications(session: Session) -> impl Responder {
    let admin_id = match ensure_admin(&session) { Ok(id)=>id, Err(resp)=>return resp };
    let pool_data = match POOL_DATA.get() { Some(p)=>p, None=>return HttpResponse::InternalServerError().json(json!({"error":"no db"}))};

    let rows = sqlx::query("SELECT n.id, n.title, n.body, n.attachment_path, n.created_at, u.full_name, u.email, u.role FROM notifications n JOIN users u ON u.id = n.user_id WHERE n.sender_id = $1 ORDER BY n.created_at DESC LIMIT 100")
        .bind(admin_id)
        .fetch_all(pool_data.get_ref())
        .await;

    match rows {
        Ok(list) => {
            let mapped: Vec<serde_json::Value> = list.into_iter().map(|r| {
                let id: i32 = r.get("id");
                let attachment_path: Option<String> = r.try_get("attachment_path").ok();
                let created_at = r
                    .try_get::<DateTime<Utc>, _>("created_at")
                    .unwrap_or_else(|_| {
                        let naive: NaiveDateTime = r.get("created_at");
                        DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc)
                    });
                json!({
                    "id": id,
                    "title": r.get::<String,_>("title"),
                    "body": r.get::<String,_>("body"),
                    "attachment_url": attachment_path.map(|_| format!("/api/notifications/{}/attachment", id)),
                    "created_at": created_at.to_rfc3339(),
                    "user_name": r.get::<String,_>("full_name"),
                    "user_email": r.get::<String,_>("email"),
                    "user_role": r.try_get::<String,_>("role").unwrap_or_else(|_| "student".to_string()),
                })
            }).collect();
            HttpResponse::Ok().json(json!({"items": mapped}))
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)})),
    }
}

#[derive(Deserialize)]
struct UpdateNotificationPayload {
    title: Option<String>,
    body: Option<String>,
    remove_attachment: Option<bool>,
}

#[post("/api/admin/notifications/{id}/update")]
async fn admin_update_notification(path: web::Path<i32>, payload: web::Json<UpdateNotificationPayload>, session: Session) -> impl Responder {
    if let Err(resp) = ensure_admin(&session) { return resp; }
    let pool_data = match POOL_DATA.get() { Some(p)=>p, None=>return HttpResponse::InternalServerError().json(json!({"error":"no db"}))};

    let notif_id = *path;
    let row = sqlx::query("SELECT attachment_path FROM notifications WHERE id = $1")
        .bind(notif_id)
        .fetch_optional(pool_data.get_ref())
        .await;
    let existing = match row {
        Ok(Some(r)) => r,
        Ok(None) => return HttpResponse::NotFound().json(json!({"error":"not found"})),
        Err(e) => return HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)})),
    };

    let mut new_title = payload.title.as_ref().map(|s| s.trim().to_string()).unwrap_or_default();
    let mut new_body = payload.body.as_ref().map(|s| s.trim().to_string()).unwrap_or_default();
    if new_title.is_empty() && new_body.is_empty() && payload.remove_attachment.unwrap_or(false)==false {
        return HttpResponse::BadRequest().json(json!({"error":"nothing to update"}));
    }

    let remove_attach = payload.remove_attachment.unwrap_or(false);
    let existing_attach: Option<String> = existing.try_get("attachment_path").ok();

    let res = sqlx::query("UPDATE notifications SET title = COALESCE(NULLIF($1,''), title), body = COALESCE(NULLIF($2,''), body), attachment_path = CASE WHEN $3 THEN NULL ELSE attachment_path END WHERE id = $4")
        .bind(&new_title)
        .bind(&new_body)
        .bind(remove_attach)
        .bind(notif_id)
        .execute(pool_data.get_ref())
        .await;

    if let Err(e) = res {
        return HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)}));
    }

    if remove_attach {
        if let Some(path) = existing_attach {
            let _ = std::fs::remove_file(path);
        }
    }

    HttpResponse::Ok().json(json!({"ok": true, "id": notif_id}))
}

#[post("/api/admin/notifications/{id}/delete")]
async fn admin_delete_notification(path: web::Path<i32>, session: Session) -> impl Responder {
    if let Err(resp) = ensure_admin(&session) { return resp; }
    let pool_data = match POOL_DATA.get() { Some(p)=>p, None=>return HttpResponse::InternalServerError().json(json!({"error":"no db"}))};
    let notif_id = *path;

    let row = sqlx::query("SELECT attachment_path FROM notifications WHERE id = $1")
        .bind(notif_id)
        .fetch_optional(pool_data.get_ref())
        .await;
    let attachment: Option<String> = match row {
        Ok(Some(r)) => r.try_get("attachment_path").ok(),
        Ok(None) => return HttpResponse::NotFound().json(json!({"error":"not found"})),
        Err(e) => return HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)})),
    };

    let res = sqlx::query("DELETE FROM notifications WHERE id = $1")
        .bind(notif_id)
        .execute(pool_data.get_ref())
        .await;

    if let Err(e) = res {
        return HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)}));
    }

    if let Some(path) = attachment {
        let _ = std::fs::remove_file(path);
    }

    HttpResponse::Ok().json(json!({"ok": true, "id": notif_id}))
}

#[derive(Deserialize)]
struct ImpersonatePayload { user_id: i32 }

#[post("/api/admin/impersonate")]
async fn impersonate(payload: web::Json<ImpersonatePayload>, session: Session) -> impl Responder {
    let admin_id = match ensure_admin(&session) { Ok(id)=>id, Err(resp)=>return resp };
    let pool_data = match POOL_DATA.get() { Some(p)=>p, None=>return HttpResponse::InternalServerError().json(json!({"error":"no db"}))};
    let row = sqlx::query("SELECT id, role FROM users WHERE id = $1")
        .bind(payload.user_id)
        .fetch_optional(pool_data.get_ref())
        .await;
    match row {
        Ok(Some(r)) => {
            let role: String = r.try_get("role").unwrap_or_else(|_| "student".to_string());
            let _ = session.insert("admin_original_user_id", admin_id);
            let _ = session.insert("user_id", payload.user_id);
            let _ = session.insert("role", role.clone());
            HttpResponse::Ok().json(json!({"ok": true, "impersonating": payload.user_id, "role": role}))
        }
        Ok(None) => HttpResponse::NotFound().json(json!({"error": "user not found"})),
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)})),
    }
}

#[post("/api/admin/impersonate/stop")]
async fn stop_impersonate(session: Session) -> impl Responder {
    if let Err(resp) = ensure_admin(&session) { return resp; }
    if let Some(original) = session.get::<i32>("admin_original_user_id").unwrap_or(None) {
        let _ = session.insert("user_id", original);
        let _ = session.remove("admin_original_user_id");
        // role will refresh on next request from DB; keep as admin
        let _ = session.insert("role", "admin");
        HttpResponse::Ok().json(json!({"ok": true, "user_id": original}))
    } else {
        HttpResponse::Ok().json(json!({"ok": true, "message": "not impersonating"}))
    }
}

pub fn init(cfg: &mut web::ServiceConfig) {
    cfg.service(admin_portal)
        .service(list_kyc)
        .service(decide_kyc)
        .service(bulk_decide)
        .service(download_kyc_file)
        .service(export_kyc)
        .service(list_users)
        .service(update_role)
        .service(update_active)
        .service(reset_password)
        .service(impersonate)
    .service(stop_impersonate)
    .service(search_users)
        .service(admin_send_notification)
        .service(admin_list_notifications)
        .service(admin_update_notification)
        .service(admin_delete_notification)
        .service(list_support_requests)
        .service(update_support_status)
        .service(support_attachment);
}
