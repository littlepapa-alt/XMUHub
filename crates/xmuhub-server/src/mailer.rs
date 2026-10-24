//! Outbound mail (verification codes) through an authenticated SMTP relay —
//! Aliyun DirectMail on 465/implicit TLS, since the host cannot send on port 25. Other ports
//! (80, for hosts that block every mail port) use STARTTLS, which is required, never plain text.

use lettre::message::{Mailbox, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

/// Only an explicit destination-address rejection proves the mailbox is missing.
/// A plain 550 can also mean spam policy, authentication, or a full mailbox.
pub fn recipient_missing(error: &lettre::transport::smtp::Error) -> bool {
    use std::error::Error;

    if !error.is_permanent() {
        return false;
    }
    let Some(source) = error.source() else { return false };
    let reply = source.to_string().to_lowercase();
    // Enhanced status codes identify the destination mailbox / destination domain.
    // Read only the leading status token, never a quoted address or URL in the reply.
    if matches!(reply.split_whitespace().next(), Some("5.1.1" | "5.1.2")) {
        return true;
    }
    // Do not reinterpret explicit policy/authentication failures as address failures.
    if reply.split_whitespace().next().is_some_and(|s| s.starts_with("5.")) {
        return false;
    }
    ["user unknown", "unknown user", "no such user", "no such mailbox", "mailbox not found", "recipient does not exist", "recipient not found", "收件人不存在", "用户不存在"]
        .iter()
        .any(|reason| reply.contains(reason))
        && !reply.contains("sender")
}

pub struct Mailer {
    senders: Vec<Sender>,
}

struct Sender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub from: String,
}

