# Developer Guide

## Environment
- DB: PostgreSQL. Required: DATABASE_URL.
- Sessions/cookies: check src/config for key names; ensure secure keys in production.
- Local example (PowerShell): `$env:DATABASE_URL="postgres://skillvine_user:skillvine_user_2025@localhost:5432/skillvine_db"`.

## Setup
1) Install Rust toolchain and sqlx-cli.
2) `sqlx migrate run` (with DATABASE_URL set).
3) `cargo run` to start the server.
4) Assets: served from /static; templates from /templates. Ensure uploads/ exists and is writable.

## Code structure
- src/main.rs: server bootstrap and route wiring.
- src/routes/: HTTP endpoints (auth, notifications, admin, profile, etc.).
- src/controllers/: request handling logic.
- src/services/: business logic (email, session, token, wallet, etc.).
- src/models/: DB models (user, teacher, service, transaction).
- src/config/: env/config loaders.
- src/utils/logging.rs: logging helpers.
- templates/: HTML pages (dashboards, login, signup, etc.).
- static/: CSS/JS.
- uploads/: stored attachments (profile pics, teacher IDs, notices/support attachments).

## Notifications & attachments
- APIs: see README key APIs and docs/notifications.md for full flow.
- WebSocket: /ws/notifications, backed by tokio::broadcast.
- Frontend notice viewers: templates/teacher_dashboard.html and templates/student_dashboard.html.
  - Type detection by path extension; HEAD request fallback to content-type.
  - Inline preview: video/audio/PDF; Office docs via link; images with fallback link on error.
  - Portrait media centered in a flex frame (attachment-frame). Fallback link always available.
- Storage: notifications.attachment_path; served via GET /api/notifications/{id}/attachment (owner or admin).
- Admin delete/update remove stored files best-effort when removing attachments.

## Support requests
- User submissions allow optional attachment (UI hints 25MB). Stored in DB with TIMESTAMPTZ created_at.
- Admin routes serve attachments (see src/routes/admin.rs).

## Auth/Profile
- Profile endpoints: /api/profile, /api/update_profile, /api/change_password, /api/upload_avatar.
- KYC flow (teacher dashboard) hooks into backend; status reflected in UI.

## Wallet/Transactions
- Logic in src/controllers/wallet.rs, src/services/wallet_service.rs, src/models/transaction.rs. Review before changes.

## Running & testing
- `cargo check` for fast validation.
- `cargo test` when tests are added.
- For runtime verification, run `cargo run` and exercise dashboards (teacher/student/admin) for notifications and attachments.

## Deployment notes
- Set DATABASE_URL and session/crypto keys.
- Ensure uploads/ exists and is writable or replace with object storage (adjust attachment_path writes and URLs).
- If behind HTTPS proxy, allow WebSocket upgrade on /ws/notifications.

## Extending
- To add new preview types, extend openNotice in teacher_dashboard.html and student_dashboard.html.
- To switch storage to cloud, adjust attachment_path handling in src/routes/admin.rs and src/routes/notifications.rs and update URL builder.
- To add roles/permissions, modify ensure_admin and related guards.

## Housekeeping
- Attachments delete is best-effort; consider periodic cleanup for orphaned files.
- HEAD fallback for type detection may be blocked by some storages/CDNs; in that case, rely on extensions or add server-side content-type hints.
