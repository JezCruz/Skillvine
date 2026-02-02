// src/main.rs

mod routes;
mod services;

use actix_files::{Files, NamedFile};
use actix_session::Session;
use actix_web::{cookie::{Cookie, Key}, get, post, web, HttpRequest, HttpResponse, Responder};
use actix_session::SessionMiddleware;
use actix_session::storage::CookieSessionStore;
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use dotenv::dotenv;
use password_hash::SaltString;
use rand_core::OsRng;
use serde::Deserialize;
use actix_web::http::header::CONTENT_TYPE;
use sqlx::PgPool;
use sqlx::Row;
use std::time::Duration;
use tokio::time::sleep;

// Shuttle / secrets (optional for local builds)
#[cfg(feature = "shuttle")]
use shuttle_actix_web::ActixWebService;
#[cfg(feature = "shuttle")]
use shuttle_runtime::Error as ShuttleError;
#[cfg(feature = "shuttle")]
use shuttle_secrets::SecretStore;

// once_cell to hold globals accessible by a function pointer
use once_cell::sync::OnceCell;

// utilities
// utilities

// ------------------- DATABASE MODELS -------------------
#[derive(Debug, sqlx::FromRow)]
struct User {
    id: i32,
    full_name: String,
    email: String,
    password_hash: String,
    role: Option<String>,
}

// ------------------- HELPERS -------------------
fn require_login(session: &Session) -> bool {
    session.get::<i32>("user_id").unwrap_or(None).is_some()
}

async fn log_activity(user_id: i32, action: &str) {
    if let Some(pool_data) = POOL_DATA.get() {
        let db = pool_data.get_ref();
        let _ = sqlx::query("INSERT INTO user_activity (user_id, action) VALUES ($1, $2)")
            .bind(user_id)
            .bind(action)
            .execute(db)
            .await;
    }
}

// ------------------- STATIC PAGES -------------------
#[get("/")]
async fn home(session: Session, req: HttpRequest) -> actix_web::Result<HttpResponse> {
    if let Some(role) = session.get::<String>("role").unwrap_or(None) {
        let redirect_url = match role.as_str() {
            "admin" => "/admin",
            "teacher" => "/teacher_dashboard",
            _ => "/student_dashboard",
        };
        return Ok(HttpResponse::Found()
            .append_header(("Location", redirect_url))
            .finish());
    }

    let file = NamedFile::open_async("./templates/overview.html").await?;
    Ok(file.into_response(&req))
}

#[get("/login")]
async fn login(session: Session, req: HttpRequest) -> actix_web::Result<HttpResponse> {
    if let Some(role) = session.get::<String>("role").unwrap_or(None) {
        let redirect_url = match role.as_str() {
            "admin" => "/admin",
            "teacher" => "/teacher_dashboard",
            _ => "/student_dashboard",
        };
        return Ok(HttpResponse::Found()
            .append_header(("Location", redirect_url))
            .finish());
    }

    let file = NamedFile::open_async("./templates/login.html").await?;
    Ok(file.into_response(&req))
}

#[get("/signup")]
async fn signup(session: Session, req: HttpRequest) -> actix_web::Result<HttpResponse> {
    if let Some(role) = session.get::<String>("role").unwrap_or(None) {
        let redirect_url = match role.as_str() {
            "admin" => "/admin",
            "teacher" => "/teacher_dashboard",
            _ => "/student_dashboard",
        };
        return Ok(HttpResponse::Found()
            .append_header(("Location", redirect_url))
            .finish());
    }

    let file = NamedFile::open_async("./templates/signup.html").await?;
    Ok(file.into_response(&req))
}

#[get("/logout")]
async fn logout(session: Session) -> impl Responder {
    if let Some(user_id) = session.get::<i32>("user_id").unwrap_or(None) {
        log_activity(user_id, "Logged out").await;
    }

    session.purge();
    HttpResponse::Found()
        .append_header(("Location", "/"))
        .finish()
}

// ------------------- DASHBOARD & SETTINGS -------------------
#[get("/profile")]
async fn profile(session: Session, req: HttpRequest) -> actix_web::Result<HttpResponse> {
    if !require_login(&session) {
        let file = NamedFile::open_async("./templates/login.html").await?;
        return Ok(file.into_response(&req));
    }
    let file = NamedFile::open_async("./templates/profile.html").await?;
    Ok(file.into_response(&req))
}

#[get("/settings")]
async fn settings(session: Session, req: HttpRequest) -> actix_web::Result<HttpResponse> {
    if !require_login(&session) {
        let file = NamedFile::open_async("./templates/login.html").await?;
        return Ok(file.into_response(&req));
    }
    let file = NamedFile::open_async("./templates/settings.html").await?;
    Ok(file.into_response(&req))
}

