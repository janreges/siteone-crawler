// SiteOne Crawler - MailerExporter
// (c) Jan Reges <jan.reges@siteone.cz>
//
// Sends crawl report via SMTP email using the lettre crate.

use std::sync::atomic::{AtomicBool, Ordering};

use lettre::message::{Attachment, MultiPart, SinglePart, header::ContentType};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};

use crate::error::{CrawlerError, CrawlerResult};
use crate::export::exporter::Exporter;
use crate::output::output::Output;
use crate::result::status::Status;
use crate::version;

/// Global flag to prevent sending emails when crawler is interrupted (CTRL+C).
static CRAWLER_INTERRUPTED: AtomicBool = AtomicBool::new(false);

pub fn set_crawler_interrupted(interrupted: bool) {
    CRAWLER_INTERRUPTED.store(interrupted, Ordering::SeqCst);
}

pub fn is_crawler_interrupted() -> bool {
    CRAWLER_INTERRUPTED.load(Ordering::SeqCst)
}

pub struct MailerExporter {
    /// Recipient email addresses (--mail-to, can be multiple)
    pub mail_to: Vec<String>,
    /// Sender email address (--mail-from)
    pub mail_from: String,
    /// Sender display name (--mail-from-name)
    pub mail_from_name: String,
    /// SMTP host (--mail-smtp-host)
    pub mail_smtp_host: String,
    /// SMTP port (--mail-smtp-port)
    pub mail_smtp_port: u16,
    /// SMTP username (--mail-smtp-user)
    pub mail_smtp_user: Option<String>,
    /// SMTP password (--mail-smtp-pass)
    pub mail_smtp_pass: Option<String>,
    /// Email subject template (--mail-subject-template)
    pub mail_subject_template: String,
    /// Initial host from crawled URL (for subject/body interpolation)
    pub initial_host: Option<String>,
    /// HTML report content to attach (set before export)
    pub html_report_content: Option<String>,
}

