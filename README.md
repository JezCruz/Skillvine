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
     │ 2. Save token + user to DB │
     │    (models/user.rs)        │
     └──────────────┬──────────────┘
                    │
                    ▼
     ┌────────────────────────────┐
     │ 3. Send verification email │
     │    (services/email.rs)     │
     │    -> includes URL:        │
     │       https://yourapp.com/verify?token=XYZ │
     └──────────────┬──────────────┘
                    │
                    ▼
          📧 User receives email
                    │
                    ▼
     ┌────────────────────────────┐
     │ 4. User clicks verify link │
     │    (GET /verify?token=XYZ) │
     │    handled by:             │
     │    routes/verify.rs        │
     └──────────────┬──────────────┘
                    │
                    ▼
     ┌────────────────────────────┐
     │ 5. Backend validates token │
     │    (services/token.rs)     │
     │    and marks user verified │
     └──────────────┬──────────────┘
                    │
                    ▼
     ┌────────────────────────────┐
     │ 6. Responds with success   │
     │    page (HTML or redirect) │
     └────────────────────────────┘
     
🧩 How the Files Connect

| File                | Responsibility                            | Calls / Depends On                    |
| ------------------- | ----------------------------------------- | ------------------------------------- |
| `main.rs`           | Starts Actix server and routes            | `routes::auth`, `routes::verify`      |
| `routes/auth.rs`    | Handles signup (register) endpoint        | `services::token`, `services::email`  |
| `services/token.rs` | Creates and validates verification tokens | Used by `auth.rs` and `verify.rs`     |
| `services/email.rs` | Sends the actual HTML email               | Called by `auth.rs`                   |
| `routes/verify.rs`  | Handles `/verify?token=XYZ` endpoint      | Uses `services::token` and updates DB |
| `models/user.rs`    | Defines user structure and DB interaction | Used by `auth.rs` and `verify.rs`     |

**Secrets & .env (Important)**

- Do NOT commit real secrets into the repository. Keep an `.env` with placeholders and use a separate `.env.local` for real local secrets.
- This repository includes `.env.example` showing required keys. Copy it to `.env` and edit values for shared/default development values.
- Real secrets should be stored in `.env.local` and **must** be gitignored. We added `.env` and `.env.local` to `.gitignore`.
- If secrets were ever committed to git, rotate them immediately (DB password, email app password, AWS keys, session keys).

Quick commands:

```
# create a local copy from example
cp .env.example .env

# keep real secrets local (gitignored)
# edit .env.local and do NOT commit it
```

Security checklist before production:
- Use a secrets manager (AWS Secrets Manager, Azure Key Vault, HashiCorp Vault).
- Use HTTPS in production and enforce secure cookies.
- Use `sslmode=require` for Postgres connections when appropriate.
- Rotate keys if they are ever exposed.