#[get("/teacher_dashboard")]
async fn teacher_dashboard(session: Session, req: HttpRequest) -> actix_web::Result<HttpResponse> {
    if !require_login(&session) {
        let file = NamedFile::open_async("./templates/login.html").await?;
        return Ok(file.into_response(&req));
    }

    if let Some(user_id) = session.get::<i32>("user_id").unwrap_or(None) {
        log_activity(user_id, "Visited Teacher Dashboard").await;
    }

    let file = NamedFile::open_async("./templates/teacher_dashboard.html").await?;
    Ok(file.into_response(&req))
}

#[get("/student_dashboard")]
async fn student_dashboard(session: Session, req: HttpRequest) -> actix_web::Result<HttpResponse> {
    if !require_login(&session) {
        let file = NamedFile::open_async("./templates/login.html").await?;
        return Ok(file.into_response(&req));
    }

    if let Some(user_id) = session.get::<i32>("user_id").unwrap_or(None) {
        log_activity(user_id, "Visited Student Dashboard").await;
    }

    let file = NamedFile::open_async("./templates/student_dashboard.html").await?;
    Ok(file.into_response(&req))
}

#[get("/status")]
async fn status() -> impl Responder {
    let db = POOL_DATA.get().is_some();
    let secret = SECRET_KEY_CELL.get().is_some();
    HttpResponse::Ok().json(serde_json::json!({"db_connected": db, "secret_set": secret}))
}

// ------------------- AUTH HANDLERS -------------------
#[derive(Deserialize)]
struct SignupData {
    full_name: String,
    email: String,
    password: String,
    role: String,
}

#[post("/signup")]
async fn signup_submit(req: HttpRequest, body: web::Bytes, session: Session) -> impl Responder {
    // Parse JSON or form-urlencoded based on Content-Type
    let data: SignupData = match req.headers().get(CONTENT_TYPE).and_then(|v| v.to_str().ok()) {
        Some(ct) if ct.contains("application/json") => match serde_json::from_slice(&body) {
            Ok(d) => d,
            Err(e) => return HttpResponse::BadRequest().body(format!("Invalid JSON: {}", e)),
        },
        Some(ct) if ct.contains("application/x-www-form-urlencoded") => match serde_urlencoded::from_bytes::<SignupData>(&body) {
            Ok(d) => d,
            Err(e) => return HttpResponse::BadRequest().body(format!("Invalid form data: {}", e)),
        },
        _ => return HttpResponse::UnsupportedMediaType().body("Unsupported content type"),
    };
    let full_name = data.full_name.trim().to_string();
    let email = data.email.trim().to_lowercase();

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hashed_password = match argon2.hash_password(data.password.as_bytes(), &salt) {
        Ok(hash) => hash.to_string(),
        Err(_) => return HttpResponse::InternalServerError().body("Server error"),
    };

    let pool = match POOL_DATA.get() {
        Some(d) => d.get_ref(),
        None => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Database not configured"
            }));
        }
    };

    let result = sqlx::query(
        "INSERT INTO users (full_name, email, password_hash, role) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(&full_name)
    .bind(&email)
    .bind(&hashed_password)
    .bind(&data.role)
    .fetch_one(pool)
    .await;

    match result {
        Ok(row) => {
            let inserted_id: i32 = row.get("id");
            session.insert("user_id", inserted_id).unwrap_or_default();
            session.insert("full_name", &full_name).unwrap_or_default();
            session.insert("role", &data.role).unwrap_or_default();

            let redirect_url = match data.role.as_str() {
                "teacher" => "/teacher_dashboard",
                _ => "/student_dashboard",
            };

            let flash_cookie = Cookie::build("flash", "Registered successfully").path("/").finish();
            HttpResponse::Ok().cookie(flash_cookie).json(serde_json::json!({
                "redirect": redirect_url,
                "full_name": full_name,
                "role": data.role
            }))
        }
        Err(e) => {
            // Log DB error for debugging (do not expose details in production)
            eprintln!("DB insert error (signup): {:?}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }))
        }
    }
}

#[derive(Deserialize)]
struct LoginData {
    email: String,
    password: String,
}

#[post("/login")]
async fn login_submit(req: HttpRequest, body: web::Bytes, session: Session) -> Result<HttpResponse, actix_web::Error> {
    let login_data: LoginData = match req.headers().get(CONTENT_TYPE).and_then(|v| v.to_str().ok()) {
        Some(ct) if ct.contains("application/json") => match serde_json::from_slice(&body) {
            Ok(d) => d,
            Err(e) => return Err(actix_web::error::ErrorBadRequest(format!("Invalid JSON: {}", e))),
        },
        Some(ct) if ct.contains("application/x-www-form-urlencoded") => match serde_urlencoded::from_bytes::<LoginData>(&body) {
            Ok(d) => d,
            Err(e) => return Err(actix_web::error::ErrorBadRequest(format!("Invalid form data: {}", e))),
        },
        _ => return Err(actix_web::error::ErrorUnsupportedMediaType("Unsupported content type")),
    };
    let email = login_data.email.trim();
    let password = login_data.password.trim();

    let pool = POOL_DATA
        .get()
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("Database not configured"))?;

    let row = match sqlx::query_as::<_, User>(
        "SELECT id, full_name, email, password_hash, role FROM users WHERE email = $1",
    )
    .bind(email)
    .fetch_optional(pool.get_ref())
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("DB query error (login): {:?}", e);
            return Err(actix_web::error::ErrorInternalServerError("Database error"));
        }
    };

    if let Some(user) = row {
        let parsed_hash = password_hash::PasswordHash::new(&user.password_hash)
            .map_err(actix_web::error::ErrorInternalServerError)?;

        if Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok()
        {
            session.insert("user_id", user.id)?;
            session.insert("full_name", &user.full_name)?;
            session.insert(
                "role",
                user.role.clone().unwrap_or_else(|| "student".to_string()),
            )?;

            log_activity(user.id, "Logged in").await;

            let redirect_url = match user.role.as_deref() {
                Some("admin") => "/admin",
                Some("teacher") => "/teacher_dashboard",
                _ => "/student_dashboard",
            };

            // If client expects JSON, return JSON with redirect so frontend can show notice
            let accept_hdr = req.headers().get("Accept").and_then(|v| v.to_str().ok()).unwrap_or("");
            let content_type_hdr = req.headers().get("Content-Type").and_then(|v| v.to_str().ok()).unwrap_or("");
            if accept_hdr.contains("application/json") || content_type_hdr.contains("application/json") {
                let flash_cookie = Cookie::build("flash", "Welcome back!").path("/").finish();
                return Ok(HttpResponse::Ok().cookie(flash_cookie).json(serde_json::json!({"redirect": redirect_url})));
            }

            let flash_cookie = Cookie::build("flash", "Welcome back!").path("/").finish();
            Ok(HttpResponse::Found()
                .cookie(flash_cookie)
                .append_header(("Location", redirect_url))
                .finish())
        } else {
            Ok(HttpResponse::Unauthorized().json(serde_json::json!({
                "error": "Invalid email or password"
            })))
        }
    } else {
        Ok(HttpResponse::Unauthorized().body("User not registered"))
    }
}

