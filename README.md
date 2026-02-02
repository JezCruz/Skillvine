
Folder Structure:

Skillvine/               <-- Root of your Rust project (Cargo.toml here)
├── Cargo.toml           <-- Rust package file
├── src/                 <-- Rust source code
│   ├── main.rs          <-- Entry point
│   ├── lib.rs           <-- Optional, for modules
│   ├── config/          <-- App configuration (DB, env variables)
│   │   └── mod.rs
│   ├── controllers/     <-- Handles HTTP requests / API endpoints
│   │   ├── auth.rs
│   │   ├── sessions.rs
│   │   ├── wallet.rs
│   │   └── teachers.rs
│   ├── models/          <-- Database models / ORM structs
│   │   ├── user.rs
│   │   ├── teacher.rs
│   │   ├── service.rs
│   │   └── transaction.rs
│   ├── services/        <-- Business logic (coins deduction, ratings calculation)
│   │   ├── wallet_service.rs
│   │   └── session_service.rs
│   ├── routes/          <-- Route definitions
│   │   └── mod.rs
│   ├── utils/           <-- Helper functions
│   │   └── mod.rs
│   └── db.rs            <-- Database connection and pool
├── templates/           <-- HTML templates for web rendering
│   ├── index.html
│   ├── login.html
│   ├── dashboard.html
│   └── services.html
├── static/              <-- Static files (CSS, JS, images)
│   ├── css/
│   ├── js/
│   └── images/
├── uploads/             <-- Uploaded files (IDs, profile pics, session attachments)
│   ├── teacher_ids/
│   └── profile_pics/
├── migrations/          <-- Database migrations (if using Diesel or SeaORM)
├── .env                 <-- Environment variables (DB URL, secret keys)
└── README.md


Key Notes:

src/controllers/ → Handles incoming HTTP requests (API endpoints). Think of it as “route logic.”

src/models/ → Database representations; each table has a corresponding Rust struct.

src/services/ → Business logic, like automatic coin deductions, 70/30 split, ratings computation.

templates/ → For rendering HTML pages (if you plan a server-rendered web app).

static/ → Frontend assets.

uploads/ → Never put sensitive files in static; store ID scans securely here, ideally linked to cloud storage (AWS S3 or Google Cloud Storage) in production.

.env → Store DB credentials, API keys, secrets here. Never commit this to Git.

*******************************************************************
💡 Optional for bigger projects:

src/middleware/ → JWT authentication, logging, rate limiting.

src/jobs/ → Background jobs like payouts, notifications.

src/api/ → If you separate internal/external API logic.
*******************************************************************


Email Verification Flowchart (for Actix Web)

 ┌─────────────────────────┐
 │ User signs up (POST /register) │
 └──────────────┬────────────┘
                │
                ▼
     ┌────────────────────────────┐
     │ 1. Generate verification token │
     │    (services/token.rs)        │
     └──────────────┬──────────────┘
                    │
                    ▼
     ┌────────────────────────────┐
     # Skillvine

     Rust + Actix Web app with PostgreSQL, server-rendered dashboards, and real-time notifications for students, teachers, and admins.

     ## Features
     - Authenticated dashboards for students/teachers; admin panel for managing notices.
     - Real-time notifications via WebSocket (/ws/notifications).
     - Rich attachment previews (video, audio, PDF, Office doc link, images) with portrait-friendly centering and fallback links.
     - Support requests with optional attachments.
     - Avatar/KYC/profile flows; wallet/services modules.

     ## Quickstart
     1) Set env: `set DATABASE_URL=postgres://skillvine_user:skillvine_user_2025@localhost/skillvine_db` (PowerShell: `$env:DATABASE_URL="..."`).
     2) Migrations: `sqlx migrate run`.
     3) Run: `cargo run` (from repo root).
     4) Open the app at the configured bind address (see main.rs; typically 127.0.0.1:PORT).

     ## Project layout
     - Backend: src/ (routes, controllers, services, models, config, utils, db.rs).
     - Frontend: templates/ (HTML) and static/ (CSS/JS/assets).
     - Data: migrations/ (SQLx), uploads/ (attachment storage), docs/ (project docs).

     ## Docs
     - Project overview: docs/project_overview.md
     - Notifications & attachments: docs/notifications.md
     - Developer guide: developer.md (see root)

     ## Key APIs
     - GET /api/notifications, POST /api/notifications/read/{id}, /read_all
     - GET /api/notifications/{id}/attachment (owner/admin)
     - Admin: GET /api/admin/notifications; POST /api/admin/notifications/{id}/update; POST /api/admin/notifications/{id}/delete
     - WebSocket: /ws/notifications

     ## Notes
     - Attachments are stored on disk (uploads/) with attachment_path; admin deletes attempt to remove the file.
     - Inline preview falls back to link if detection fails or HEAD is blocked.
     - Ensure uploads/ subfolders are writable in your environment.


| File                | Responsibility                            | Calls / Depends On                    |
