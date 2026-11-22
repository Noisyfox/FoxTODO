use crate::store::Result;
use reqwest::{RequestBuilder, Response};
use std::{error::Error, io, time::Duration};

fn network_cause(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        return "请求超时";
    }
    let mut cause = error.source();
    while let Some(source) = cause {
        // Classify the cause without exposing a redirect or a signed OneDrive URL.
        let message = source.to_string().to_ascii_lowercase();
        if message.contains("certificate") {
            return "TLS 证书验证失败";
        }
        if message.contains("dns") || message.contains("failed to lookup address") {
            return "域名解析失败";
        }
        if let Some(e) = source.downcast_ref::<io::Error>() {
            match e.kind() {
                io::ErrorKind::ConnectionRefused => return "连接被拒绝",
                io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted => {
                    return "连接中断";
                }
                _ => {}
            }
        }
        cause = source.source();
    }
    if error.is_connect() {
        "无法建立连接"
    } else {
        "请求未能完成"
    }
}

pub async fn send(request: RequestBuilder, service: &str) -> Result<Response> {
    let retry = request.try_clone();
    let result = match (request.send().await, retry) {
        // Retry only if the connection could not be established. A dropped
        // response may already have committed a write or consumed an OAuth code.
        (Err(e), Some(next)) if e.is_connect() => {
            tokio::time::sleep(Duration::from_millis(750)).await;
            next.send().await
        }
        (result, _) => result,
    };
    result.map_err(|e| format!("{service}：{}", network_cause(&e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{io::AsyncReadExt, net::TcpListener};

    #[tokio::test]
    async fn an_interrupted_post_is_not_replayed_or_reported_with_its_url() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let url = format!("http://{}/?private=test", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0; 1024];
            assert!(socket.read(&mut buf).await.unwrap() > 0);
            drop(socket);
            tokio::time::timeout(Duration::from_secs(2), listener.accept())
                .await
                .is_err()
        });
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let error = send(client.post(url).body("test"), "OneDrive 文件上传")
            .await
            .unwrap_err();
        assert!(error.starts_with("OneDrive 文件上传："));
        assert!(!error.contains("private"));
        assert!(server.await.unwrap());
    }
}
