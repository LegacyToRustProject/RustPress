use lettre::{
    message::header::ContentType, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};

/// Configuration for the mail system.
#[derive(Debug, Clone)]
pub struct MailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password: String,
    pub from_name: String,
    pub from_email: String,
}

impl Default for MailConfig {
    fn default() -> Self {
        Self {
            smtp_host: "localhost".to_string(),
            smtp_port: 25,
            smtp_username: String::new(),
            smtp_password: String::new(),
            from_name: "WordPress".to_string(),
            from_email: "wordpress@localhost".to_string(),
        }
    }
}

/// WordPress-compatible mail sender.
pub struct WpMail {
    config: MailConfig,
}

impl WpMail {
    pub fn new(config: MailConfig) -> Self {
        Self { config }
    }

    /// Create from wp_options database settings.
    pub fn from_options(options: &std::collections::HashMap<String, String>) -> Self {
        let config = MailConfig {
            smtp_host: options
                .get("smtp_host")
                .cloned()
                .unwrap_or_else(|| "localhost".to_string()),
            smtp_port: options
                .get("smtp_port")
                .and_then(|s| s.parse().ok())
                .unwrap_or(25),
            smtp_username: options.get("smtp_username").cloned().unwrap_or_default(),
            smtp_password: options.get("smtp_password").cloned().unwrap_or_default(),
            from_name: options
                .get("blogname")
                .cloned()
                .unwrap_or_else(|| "WordPress".to_string()),
            from_email: options
                .get("admin_email")
                .cloned()
                .unwrap_or_else(|| "wordpress@localhost".to_string()),
        };
        Self::new(config)
    }

    /// Send an email (WordPress wp_mail equivalent).
    pub async fn wp_mail(
        &self,
        to: &str,
        subject: &str,
        message: &str,
        headers: Option<&str>,
    ) -> Result<(), MailError> {
        let content_type = if headers.is_some_and(|h| h.contains("Content-Type: text/html")) {
            ContentType::TEXT_HTML
        } else {
            ContentType::TEXT_PLAIN
        };

        let from = format!("{} <{}>", self.config.from_name, self.config.from_email);

        let email = Message::builder()
            .from(
                from.parse()
                    .map_err(|e| MailError::Build(format!("Invalid from: {e}")))?,
            )
            .to(to
                .parse()
                .map_err(|e| MailError::Build(format!("Invalid to: {e}")))?)
            .subject(subject)
            .header(content_type)
            .body(message.to_string())
            .map_err(|e| MailError::Build(e.to_string()))?;

        // Build SMTP transport
        let mut builder = if self.config.smtp_port == 465 {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&self.config.smtp_host)
                .map_err(|e| MailError::Transport(e.to_string()))?
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.config.smtp_host)
                .map_err(|e| MailError::Transport(e.to_string()))?
        };

        if !self.config.smtp_username.is_empty() {
            builder = builder.credentials(Credentials::new(
                self.config.smtp_username.clone(),
                self.config.smtp_password.clone(),
            ));
        }

        let mailer = builder.port(self.config.smtp_port).build();

        mailer
            .send(email)
            .await
            .map_err(|e| MailError::Send(e.to_string()))?;

        Ok(())
    }

    /// Send password reset email.
    pub async fn send_password_reset(
        &self,
        to: &str,
        user_login: &str,
        reset_url: &str,
    ) -> Result<(), MailError> {
        let subject = format!("[{}] Password Reset", self.config.from_name);
        let message = format!(
            "Someone has requested a password reset for the following account:\n\n\
             Site Name: {}\n\
             Username: {}\n\n\
             If this was a mistake, ignore this email and nothing will happen.\n\n\
             To reset your password, visit the following address:\n\n\
             {}\n\n\
             This password reset request originated from {}.",
            self.config.from_name, user_login, reset_url, self.config.from_name
        );
        self.wp_mail(to, &subject, &message, None).await
    }

    /// Send new user notification to admin.
    pub async fn send_new_user_notification(
        &self,
        admin_email: &str,
        user_login: &str,
        user_email: &str,
        role: &str,
    ) -> Result<(), MailError> {
        let subject = format!("[{}] New User Registration", self.config.from_name);
        let message = format!(
            "New user registration on your site {}:\n\n\
             Username: {}\n\
             Email: {}\n\
             Role: {}\n",
            self.config.from_name, user_login, user_email, role
        );
        self.wp_mail(admin_email, &subject, &message, None).await
    }

    /// Send comment notification.
    pub async fn send_comment_notification(
        &self,
        to: &str,
        post_title: &str,
        comment_author: &str,
        comment_content: &str,
        moderate_url: &str,
    ) -> Result<(), MailError> {
        let subject = format!("[{}] Comment: \"{}\"", self.config.from_name, post_title);
        let message = format!(
            "A new comment on the post \"{post_title}\" is waiting for your approval.\n\n\
             Author: {comment_author}\n\
             Comment:\n{comment_content}\n\n\
             Approve it: {moderate_url}\n"
        );
        self.wp_mail(to, &subject, &message, None).await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("Failed to build email: {0}")]
    Build(String),
    #[error("SMTP transport error: {0}")]
    Transport(String),
    #[error("Failed to send email: {0}")]
    Send(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mail_config_default() {
        let config = MailConfig::default();
        assert_eq!(config.smtp_host, "localhost");
        assert_eq!(config.smtp_port, 25);
    }

    #[test]
    fn test_from_options() {
        let mut options = std::collections::HashMap::new();
        options.insert("smtp_host".to_string(), "mail.example.com".to_string());
        options.insert("smtp_port".to_string(), "587".to_string());
        options.insert("blogname".to_string(), "My Blog".to_string());
        options.insert("admin_email".to_string(), "admin@example.com".to_string());

        let mailer = WpMail::from_options(&options);
        assert_eq!(mailer.config.smtp_host, "mail.example.com");
        assert_eq!(mailer.config.smtp_port, 587);
        assert_eq!(mailer.config.from_name, "My Blog");
        assert_eq!(mailer.config.from_email, "admin@example.com");
    }
}