// ------------------- SHUTTLE ENTRYPOINT -------------------

// OnceCell globals so a top-level function pointer can access them (avoids non-Clone closure)
static POOL_DATA: OnceCell<web::Data<PgPool>> = OnceCell::new();
static SECRET_KEY_CELL: OnceCell<Key> = OnceCell::new();

fn configure(cfg: &mut web::ServiceConfig) {
    // Attach pool/secret_key only if available (allow running without DB/shuttle)
    if let Some(pool_data) = POOL_DATA.get() {
        cfg.app_data(pool_data.clone());
    }

    if let Some(secret_key) = SECRET_KEY_CELL.get() {
        cfg.app_data(web::Data::new(secret_key.clone()));
    }

    cfg.service(home)
        .service(login)
        .service(login_submit)
        .service(signup)
        .service(signup_submit)
        .configure(crate::routes::profile::init)
        .configure(crate::routes::admin::init)
        .configure(crate::routes::notifications::init)
        .service(profile)
        .service(settings)
        .service(teacher_dashboard)
        .service(student_dashboard)
        .service(logout)
        .service(Files::new("/static", "./static").show_files_listing());
}

/// Shuttle entrypoint
/// Request the DB connection string from shuttle and build a PgPool locally.
#[cfg(feature = "shuttle")]
#[shuttle_runtime::main]
async fn actix_web(
    #[shuttle_shared_db::Postgres] db_conn: String,
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
) -> Result<ActixWebService<fn(&mut web::ServiceConfig)>, ShuttleError> {
    dotenv().ok();

    // Create pool from provided connection string
    let pool = PgPool::connect(&db_conn)
        .await
        .map_err(|e| ShuttleError::from(anyhow!(e)))?;

    // read secret key from secrets store
    let secret_key_hex = secret_store
        .get("SECRET_KEY")
        .ok_or_else(|| ShuttleError::from(anyhow!("Missing SECRET_KEY secret")))?;

    let secret_bytes = hex::decode(&secret_key_hex).map_err(|e| ShuttleError::from(anyhow!(e)))?;

    let secret_key = Key::from(&secret_bytes);

    // store pool & key into OnceCell globals so `configure` (function pointer) can access them
    POOL_DATA
        .set(web::Data::new(pool))
        .map_err(|_| ShuttleError::from(anyhow!("POOL_DATA already set")))?;
    SECRET_KEY_CELL
        .set(secret_key)
        .map_err(|_| ShuttleError::from(anyhow!("SECRET_KEY already set")))?;

    // return a function pointer (fn), not a closure — function pointers are Clone
    Ok(ActixWebService(configure))
}

