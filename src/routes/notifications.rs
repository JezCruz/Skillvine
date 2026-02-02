use actix::{Actor, ActorContext, AsyncContext, StreamHandler};
use actix_files::NamedFile;
use actix_session::Session;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use actix_web_actors::ws;
use chrono::{DateTime, NaiveDateTime, Utc};
use once_cell::sync::OnceCell;
use serde::Serialize;
use serde_json::json;
use sqlx::Row;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::broadcast;

use crate::POOL_DATA;

#[derive(Clone, Debug, Serialize)]
pub struct NotificationEvent {
    pub id: i32,
    pub user_id: i32,
    pub title: String,
    pub body: String,
    pub attachment_url: Option<String>,
    pub created_at: String,
    pub read: bool,
}

static CHANNEL: OnceCell<broadcast::Sender<NotificationEvent>> = OnceCell::new();

fn channel() -> broadcast::Sender<NotificationEvent> {
    CHANNEL
        .get_or_init(|| {
            let (tx, _rx) = broadcast::channel(200);
            tx
        })
        .clone()
}

pub fn broadcast_notification(event: NotificationEvent) {
    let _ = channel().send(event);
}

fn subscribe() -> broadcast::Receiver<NotificationEvent> {
    channel().subscribe()
}

struct NotificationWs {
    user_id: i32,
    rx: broadcast::Receiver<NotificationEvent>,
}

impl Actor for NotificationWs {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // Poll for events frequently so clients get near-real-time updates
        ctx.run_interval(Duration::from_millis(800), |act, ctx| {
            while let Ok(evt) = act.rx.try_recv() {
                if evt.user_id == act.user_id {
                    if let Ok(text) = serde_json::to_string(&evt) {
                        ctx.text(text);
                    }
                }
            }
        });

        // Heartbeat pings to keep the connection alive
        ctx.run_interval(Duration::from_secs(20), |_act, ctx| {
            ctx.ping(b"hb");
        });
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for NotificationWs {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Pong(_)) => {}
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            Ok(ws::Message::Text(_)) => {}
            Ok(ws::Message::Binary(_)) => {}
            Err(_) => ctx.stop(),
            _ => {}
        }
    }
}

#[get("/ws/notifications")]
pub async fn ws_notifications(
    req: HttpRequest,
    stream: web::Payload,
    session: Session,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = session
        .get::<i32>("user_id")
        .unwrap_or(None)
        .ok_or_else(|| actix_web::error::ErrorUnauthorized("not logged in"))?;

    let actor = NotificationWs {
        user_id,
        rx: subscribe(),
    };

    ws::start(actor, &req, stream)
}

#[get("/api/notifications")]
async fn list_notifications(session: Session) -> impl Responder {
    let user_id = match session.get::<i32>("user_id").unwrap_or(None) {
        Some(id) => id,
        None => return HttpResponse::Unauthorized().json(json!({"error": "not logged in"})),
    };

    let pool_data = match POOL_DATA.get() {
        Some(p) => p,
        None => return HttpResponse::InternalServerError().json(json!({"error": "no db"})),
    };

    let rows = sqlx::query(
        "SELECT id, title, body, attachment_path, created_at, read FROM notifications WHERE user_id = $1 ORDER BY created_at DESC LIMIT 50",
    )
    .bind(user_id)
    .fetch_all(pool_data.get_ref())
    .await;

    match rows {
        Ok(list) => {
            let mapped: Vec<serde_json::Value> = list
                .into_iter()
                .map(|r| {
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
                        "read": r.get::<bool,_>("read"),
                    })
                })
                .collect();
            HttpResponse::Ok().json(json!({"items": mapped}))
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)})),
    }
}

#[post("/api/notifications/read/{id}")]
async fn mark_read(path: web::Path<i32>, session: Session) -> impl Responder {
    let user_id = match session.get::<i32>("user_id").unwrap_or(None) {
        Some(id) => id,
        None => return HttpResponse::Unauthorized().json(json!({"error": "not logged in"})),
    };
    let notif_id = *path;

    let pool_data = match POOL_DATA.get() {
        Some(p) => p,
        None => return HttpResponse::InternalServerError().json(json!({"error": "no db"})),
    };

    let res = sqlx::query("UPDATE notifications SET read = TRUE WHERE id = $1 AND user_id = $2")
        .bind(notif_id)
        .bind(user_id)
        .execute(pool_data.get_ref())
        .await;

    match res {
        Ok(_) => HttpResponse::Ok().json(json!({"ok": true})),
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)})),
    }
}

#[post("/api/notifications/read_all")]
async fn mark_all_read(session: Session) -> impl Responder {
    let user_id = match session.get::<i32>("user_id").unwrap_or(None) {
        Some(id) => id,
        None => return HttpResponse::Unauthorized().json(json!({"error": "not logged in"})),
    };

    let pool_data = match POOL_DATA.get() {
        Some(p) => p,
        None => return HttpResponse::InternalServerError().json(json!({"error": "no db"})),
    };

    let res = sqlx::query("UPDATE notifications SET read = TRUE WHERE user_id = $1")
        .bind(user_id)
        .execute(pool_data.get_ref())
        .await;

    match res {
        Ok(_) => HttpResponse::Ok().json(json!({"ok": true})),
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": format!("db: {}", e)})),
    }
}

#[get("/api/notifications/{id}/attachment")]
async fn notification_attachment(
    path: web::Path<i32>,
    session: Session,
    req: HttpRequest,
) -> actix_web::Result<HttpResponse> {
    let user_id = session
        .get::<i32>("user_id")
        .unwrap_or(None)
        .ok_or_else(|| actix_web::error::ErrorUnauthorized("not logged in"))?;
    let role = session.get::<String>("role").unwrap_or(None).unwrap_or_default();
    let notif_id = *path;

    let pool_data = POOL_DATA
        .get()
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("no db"))?;

    let row = sqlx::query(
        "SELECT user_id, attachment_path FROM notifications WHERE id = $1",
    )
    .bind(notif_id)
    .fetch_optional(pool_data.get_ref())
    .await
    .map_err(|e| actix_web::error::ErrorInternalServerError(e.to_string()))?;

    if let Some(r) = row {
        let owner: i32 = r.get("user_id");
        if owner != user_id && role != "admin" {
            return Err(actix_web::error::ErrorForbidden("forbidden"));
        }
        let path: Option<String> = r.try_get("attachment_path").ok();
        if let Some(p) = path {
            let fs_path = PathBuf::from(&p);
            if fs_path.exists() {
                let file = NamedFile::open_async(fs_path).await?;
                return Ok(file.into_response(&req));
            }
        }
    }

    Err(actix_web::error::ErrorNotFound("not found"))
}

pub fn init(cfg: &mut web::ServiceConfig) {
    // Ensure channel is created at startup
    let _ = channel();
    cfg.service(ws_notifications)
        .service(list_notifications)
        .service(mark_read)
        .service(mark_all_read)
        .service(notification_attachment);
}
