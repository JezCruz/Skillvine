# Admin Panel Guide

## Overview
The admin dashboard is a single page with tabbed sections for KYC, users, notices, and support requests. Tabs live in templates/admin_dashboard.html and are toggled client-side.

## Sections
- **KYC**: Filter, view, approve/reject (bulk or single). Drawer shows details and lets you add an admin note. Bulk actions respect selected rows.
- **Users**: View role/active/verified. Change role, toggle active, impersonate, reset password.
- **Send Notice**: Search a user, compose title/body, optional attachment; sends via /api/notifications with sender_id=admin session.
- **Sent Notices**: Lists admin-sent notices; edit/delete available. Deletes also best-effort delete stored attachment file.
- **Support Requests**: New vs Done buckets; attachments accessible via admin routes.

## Key endpoints (admin)
- KYC: /api/admin/kyc_requests, /bulk_decision, /kyc_export
- Users: /api/admin/users, /users/{id}/role, /users/{id}/active, /users/{id}/reset_password, /admin/impersonate
- Notices: /api/admin/notifications, /notifications/{id}/update, /notifications/{id}/delete, POST /api/notifications (create)
- Support: /api/admin/support_requests and attachment routes in src/routes/admin.rs

## Attachments
- Stored on disk via attachment_path; attachment_url derived for serving. Admin delete/update will attempt to remove the file.
- Frontend notice viewers support inline previews (video/audio/PDF, Office via link, images) with portrait-friendly centering.

## Running locally
- Set DATABASE_URL to Postgres.
- Run migrations: sqlx migrate run.
- Start: cargo run.
- Ensure uploads/ exists and is writable for attachments.

## Extending
- To add a new admin section, mirror the tab pattern: add a button with data-target, wrap section in .view-section with matching data-section, and reuse activateSection.
- To change storage to cloud, update attachment_path handling in src/routes/admin.rs and src/routes/notifications.rs, and adjust URL building.
- To add more inline preview types, update openNotice in teacher_dashboard.html and student_dashboard.html.
