use crate::store::{err, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::Url;
use sha2::{Digest, Sha256};
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;
use tokio::time::timeout;

pub const IDP: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

pub fn scopes(tasks: bool) -> &'static str {
    if tasks {
        "Files.ReadWrite Tasks.Read offline_access"
    } else {
        "Files.ReadWrite offline_access"
    }
}

pub struct Authorization {
    pub code: String,
    pub verifier: String,
    pub redirect: String,
}

fn random_secret() -> Result<String> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(err)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn callback(url: &Url, redirect: &Url, state: &str) -> Option<Result<String>> {
    let mut endpoint = url.clone();
    endpoint.set_query(None);
    if endpoint != *redirect {
        return None;
    }
    let mut states = url.query_pairs().filter(|(k, _)| k == "state");
    if states.next()?.1 != state || states.next().is_some() {
        return None;
    }
    if let Some((_, error)) = url.query_pairs().find(|(k, _)| k == "error") {
        return Some(Err(if error == "access_denied" {
            "已取消微软授权".into()
        } else {
            format!("微软授权失败：{error}")
        }));
    }
    let mut codes = url.query_pairs().filter(|(k, _)| k == "code");
    let code = codes.next()?.1;
    if code.is_empty() || codes.next().is_some() {
        return None;
    }
    Some(Ok(code.into_owned()))
}

pub async fn authorize(app: &AppHandle, client: &str, tasks: bool) -> Result<Authorization> {
    let verifier = random_secret()?;
    let state = random_secret()?;
    #[cfg(not(target_os = "android"))]
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(err)?;
    #[cfg(not(target_os = "android"))]
    let redirect = Url::parse(&format!(
        "http://localhost:{}/",
        listener.local_addr().map_err(err)?.port()
    ))
    .map_err(err)?;
    #[cfg(target_os = "android")]
    let redirect = Url::parse("msauth://io.noisyfox.foxtodo/q%2B8H2dckkpGcwQ%2FaQxD%2BN2SH2hs%3D")
        .map_err(err)?;

    let mut url = Url::parse(&format!("{IDP}/authorize")).map_err(err)?;
    url.query_pairs_mut().extend_pairs([
        ("client_id", client),
        ("response_type", "code"),
        ("response_mode", "query"),
        ("redirect_uri", redirect.as_str()),
        ("scope", scopes(tasks)),
        ("state", &state),
        ("code_challenge_method", "S256"),
        ("code_challenge", &challenge(&verifier)),
    ]);
    #[cfg(target_os = "android")]
    let mut links = AndroidCallback::listen(app, redirect.clone(), state.clone());
    app.opener()
        .open_url(url.as_str(), Some("inAppBrowser"))
        .map_err(err)?;
    #[cfg(target_os = "android")]
    let wait = async { links.receiver.recv().await.ok_or("登录窗口已关闭")? };
    #[cfg(not(target_os = "android"))]
    let wait = receive_local(listener, &redirect, &state);
    let result = timeout(LOGIN_TIMEOUT, wait)
        .await
        .map_err(|_| "登录已超时，请重试")?;
    #[cfg(not(target_os = "android"))]
    {
        use tauri::Manager;
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    }
    Ok(Authorization {
        code: result?,
        verifier,
        redirect: redirect.into(),
    })
}

#[cfg(not(target_os = "android"))]
async fn receive_local(
    listener: tokio::net::TcpListener,
    redirect: &Url,
    state: &str,
) -> Result<String> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    loop {
        let (mut socket, _) = listener.accept().await.map_err(err)?;
        let mut line = String::new();
        let mut reader = BufReader::new((&mut socket).take(16384));
        let request = reader.read_line(&mut line);
        if !matches!(timeout(Duration::from_secs(2), request).await, Ok(Ok(_))) {
            continue;
        }
        let result = line
            .strip_prefix("GET ")
            .and_then(|r| r.split_once(" HTTP/"))
            .filter(|(path, _)| path.starts_with("/?"))
            .and_then(|(path, _)| redirect.join(path).ok())
            .and_then(|url| callback(&url, redirect, state));
        let (status, body) = if result.is_some() {
            ("200 OK", "已收到登录结果，可以关闭此页并返回 FoxTODO。")
        } else {
            ("404 Not Found", "无效的回调地址。")
        };
        let reply = format!("HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", body.len());
        let _ = socket.write_all(reply.as_bytes()).await;
        if let Some(result) = result {
            return result;
        }
    }
}

#[cfg(target_os = "android")]
struct AndroidCallback {
    app: AppHandle,
    listener: tauri::EventId,
    receiver: tokio::sync::mpsc::Receiver<Result<String>>,
}
#[cfg(target_os = "android")]
impl AndroidCallback {
    fn listen(app: &AppHandle, redirect: Url, state: String) -> Self {
        use tauri_plugin_deep_link::DeepLinkExt;
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        let listener = app.deep_link().on_open_url(move |event| {
            for url in event.urls() {
                if let Some(result) = callback(&url, &redirect, &state) {
                    let _ = sender.try_send(result);
                }
            }
        });
        Self {
            app: app.clone(),
            listener,
            receiver,
        }
    }
}
#[cfg(target_os = "android")]
impl Drop for AndroidCallback {
    fn drop(&mut self) {
        use tauri::Listener;
        self.app.unlisten(self.listener);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pkce_matches_rfc7636_example() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert_eq!(random_secret().unwrap().len(), 43);
    }
    #[cfg(not(target_os = "android"))]
    #[tokio::test]
    async fn browser_callback_survives_an_unrelated_request() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let redirect = Url::parse(&format!("http://localhost:{}/", address.port())).unwrap();
        let server =
            tokio::spawn(async move { receive_local(listener, &redirect, "attempt").await });
        let browser = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        for (state, status) in [("unrelated", 404), ("attempt", 200)] {
            let reply = browser
                .get(format!("http://{address}/?code=test-code&state={state}"))
                .send()
                .await
                .unwrap();
            assert_eq!(reply.status().as_u16(), status);
        }
        assert_eq!(server.await.unwrap().unwrap(), "test-code");
    }
    #[test]
    fn only_accept_matching_endpoint_state_and_unambiguous_code() {
        let redirect = Url::parse("http://localhost:12345/").unwrap();
        let read = |s: &str| callback(&redirect.join(s).unwrap(), &redirect, "nonce");
        assert_eq!(read("?state=nonce&code=a%2Bb"), Some(Ok("a+b".into())));
        assert!(matches!(
            read("?state=nonce&error=access_denied"),
            Some(Err(_))
        ));
        for bad in [
            "?state=wrong&code=x",
            "?state=nonce&state=x&code=a",
            "?state=nonce&code=a&code=b",
            "//example.org/?state=nonce&code=x",
            "/other?state=nonce&code=x",
        ] {
            assert!(read(bad).is_none());
        }
    }
}
