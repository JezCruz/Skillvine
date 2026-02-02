# Notifications & Attachments

## Frontend viewers (teachers & students)
- Notice modals live in templates/teacher_dashboard.html and templates/student_dashboard.html.
- Attachment rendering flow:
  - Detect file type by URL path extension; if unknown, fallback to a HEAD request and inspect content-type.
  - Inline previews: video (mp4/webm/ogg), audio (mp3/wav/aac/m4a), PDF (object embed), Office docs (doc/docx/ppt/pptx/xls/xlsx as link with icon), images with fallback link on error.
  - Portrait or tall media is centered in a flex frame with max-height containment.
  - If detection fails, we show an image attempt; if that errors, we fall back to an "Open attachment" link.
- Key code: openNotice in templates/teacher_dashboard.html and templates/student_dashboard.html.
- Note: HEAD requests must be permitted by the server/storage. If blocked, previews rely on file extensions only.

## Admin notice management
- Listing/editing/deleting sent notices is in src/routes/admin.rs.
- Deleting a notice removes the DB row and best-effort deletes its stored attachment file path.
- Updating with remove_attachment=true clears attachment_path and attempts to delete the stored file.

## API surfaces
- GET /api/notifications: returns items with attachment_url (if present).
- POST /api/notifications/read/{id} and /api/notifications/read_all: mark as read.
- Admin-only:
  - GET /api/admin/notifications: list notices sent by the admin.
  - POST /api/admin/notifications/{id}/update: accepts optional title/body, remove_attachment boolean.
  - POST /api/admin/notifications/{id}/delete: deletes notice and attachment file.
  - POST /api/notifications (multipart): create notice with optional attachment (uses sender_id from session).

## Developer notes
- Attachments are stored via attachment_path in the notifications table; attachment_url is derived as /api/notifications/{id}/attachment.
- The attachment download/stream endpoint checks ownership/admin role: src/routes/notifications.rs -> GET /api/notifications/{id}/attachment.
- When adding new file types for inline preview, extend the type detection in the openNotice functions; prefer content-type hints for extensionless URLs.
- Default size limits: frontend warns at 25MB for support uploads; adjust server limits as needed.
