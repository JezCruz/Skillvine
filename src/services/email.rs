use anyhow::Result;
use dotenv::dotenv;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{message::Mailbox, Message, SmtpTransport, Transport};
use std::env; // Make sure to add `dotenv = "0.15"` in Cargo.toml

pub fn send_verification_email(to: &str, token: &str) -> Result<()> {
    // Load environment variables from .env file
    dotenv().ok();

    // Get Gmail credentials from .env
    let gmail_username = env::var("GMAIL_USERNAME").expect("GMAIL_USERNAME must be set in .env");
    let gmail_app_password =
        env::var("GMAIL_APP_PASSWORD").expect("GMAIL_APP_PASSWORD must be set in .env");

    // Construct the verification URL
    let verify_url = format!("http://127.0.0.1:8080/verify?token={}", token);

    // Create the HTML body
    let html_body = format!(
        r#"
        <html>
          <body>
            <h2>Verify your email</h2>
            <p>Click the button below to verify your account:</p>
            <a href="{}" 
               style="display:inline-block;
                      background:#007BFF;
                      color:white;
                      padding:10px 20px;
                      text-decoration:none;
                      border-radius:5px;">
                Verify Email
            </a>
            <p>If you didn't request this, ignore this email.</p>
          </body>
        </html>
    "#,
        verify_url
    );

    // Build the email
    let email = Message::builder()
        .from(gmail_username.parse::<Mailbox>()?)
        .to(to.parse::<Mailbox>()?)
        .subject("Verify your email")
        .header(lettre::message::header::ContentType::TEXT_HTML)
        .body(html_body)?;

    // Setup the Gmail SMTP transport
    let mailer = SmtpTransport::relay("smtp.gmail.com")?
        .credentials(Credentials::new(gmail_username, gmail_app_password))
        .build();

    // Send the email
    mailer.send(&email)?;
    Ok(())
}
