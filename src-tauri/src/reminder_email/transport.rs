//! TLS保持证书验证；只向外返回经过分类的固定文案，不暴露服务端响应或凭据。
use super::ConnectionConfig;
use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use std::time::Duration;

#[derive(Debug)]
pub(super) struct Failure {
    pub message: String,
    pub retryable: bool,
    pub uncertain: bool,
}
impl Failure {
    pub(super) fn permanent(message: &str) -> Self {
        Self {
            message: message.into(),
            retryable: false,
            uncertain: false,
        }
    }
}
fn classify(error: lettre::transport::smtp::Error, sending: bool) -> Failure {
    let status = error.status().map(|v| v.to_string());
    if status
        .as_ref()
        .is_some_and(|v| v.starts_with("53") || v.starts_with("50"))
    {
        return Failure::permanent(
            "SMTP账号或授权码不匹配，或尚未开启SMTP服务；请按授权码引导检查后重新验证",
        );
    }
    if error.is_permanent() {
        return Failure::permanent("SMTP服务器拒绝发送，请检查发件地址、收件地址和邮箱的发信限制");
    }
    if error.is_transient() {
        return Failure {
            message: "SMTP服务器暂时无法接收邮件，将有限重试".into(),
            retryable: true,
            uncertain: false,
        };
    }
    if sending {
        return Failure {
            message:
                "发送过程中连接中断，无法确定服务器是否已接收；请先检查收件箱，再决定是否重新发送"
                    .into(),
            retryable: false,
            uncertain: true,
        };
    }
    Failure {
        message: "无法连接或验证SMTP服务，请检查服务器、端口、加密方式和网络；部分邮箱需先开启SMTP"
            .into(),
        retryable: true,
        uncertain: false,
    }
}
pub(super) async fn send(
    config: &ConnectionConfig,
    secret: &str,
    prepare: impl FnOnce() -> Result<(String, String), Failure>,
) -> Result<(), Failure> {
    let from = config
        .from
        .parse()
        .map_err(|_| Failure::permanent("发件邮箱无效，请重新配置"))?;
    let to = config
        .to
        .parse()
        .map_err(|_| Failure::permanent("收件邮箱无效，请重新配置"))?;
    let builder = if config.encryption == "starttls" {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
    }
    .map_err(|_| Failure::permanent("SMTP加密配置无效，请检查服务器和证书"))?;
    let mailer = builder
        .port(config.port)
        .credentials(Credentials::new(config.username.clone(), secret.to_owned()))
        .timeout(Some(Duration::from_secs(20)))
        .build();
    deliver(&mailer, || {
        let (subject, body) = prepare()?;
        Message::builder()
            .from(Mailbox::new(Some("颜喵提醒".into()), from))
            .to(Mailbox::new(None, to))
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            .body(body)
            .map_err(|_| Failure::permanent("邮件格式无效，请检查提醒内容和邮箱配置"))
    })
    .await
}
async fn deliver(
    mailer: &AsyncSmtpTransport<Tokio1Executor>,
    prepare: impl FnOnce() -> Result<Message, Failure>,
) -> Result<(), Failure> {
    // 单独探测连接/认证，使连接失败可安全重试；提交正文阶段的未知IO错误不盲目重试。
    let connected = tokio::time::timeout(Duration::from_secs(30), mailer.test_connection())
        .await
        .map_err(|_| Failure {
            message: "SMTP连接超时，请检查网络和SMTP设置".into(),
            retryable: true,
            uncertain: false,
        })?
        .map_err(|e| classify(e, false))?;
    if !connected {
        return Err(Failure::permanent("SMTP服务未正常响应，请确认已开启SMTP"));
    }
    let message = prepare()?;
    tokio::time::timeout(Duration::from_secs(45), mailer.send(message))
        .await
        .map_err(|_| Failure {
            message: "提交邮件时超时，送达结果不确定；请检查收件箱后再决定是否重发".into(),
            retryable: false,
            uncertain: true,
        })?
        .map_err(|e| classify(e, true))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    #[tokio::test]
    async fn local_smtp_receives_built_message_without_real_account() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            let mut captured = String::new();
            let mut accepted = 0;
            while accepted < 2 {
                let (mut stream, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(
                            std::time::Instant::now() < deadline,
                            "SMTP test did not connect"
                        );
                        std::thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                accepted += 1;
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                stream.write_all(b"220 localhost ESMTP test\r\n").unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut data = false;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap() == 0 {
                        break;
                    }
                    if data {
                        if line == ".\r\n" {
                            data = false;
                            stream.write_all(b"250 accepted\r\n").unwrap();
                        } else {
                            captured.push_str(&line);
                        }
                        continue;
                    }
                    let response = if line.starts_with("EHLO") {
                        "250 localhost\r\n"
                    } else if line.starts_with("DATA") {
                        data = true;
                        "354 continue\r\n"
                    } else if line.starts_with("QUIT") {
                        stream.write_all(b"221 bye\r\n").unwrap();
                        break;
                    } else {
                        "250 ok\r\n"
                    };
                    stream.write_all(response.as_bytes()).unwrap();
                }
            }
            captured
        });
        // 只有本机测试使用无TLS测试宿主；正式send始终由relay/starttls_relay构造。
        let mailer = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous("127.0.0.1")
            .port(port)
            .timeout(Some(Duration::from_secs(3)))
            .build();
        let message = Message::builder()
            .from("sender@example.com".parse().unwrap())
            .to("recipient@example.com".parse().unwrap())
            .subject("Reminder SMTP test")
            .body("deterministic reminder test".to_owned())
            .unwrap();
        deliver(&mailer, || Ok(message)).await.unwrap();
        let captured = server.join().unwrap();
        assert!(captured.contains("recipient@example.com"));
        assert!(captured.contains("deterministic reminder test"));
    }
}
