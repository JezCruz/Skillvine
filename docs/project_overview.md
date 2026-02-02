# Skillvine Project Overview

## 1) Stack
- Backend: Rust (Actix Web, SQLx, actix-session, actix-identity).
- DB: PostgreSQL with SQLx migrations.
- Frontend: Server-rendered HTML templates (teacher_dashboard.html, student_dashboard.html, admin dashboards) + vanilla JS and CSS in /static.
- Auth/session: Cookie-based sessions.
- Realtime: WebSocket notifications (/ws/notifications) broadcast via tokio::broadcast.

## 2) Project Layout (key paths)
- src/main.rs, src/lib.rs: App bootstrap and route registration.
- src/config/: Configuration loading (env vars) and setup utilities.
- src/routes/: HTTP routes (auth, profile, notifications, admin, etc.).
- src/controllers/: Auth/session/teacher/wallet controllers.
- src/models/: Data models for user, teacher, service, transaction, etc.
- src/services/: Email, session, token, wallet helpers.
- src/utils/logging.rs: Logging helpers.
- templates/: HTML templates for dashboards and pages.
- static/: CSS/JS/images assets.
- migrations/: SQLx migrations for schema.
- scripts/: Helper scripts (start_server.ps1, dump_env.ps1).

## 3) Runtime & Environment
- Primary env vars: DATABASE_URL (Postgres). Sessions/cookies rely on configured keys (check src/config for current names). Set DATABASE_URL before running migrations or the server.
- Migrations: `sqlx migrate run` (already used in workflow).
- Run server: `cargo run` from repo root.
- Build/test: `cargo check` / `cargo test` (add tests as needed).

## 4) Notifications & Attachments
- API
  - GET /api/notifications: List user notices (attachment_url included when present).
  - POST /api/notifications/read/{id}, POST /api/notifications/read_all: Mark read.
  - GET /api/notifications/{id}/attachment: Serves attachment if owner or admin.
  - WebSocket /ws/notifications: Push new NotificationEvent to the user.
- Admin
  - GET /api/admin/notifications: List notices sent by the admin.
  - POST /api/admin/notifications/{id}/update: Update title/body; remove_attachment deletes stored file.
  - POST /api/admin/notifications/{id}/delete: Deletes notice and best-effort removes stored attachment.
  - POST /api/notifications (multipart): Create notice with optional attachment (sender_id from session).
- Frontend (teacher & student dashboards)
  - Notice modal with full-view body.
  - Attachment rendering with detection by extension and HEAD content-type fallback: inline video/audio/PDF; Office docs as link with icon; images with fallback link. Portrait media centered in a flex frame.

## 5) Support Requests
- Users can submit support/issue reports with optional attachment (<=25MB hinted on UI). Stored in DB with created_at as TIMESTAMPTZ; admin can fetch attachment via admin routes (see src/routes/admin.rs).

## 6) Auth & Profile
- Profile fetch/update endpoints in /api/profile and /api/update_profile.
- Password change /api/change_password.
- Avatar upload /api/upload_avatar (image).
- KYC flow (teacher dashboard) with status UI; KYC submission endpoint exists in backend controllers/services (see src/controllers/auth.rs and services/token/session if needed).

## 7) Wallet/Transactions (high level)
- Controllers/services exist (src/controllers/wallet.rs, src/services/wallet_service.rs, src/models/transaction.rs). Review those files for exact flows (deposits/withdrawals/ledger) before modifying.

## 8) File Storage Model
- Notifications table stores attachment_path (server file path); attachment_url is derived for clients.
- Deleting or removing attachments from admin endpoints attempts to delete the on-disk file.
- Support request attachments stored similarly; served via admin attachment endpoint.

## 9) Frontend Assets
- CSS under static/css and static/styles.css; page-specific CSS in static/css/{auth,dashboard,student,teacher}.css.
- JS under static/js for auth, flash messages, login, profile manager, signup, etc.
- Templates under templates/ (home, login, signup, dashboards, profile manager, etc.).

## 10) Migrations
- Located in migrations/. Examples:
  - 20251212000000_create_users_table.sql
  - 20251111181918_add_full_name_to_users.sql
- Run with `sqlx migrate run` using DATABASE_URL.

## 11) Running Locally (Windows-friendly)
1. Set DATABASE_URL env var (example):
   - PowerShell: `$env:DATABASE_URL="postgres://skillvine_user:skillvine_user_2025@localhost/skillvine_db"`
2. Run migrations: `sqlx migrate run`.
3. Start app: `cargo run`.
4. Open browser to the configured host/port (check main.rs for bind address; typically 127.0.0.1:PORT).

## 12) Deployment Notes
- Ensure DATABASE_URL and session keys are set in the target environment.
- Serve static/ templates from the configured paths; ensure upload directories (e.g., uploads/, uploads/profile_pics/, uploads/teacher_ids/) exist and are writable by the app.
- If using a reverse proxy with HTTPS, confirm WebSocket pass-through for /ws/notifications.

## 13) Extending / Modifying
- To add new attachment preview types: extend openNotice in teacher_dashboard.html and student_dashboard.html.
- To change storage backends (e.g., S3/Azure Blob), adapt attachment_path writes/reads in src/routes/admin.rs and src/routes/notifications.rs, and update the attachment_url generator.
- To add new roles/permissions, adjust session role checks in route guards (ensure_admin and related helpers).

## 14) Known UX details
- Portrait media centered with max-height to avoid overflow in notice modals.
- Fallback link always available if inline preview fails.
- HEAD requests for type detection may be blocked by some storage/CDN configs; in that case previews rely on file extensions only.

## 15) To-Do / Gaps (fill as you proceed)
- Document exact env vars for session keys, email provider settings, and any third-party API keys (check src/config).
- Add automated tests (unit/integration) for routes and services.
- Add rate limiting / input validation hardening where needed (auth, uploads, support requests).
- Consider background cleanup for orphaned attachments if deletes ever fail.
