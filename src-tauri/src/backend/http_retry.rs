use super::{Result, ServiceError};
use crate::domain::ErrorCode;
use reqwest::blocking::{RequestBuilder, Response};
use std::time::Duration;

fn delay(response: &Response, attempt: u32) -> Option<Duration> {
    let fallback = Duration::from_secs(1 << attempt);
    let requested = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|header| header.to_str().ok())
        .and_then(|value| {
            value
                .parse::<u64>()
                .ok()
                .map(Duration::from_secs)
                .or_else(|| {
                    chrono::DateTime::parse_from_rfc2822(value)
                        .ok()
                        .map(|date| {
                            (date.with_timezone(&chrono::Utc) - chrono::Utc::now())
                                .to_std()
                                .unwrap_or_default()
                        })
                })
        })
        .unwrap_or_default()
        .max(fallback);
    (requested <= Duration::from_secs(10)).then_some(requested)
}
pub fn send(build: impl FnMut() -> RequestBuilder) -> Result<Response> {
    send_with_wait(build, std::thread::sleep)
}
pub fn send_checked(
    build: impl FnMut() -> RequestBuilder,
    check: impl Fn() -> Result<()>,
) -> Result<Response> {
    send_with_checks(
        build,
        |duration| {
            let deadline = std::time::Instant::now() + duration;
            loop {
                check()?;
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                if remaining.is_zero() {
                    return Ok(());
                }
                std::thread::sleep(remaining.min(Duration::from_millis(100)));
            }
        },
        &check,
    )
}
fn send_with_wait(
    build: impl FnMut() -> RequestBuilder,
    mut wait: impl FnMut(Duration),
) -> Result<Response> {
    send_with_checks(
        build,
        |duration| {
            wait(duration);
            Ok(())
        },
        || Ok(()),
    )
}
fn send_with_checks(
    mut build: impl FnMut() -> RequestBuilder,
    mut wait: impl FnMut(Duration) -> Result<()>,
    check: impl Fn() -> Result<()>,
) -> Result<Response> {
    for attempt in 0..3 {
        check()?;
        match build().send() {
            Ok(response) => {
                check()?;
                let retryable =
                    response.status().as_u16() == 429 || response.status().is_server_error();
                if retryable && attempt < 2 {
                    if let Some(duration) = delay(&response, attempt) {
                        drop(response);
                        wait(duration)?;
                        continue;
                    }
                }
                return Ok(response);
            }
            Err(_) if attempt < 2 => wait(Duration::from_secs(1 << attempt))?,
            Err(_) => {
                check()?;
                return Err(ServiceError(
                    ErrorCode::NetworkUnavailable,
                    "资料请求超时或网络不可用，请稍后再试。",
                ));
            }
        }
    }
    unreachable!("bounded retry always returns")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };
    fn server(
        replies: Vec<(&'static str, &'static str, &'static str)>,
        pause: Duration,
    ) -> (String, thread::JoinHandle<usize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/fixture", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let mut count = 0;
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            for (status, header, body) in replies {
                let (mut stream, _) = loop {
                    match listener.accept() {
                        Ok(connection) => break connection,
                        Err(error)
                            if error.kind() == std::io::ErrorKind::WouldBlock
                                && std::time::Instant::now() < deadline =>
                        {
                            thread::sleep(Duration::from_millis(1))
                        }
                        Err(error) => {
                            panic!("fixture accept failed within bounded deadline: {error}")
                        }
                    }
                };
                // Windows can inherit the listener's nonblocking mode.
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                    let mut buffer = [0u8; 1024];
                    let size = stream.read(&mut buffer).unwrap();
                    assert!(size > 0, "fixture request closed before headers completed");
                    request.extend_from_slice(&buffer[..size]);
                    assert!(request.len() <= 8192, "fixture request headers too large");
                }
                thread::sleep(pause);
                let response = format!(
                    "HTTP/1.1 {status}\r\nConnection: close\r\nContent-Length: {}\r\n{header}\r\n{body}",
                    body.len()
                );
                // The timeout fixture deliberately closes before this write.
                let sent = stream.write_all(response.as_bytes());
                if pause == Duration::ZERO {
                    sent.unwrap();
                    stream.flush().unwrap();
                }
                count += 1;
            }
            count
        });
        (url, handle)
    }
    fn client(timeout: Duration) -> reqwest::blocking::Client {
        reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(timeout)
            .build()
            .unwrap()
    }
    #[test]
    fn respects_retry_after_and_recovers_without_remote_credentials() {
        let (url, server) = server(
            vec![
                ("429 Too Many Requests", "Retry-After: 2\r\n", "{}"),
                ("200 OK", "", "{}"),
            ],
            Duration::ZERO,
        );
        let client = client(Duration::from_secs(1));
        let mut delays = Vec::new();
        let response = send_with_wait(|| client.get(&url), |delay| delays.push(delay)).unwrap();
        assert_eq!(response.status().as_u16(), 200);
        assert_eq!(delays, vec![Duration::from_secs(2)]);
        assert_eq!(server.join().unwrap(), 2);
    }
    #[test]
    fn repeated_429_stops_at_three_and_long_wait_is_not_shortened() {
        let (url, server) = server(vec![("429 Too Many Requests", "", "{}"); 3], Duration::ZERO);
        let client = client(Duration::from_secs(1));
        let mut delays = Vec::new();
        let response = send_with_wait(|| client.get(&url), |delay| delays.push(delay)).unwrap();
        assert_eq!(
            super::super::bangumi::request_json(response).unwrap_err().0,
            ErrorCode::RateLimited
        );
        assert_eq!(delays, vec![Duration::from_secs(1), Duration::from_secs(2)]);
        assert_eq!(server.join().unwrap(), 3);
        let (url, server) = self::server(
            vec![("429 Too Many Requests", "Retry-After: 120\r\n", "{}")],
            Duration::ZERO,
        );
        let response =
            send_with_wait(|| client.get(&url), |_| panic!("must not retry early")).unwrap();
        assert_eq!(response.status().as_u16(), 429);
        assert_eq!(server.join().unwrap(), 1);
    }
    #[test]
    fn permission_and_invalid_json_are_not_retried() {
        for (status, body, code) in [
            ("401 Unauthorized", "{}", ErrorCode::PermissionDenied),
            ("200 OK", "not json", ErrorCode::InvalidResponse),
        ] {
            let (url, server) = server(vec![(status, "", body)], Duration::ZERO);
            let client = client(Duration::from_secs(1));
            let response =
                send_with_wait(|| client.get(&url), |_| panic!("unexpected retry")).unwrap();
            assert_eq!(
                super::super::bangumi::request_json(response).unwrap_err().0,
                code
            );
            assert_eq!(server.join().unwrap(), 1);
        }
    }
    #[test]
    fn request_timeout_is_bounded_and_reports_network_failure() {
        let (url, server) = server(vec![("200 OK", "", "{}"); 3], Duration::from_millis(100));
        let client = client(Duration::from_millis(30));
        let mut delays = Vec::new();
        assert_eq!(
            send_with_wait(|| client.get(&url), |delay| delays.push(delay))
                .unwrap_err()
                .0,
            ErrorCode::NetworkUnavailable
        );
        assert_eq!(delays.len(), 2);
        assert_eq!(server.join().unwrap(), 3);
    }
    #[test]
    fn cancelled_during_retry_wait_does_not_send_another_request() {
        let (url, server) = server(vec![("503 Service Unavailable", "", "{}")], Duration::ZERO);
        let client = client(Duration::from_secs(1));
        let flag = std::sync::atomic::AtomicBool::new(false);
        let result = send_with_checks(
            || client.get(&url),
            |_| {
                flag.store(true, std::sync::atomic::Ordering::Release);
                Ok(())
            },
            || {
                if flag.load(std::sync::atomic::Ordering::Acquire) {
                    Err(ServiceError(ErrorCode::Cancelled, "资料搜索已取消。"))
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result.unwrap_err().0, ErrorCode::Cancelled);
        assert_eq!(server.join().unwrap(), 1);
    }
}
