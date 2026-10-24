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
    pub fn new(cfg: &SmtpConfig) -> anyhow::Result<Mailer> {
        let relay = if cfg.port == 465 { AsyncSmtpTransport::<Tokio1Executor>::relay } else { AsyncSmtpTransport::<Tokio1Executor>::starttls_relay };
        let transport = relay(&cfg.host)?
            .port(cfg.port)
            .credentials(Credentials::new(cfg.user.clone(), cfg.password.clone()))
            .timeout(Some(std::time::Duration::from_secs(20)))
            .build();
        let from = Mailbox::new(Some(crate::site::get().config.name.clone()), cfg.from.parse()?);
        Ok(Mailer { transport, from })
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
        let msg = Message::builder()
            .from(self.from.clone())
            .to(to.parse()?)
            .subject(subject)
            .multipart(MultiPart::alternative_plain_html(text, html))?;
        self.transport.send(msg).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lettre::address::Envelope;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

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