// Local/main entrypoint for running and testing without shuttle
#[cfg(not(feature = "shuttle"))]
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    // SECRET_KEY: prefer env var, otherwise generate a random key for local dev
    if let Ok(secret_key_hex) = std::env::var("SECRET_KEY") {
        if let Ok(secret_bytes) = hex::decode(&secret_key_hex) {
            let secret_key = Key::from(&secret_bytes);
            let _ = SECRET_KEY_CELL.set(secret_key);
        }
    } else {
        let generated = Key::generate();
        let _ = SECRET_KEY_CELL.set(generated);
    }

    // Optional DB: if DATABASE_URL is set, try to connect and store it; log error but don't panic
    if let Ok(db_conn) = std::env::var("DATABASE_URL") {
        // retry configuration: seconds between retries and max attempts (0 = retry forever)
        let retry_secs: u64 = std::env::var("DB_RETRY_SECONDS").ok().and_then(|s| s.parse().ok()).unwrap_or(2);
        let max_attempts: u32 = std::env::var("DB_RETRY_MAX").ok().and_then(|s| s.parse().ok()).unwrap_or(0);

        let mut attempt: u32 = 0;
        loop {
            attempt = attempt.saturating_add(1);
            match PgPool::connect(&db_conn).await {
                Ok(pool) => {
                    // Run migrations if migrations folder exists (non-fatal)
                    match sqlx::migrate!("./migrations").run(&pool).await {
                        Ok(_) => eprintln!("Migrations applied or already up-to-date"),
                        Err(e) => eprintln!("Migrations failed: {:?}", e),
                    }

                    let _ = POOL_DATA.set(web::Data::new(pool));
                    eprintln!("Connected to DATABASE_URL successfully (attempt {})", attempt);
                    break;
                }
                Err(e) => {
                    eprintln!("Failed to connect to DB (DATABASE_URL) on attempt {}: {:?}", attempt, e);
                    if max_attempts != 0 && attempt >= max_attempts {
                        eprintln!("Max DB connect attempts ({}) reached; continuing without DB", max_attempts);
                        break;
                    }
                    eprintln!("Retrying in {} seconds... (press Ctrl+C to abort)", retry_secs);
                    sleep(Duration::from_secs(retry_secs)).await;
                    continue;
                }
            }
        }
    }

    // (SECRET_KEY already initialized above)

    // Determine bind address and port (allow overriding via env)
    let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);

    // Log DB status (mask credentials) for convenience
    if POOL_DATA.get().is_some() {
        if let Ok(db_url) = std::env::var("DATABASE_URL") {
            if let Some(host) = extract_db_host(&db_url) {
                eprintln!("Database connected (host: {})", host);
            } else {
                eprintln!("Database connected");
            }
        } else {
            eprintln!("Database connected");
        }
    } else if std::env::var("DATABASE_URL").is_ok() {
        eprintln!("DATABASE_URL present but failed to connect");
    } else {
        eprintln!("No DATABASE_URL provided; running without DB");
    }

    let server_url = format!("http://{}:{}/", bind_addr, port);
    println!("Starting server at {}", server_url);

    // Build server and attach session middleware when we have a secret key.
    // Determine session key to use for SessionMiddleware. Prefer the OnceCell value,
    // otherwise fall back to environment or generate a new key.
    let session_key: Key = if let Some(k) = SECRET_KEY_CELL.get() {
        k.clone()
    } else if let Ok(secret_key_hex) = std::env::var("SECRET_KEY") {
        if let Ok(secret_bytes) = hex::decode(&secret_key_hex) {
            Key::from(&secret_bytes)
        } else {
            Key::generate()
        }
    } else {
        Key::generate()
    };

    let server = actix_web::HttpServer::new(move || {
        actix_web::App::new()
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), session_key.clone())
                    // Use secure cookies only when serving over HTTPS; for local HTTP dev set to false.
                    .cookie_secure(false)
                    .build(),
            )
            .configure(configure)
    })
    .bind((bind_addr.as_str(), port))?;

    // Run server and capture any error when it exits (for debugging)
    let run_result = server.run().await;
    eprintln!("Server run completed with: {:?}", run_result);
    run_result
}

// Try to extract host:port from a DATABASE_URL like postgres://user:pass@host:5432/db
fn extract_db_host(database_url: &str) -> Option<String> {
    // find '@'
    if let Some(at_idx) = database_url.find('@') {
        let after_at = &database_url[at_idx + 1..];
        // host:port is up to the next '/'
        if let Some(slash_idx) = after_at.find('/') {
            return Some(after_at[..slash_idx].to_string());
        } else {
            return Some(after_at.to_string());
        }
    }
    None
}