impl MailerExporter {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mail_to: Vec<String>,
        mail_from: String,
        mail_from_name: String,
        mail_smtp_host: String,
        mail_smtp_port: u16,
        mail_smtp_user: Option<String>,
        mail_smtp_pass: Option<String>,
        mail_subject_template: String,
        initial_host: Option<String>,
    ) -> Self {
        Self {
            mail_to,
            mail_from,
            mail_from_name,
            mail_smtp_host,
            mail_smtp_port,
            mail_smtp_user,
            mail_smtp_pass,
            mail_subject_template,
            initial_host,
            html_report_content: None,
        }
    }

    /// Set the HTML report content to be attached to the email.
    pub fn set_html_report_content(&mut self, content: String) {
        self.html_report_content = Some(content);
    }

    /// Build the email body HTML.
    fn get_email_body(&self, host: &str) -> String {
        let version_code = version::CODE;

        format!(
            r#"Hello,<br>
<br>
We are pleased to deliver the attached report detailing a thorough crawling and analysis of your website, <b>{host}</b>. Our advanced website crawler has identified key areas that require your attention, including found redirects, 404 error pages, and potential issues in accessibility, best practices, performance, and security.<br>
<br>
The report is in HTML format and for full functionality, it should be opened in a JavaScript-enabled browser. This will allow you to access advanced features such as searching and sorting data within tables. Some mobile email clients may not support all interactive elements.<br>
<br>
In case you have any suggestions for improvements and other useful features, feel free to send them as Feature requests to <a href="https://github.com/janreges/siteone-crawler/issues/">our project's GitHub</a>.<br>
<br>
Best regards,<br>
<br>
<a href="https://crawler.siteone.io/?utm_source=siteone_crawler&utm_medium=email-report&utm_campaign=crawler_report&utm_content=v{version_code}">SiteOne Crawler</a> Team"#,
            host = host,
            version_code = version_code,
        )
    }

    /// Add inline styles to the email body for better email client rendering.
    fn style_html_body_for_email(&self, html: &str) -> String {
        let styled_body = r#"<body style="font-family: Arial, Helvetica, sans-serif;">
<style>
table {
    border-collapse: collapse;
}
body table, body table th, body table td {
    border: 1px solid #555555;
    padding: 3px !important;
    vertical-align: top;
    text-align: left;
}
</style>
"#;
        html.replace("<body>", styled_body)
    }

    /// Build the email subject from the template.
    /// Replaces %domain%, %date%, %datetime% placeholders.
    fn build_subject(&self) -> String {
        let host = self.initial_host.as_deref().unwrap_or("unknown");
        let now = chrono::Local::now();
        let date = now.format("%Y-%m-%d").to_string();
        let datetime = now.format("%Y-%m-%d %H:%M").to_string();

        self.mail_subject_template
            .replace("%domain%", host)
            .replace("%date%", &date)
            .replace("%datetime%", &datetime)
    }

    /// Resolve the sender address.
    /// Replaces @your-hostname.com with @<actual-hostname>.
    fn resolve_mail_from(&self) -> String {
        let hostname = gethostname::gethostname().to_string_lossy().to_string();
        self.mail_from.replace("@your-hostname.com", &format!("@{}", hostname))
    }

    /// Send the email via SMTP using the lettre crate.
    fn send_email(
        &self,
        email_body_html: &str,
        attachment_filename: Option<&str>,
        attachment_content: Option<&str>,
    ) -> CrawlerResult<()> {
        let from_addr = self.resolve_mail_from();
        let subject = self.build_subject();

        // Build the message for the first recipient, then iterate
        if self.mail_to.is_empty() {
            return Err(CrawlerError::Mail("No recipients specified for email".to_string()));
        }

        // Parse from address
        let from_mailbox: lettre::message::Mailbox = format!("{} <{}>", self.mail_from_name, from_addr)
            .parse()
            .map_err(|e| CrawlerError::Mail(format!("Invalid sender address '{}': {}", from_addr, e)))?;

        // Build email body with optional attachment
        let styled_body = self.style_html_body_for_email(email_body_html);
        let email_body = SinglePart::builder().header(ContentType::TEXT_HTML).body(styled_body);

        let multipart = if let (Some(filename), Some(content)) = (attachment_filename, attachment_content) {
            let attachment = Attachment::new(filename.to_string()).body(
                content.as_bytes().to_vec(),
                "application/octet-stream"
                    .parse()
                    .map_err(|_| CrawlerError::Mail("Failed to parse MIME type for attachment".to_string()))?,
            );
            MultiPart::mixed().singlepart(email_body).singlepart(attachment)
        } else {
            MultiPart::mixed().singlepart(email_body)
        };

        // Send to each recipient
        for recipient in &self.mail_to {
            let to_mailbox = recipient
                .parse()
                .map_err(|e| CrawlerError::Mail(format!("Invalid recipient address '{}': {}", recipient, e)))?;

            let email = Message::builder()
                .from(from_mailbox.clone())
                .to(to_mailbox)
                .subject(&subject)
                .multipart(multipart.clone())
                .map_err(|e| CrawlerError::Mail(format!("Failed to build email message: {}", e)))?;

            // Build SMTP transport
            let mut smtp_builder = if self.mail_smtp_port == 465 {
                // Port 465 = implicit TLS
                SmtpTransport::relay(&self.mail_smtp_host)
                    .map_err(|e| {
                        CrawlerError::Mail(format!(
                            "Failed to connect to SMTP server '{}:{}': {}",
                            self.mail_smtp_host, self.mail_smtp_port, e
                        ))
                    })?
                    .port(self.mail_smtp_port)
            } else if self.mail_smtp_port == 587 {
                // Port 587 = STARTTLS
                SmtpTransport::starttls_relay(&self.mail_smtp_host)
                    .map_err(|e| {
                        CrawlerError::Mail(format!(
                            "Failed to connect to SMTP server '{}:{}': {}",
                            self.mail_smtp_host, self.mail_smtp_port, e
                        ))
                    })?
                    .port(self.mail_smtp_port)
            } else {
                // Other ports (25, etc) = no encryption by default
                SmtpTransport::builder_dangerous(&self.mail_smtp_host).port(self.mail_smtp_port)
            };

            // Add credentials if provided
            if let (Some(user), Some(pass)) = (&self.mail_smtp_user, &self.mail_smtp_pass) {
                smtp_builder = smtp_builder.credentials(Credentials::new(user.clone(), pass.clone()));
            }

            let mailer = smtp_builder.build();

            mailer
                .send(&email)
                .map_err(|e| CrawlerError::Mail(format!("Failed to send email to '{}': {}", recipient, e)))?;
        }

        Ok(())
    }
}

impl Exporter for MailerExporter {
    fn get_name(&self) -> &str {
        "MailerExporter"
    }

    fn should_be_activated(&self) -> bool {
        !self.mail_to.is_empty()
    }

    fn export(&mut self, status: &Status, _output: &dyn Output) -> CrawlerResult<()> {
        // Do not send emails if crawler was interrupted
        if is_crawler_interrupted() {
            return Ok(());
        }

        let host = self.initial_host.as_deref().unwrap_or("unknown");
        let datetime = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
        let email_body = self.get_email_body(host);
        let attachment_filename = format!("report-{}-{}.html", host, datetime);

        let html_report = match &self.html_report_content {
            Some(c) => c.clone(),
            None => {
                return Err(CrawlerError::Export(
                    "HTML report content not available. Set it via set_html_report_content() before export."
                        .to_string(),
                ));
            }
        };

        match self.send_email(&email_body, Some(&attachment_filename), Some(&html_report)) {
            Ok(()) => {
                let recipients = self.mail_to.join(", ");
                status.add_info_to_summary(
                    "mail-report-sent",
                    &format!(
                        "HTML report sent to {} using {}:{}",
                        recipients, self.mail_smtp_host, self.mail_smtp_port
                    ),
                );
            }
            Err(e) => {
                status.add_critical_to_summary("mail-report-failed", &format!("Failed to send email report: {}", e));
            }
        }

        Ok(())
    }
}