impl Mailer {
    pub fn new<'a>(configs: impl IntoIterator<Item = &'a SmtpConfig>) -> anyhow::Result<Mailer> {
        let mut senders = Vec::new();
        for cfg in configs {
            let relay = if cfg.port == 465 { AsyncSmtpTransport::<Tokio1Executor>::relay } else { AsyncSmtpTransport::<Tokio1Executor>::starttls_relay };
            let transport = relay(&cfg.host)?
                .port(cfg.port)
                .credentials(Credentials::new(cfg.user.clone(), cfg.password.clone()))
                .timeout(Some(std::time::Duration::from_secs(20)))
                .build();
            let from = Mailbox::new(Some(crate::site::get().config.name.clone()), cfg.from.parse()?);
            senders.push(Sender { transport, from });
        }
        tracing::info!(senders = senders.len(), "SMTP sender accounts configured");
        Ok(Mailer { senders })
    }

    pub async fn send_code(&self, to: &str, code: &str, purpose: &str) -> anyhow::Result<()> {
        let site = &crate::site::get().config;
        let (name, subtitle, domain) = (&site.name, &site.subtitle, &site.domain);
        let (subject, action) = match purpose {
            "reset" => (format!("{name}找回密码验证码"), "重置密码"),
            _ => (format!("{name}注册验证码"), "注册账号"),
        };
        let text = format!(
            "你正在{action}，验证码是：{code}\n\n验证码 30 分钟内有效。如果不是你本人操作，请忽略这封邮件。\n\n—— {name} https://{domain}"
        );
        let html = format!(
            r#"<div style="font-family:-apple-system,'PingFang SC','Microsoft YaHei',sans-serif;max-width:480px;margin:0 auto;padding:24px;color:#182033">
<div style="font-size:20px;font-weight:700;color:#122e66;margin-bottom:16px">{name} · {subtitle}</div>
<p>你正在{action}，验证码是：</p>
<div style="font-size:32px;font-weight:700;letter-spacing:6px;background:#f0f2f7;border-radius:10px;padding:14px 0;text-align:center;margin:12px 0">{code}</div>
<p style="color:#5f687b;font-size:14px">验证码 30 分钟内有效。如果不是你本人操作，请忽略这封邮件。</p>
<p style="color:#8a93a6;font-size:12px;margin-top:24px">{name}是同学自发维护的非官方资料共享站 · <a href="https://{domain}" style="color:#1d4494">{domain}</a></p>
</div>"#
        );
        self.send_message(to, &subject, &text, &html).await
    }

    async fn send_message(&self, to: &str, subject: &str, text: &str, html: &str) -> anyhow::Result<()> {
        let to: Mailbox = to.parse()?;
        let mut last_error = None;
        for (index, sender) in self.senders.iter().enumerate() {
            // Each relay must use its own authorized From address. The recipient and
            // verification code are identical, even if an ambiguous timeout duplicates delivery.
            let msg = Message::builder()
                .from(sender.from.clone())
                .to(to.clone())
                .subject(subject)
                .multipart(MultiPart::alternative_plain_html(text.to_owned(), html.to_owned()))?;
            match sender.transport.send(msg).await {
                Ok(_) => {
                    if index > 0 {
                        tracing::info!(sender = index + 1, "verification mail accepted by backup SMTP");
                    }
                    return Ok(());
                }
                Err(error) => {
                    // Retrying a nonexistent destination with another sender cannot fix it.
                    if recipient_missing(&error) {
                        return Err(error.into());
                    }
                    tracing::warn!(sender = index + 1, backup_available = index + 1 < self.senders.len(), "SMTP delivery failed");
                    last_error = Some(error);
                }
            }
        }
        Err(last_error.map(anyhow::Error::from).unwrap_or_else(|| anyhow::anyhow!("no SMTP sender configured")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettre::address::Envelope;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    async fn fake_sender(from: &str, rejection: Option<&'static str>) -> (Sender, std::sync::Arc<std::sync::atomic::AtomicUsize>, tokio::task::JoinHandle<String>) {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let calls = Arc::new(AtomicUsize::new(0));
        let accepted = calls.clone();
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            accepted.fetch_add(1, Ordering::SeqCst);
            let mut stream = BufReader::new(stream);
            stream.get_mut().write_all(b"220 localhost test SMTP\r\n").await.unwrap();
            let mut message = String::new();
            let mut data = false;
            loop {
                let mut line = String::new();
                if stream.read_line(&mut line).await.unwrap() == 0 { break; }
                if data {
                    if line == ".\r\n" {
                        stream.get_mut().write_all(b"250 accepted\r\n").await.unwrap();
                        break;
                    }
                    message.push_str(&line);
                    continue;
                }
                let reply = if line.starts_with("RCPT TO:") && let Some(reason) = rejection {
                    format!("{reason}\r\n")
                } else if line.starts_with("DATA") {
                    data = true;
                    "354 send message\r\n".to_string()
                } else {
                    "250 localhost\r\n".to_string()
                };
                stream.get_mut().write_all(reply.as_bytes()).await.unwrap();
                if line.starts_with("RCPT TO:") && rejection.is_some() { break; }
            }
            message
        });
        let sender = Sender {
            transport: AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("127.0.0.1")
                .port(port)
                .timeout(Some(std::time::Duration::from_secs(2)))
                .build(),
            from: from.parse().unwrap(),
        };
        (sender, calls, task)
    }

    #[tokio::test]
    async fn backup_retries_delivery_failures_with_its_own_from_and_the_same_code() {
        use std::sync::atomic::Ordering;

        for rejection in ["550 The mail may contain inappropriate words or content.", "451 4.3.0 Try later", "535 5.7.8 Authentication failed"] {
            let (primary, _, first) = fake_sender("primary@example.org", Some(rejection)).await;
            let (backup, calls, second) = fake_sender("backup@example.net", None).await;
            let mailer = Mailer { senders: vec![primary, backup] };
            mailer.send_message("recipient@example.com", "Verification", "Code: 123456", "<p>123456</p>").await.unwrap();
            first.await.unwrap();
            let message = second.await.unwrap();
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert!(message.contains("From: backup@example.net"), "{message}");
            assert!(message.contains("To: recipient@example.com"));
            assert!(message.contains("123456"));
        }
    }

    #[tokio::test]
    async fn primary_success_or_missing_recipient_does_not_contact_backup() {
        use std::sync::atomic::Ordering;

        for rejection in [None, Some("550 5.1.1 User unknown")] {
            let (primary, _, first) = fake_sender("primary@example.org", rejection).await;
            let (backup, calls, second) = fake_sender("backup@example.net", None).await;
            let mailer = Mailer { senders: vec![primary, backup] };
            let result = mailer.send_message("recipient@example.com", "Verification", "123456", "123456").await;
            assert_eq!(result.is_ok(), rejection.is_none());
            if let Err(error) = result {
                assert!(recipient_missing(error.downcast_ref().unwrap()));
            }
            first.await.unwrap();
            assert_eq!(calls.load(Ordering::SeqCst), 0);
            second.abort();
        }
    }

    #[tokio::test]
    async fn both_senders_failing_preserves_the_final_smtp_error() {
        let (primary, _, first) = fake_sender("primary@example.org", Some("451 Temporary error")).await;
        let (backup, _, second) = fake_sender("backup@example.net", Some("550 5.1.1 User unknown")).await;
        let mailer = Mailer { senders: vec![primary, backup] };
        let error = mailer.send_message("recipient@example.com", "Verification", "123456", "123456").await.unwrap_err();
        assert!(recipient_missing(error.downcast_ref().unwrap()));
        first.await.unwrap();
        second.await.unwrap();
    }

    // Use real SMTP replies through lettre: its error display includes the 550 code,
    // while the enhanced status belongs to the underlying server response.
    #[tokio::test]
    async fn missing_recipient_is_distinct_from_policy_and_temporary_failures() {
        for (reply, missing) in [
            ("550 5.1.1 The email account that you tried to reach does not exist", true),
            ("550 5.1.2 Bad destination system address", true),
            ("550 User unknown", true),
            ("550 No such mailbox", true),
            ("550 收件人不存在", true),
            ("550 The mail may contain inappropriate words or content.", false),
            ("550 5.7.1 Rejected by spam policy", false),
            ("550 5.7.1 Policy rejection: user unknown", false),
            ("550 5.2.2 Mailbox full", false),
            ("550 5.2.1 Mailbox disabled", false),
            ("550 Mailbox unavailable", false),
            ("550 Sender rejected: user unknown", false),
            ("450 4.1.1 User unknown, try again later", false),
            ("535 5.7.8 Authentication credentials invalid", false),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = tokio::spawn(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let mut stream = BufReader::new(stream);
                stream.get_mut().write_all(b"220 localhost test SMTP\r\n").await.unwrap();
                let mut line = String::new();
                loop {
                    line.clear();
                    if stream.read_line(&mut line).await.unwrap() == 0 {
                        break;
                    }
                    if line.starts_with("RCPT TO:") {
                        stream.get_mut().write_all(format!("{reply}\r\n").as_bytes()).await.unwrap();
                        break;
                    }
                    stream.get_mut().write_all(b"250 localhost\r\n").await.unwrap();
                }
            });
            // Plain SMTP is restricted to this local fake server; production always uses TLS.
            let transport = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("127.0.0.1")
                .port(port)
                .timeout(Some(std::time::Duration::from_secs(2)))
                .build();
            let envelope = Envelope::new(Some("sender@example.org".parse().unwrap()), vec!["recipient@example.org".parse().unwrap()]).unwrap();
            let error = transport.send_raw(&envelope, b"test").await.unwrap_err();
            assert_eq!(recipient_missing(&error), missing, "{reply}: {error}");
            server.await.unwrap();
        }
    }
}
