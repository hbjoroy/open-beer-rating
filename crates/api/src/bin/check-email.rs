//! Check SMTP connectivity/authentication without sending any email.
#[tokio::main]
async fn main() {
    let success = match open_tappd_api::email_recovery::RecoveryConfig::from_env() {
        Ok(Some(config)) => config.smtp.test_connection().await.unwrap_or(false),
        _ => false,
    };
    if success {
        println!("SMTP connection and authentication verified; no email sent.");
    } else {
        eprintln!("SMTP configuration, TLS connection or authentication failed; no email sent.");
        std::process::exit(1);
    }
}
