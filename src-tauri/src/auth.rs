use crate::{graph::Graph, store::*};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tauri_plugin_secure_keystore::{ItemKey, SecureKeystoreExt, SetItemRequest};
const IDP: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
pub struct Pending {
    client: String,
    code: String,
}
pub struct Session {
    pub graph: Graph,
    pub drive: String,
    pub client: String,
    expires: Instant,
    pub tasks: bool,
}
#[derive(Default)]
pub struct Auth {
    pending: Option<Pending>,
    pub session: Option<Session>,
}
fn value(v: &Value, key: &str) -> Result<String> {
    v[key]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("微软响应缺少 {key}"))
}
fn key(drive: &str, client: &str) -> String {
    format!("refresh-{}", hash(format!("{drive}/{client}")))
}
async fn grant(http: &Client, route: &str, params: &[(&str, &str)]) -> Result<Value> {
    let r = http
        .post(format!("{IDP}/{route}"))
        .form(params)
        .send()
        .await
        .map_err(|e| err(e.without_url()))?;
    let ok = r.status().is_success();
    let v: Value = r.json().await.map_err(err)?;
    if !ok && v["error"].is_null() {
        return Err("微软登录服务暂时不可用".into());
    }
    Ok(v)
}
fn check(v: &Value) -> Result<()> {
    if let Some(e) = v["error"].as_str() {
        Err(format!("微软登录：{e}"))
    } else {
        Ok(())
    }
}
impl Auth {
    pub async fn begin(&mut self, http: &Client, client: String, tasks: bool) -> Result<Value> {
        let scope = if tasks {
            "Files.ReadWrite Tasks.Read offline_access"
        } else {
            "Files.ReadWrite offline_access"
        };
        let v = grant(
            http,
            "devicecode",
            &[("client_id", &client), ("scope", scope)],
        )
        .await?;
        check(&v)?;
        self.pending = Some(Pending {
            client,
            code: value(&v, "device_code")?,
        });
        Ok(
            json!({"code":v["user_code"],"url":v["verification_uri"],"interval":v["interval"],"expires":v["expires_in"]}),
        )
    }
    pub async fn poll(&mut self, app: &AppHandle, http: &Client) -> Result<Value> {
        let p = self.pending.as_ref().ok_or("请重新登录")?;
        let client = p.client.clone();
        let v = grant(
            http,
            "token",
            &[
                ("client_id", &client),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", &p.code),
            ],
        )
        .await?;
        match v["error"].as_str() {
            Some("authorization_pending") => return Ok(json!({"pending":true})),
            Some("slow_down") => return Ok(json!({"pending":true,"slow":true})),
            _ => check(&v)?,
        }
        let s = self.accept(app, http, &client, v, None).await?;
        let drive = s.drive.clone();
        self.session = Some(s);
        self.pending = None;
        Ok(json!({"drive":drive}))
    }
    async fn accept(
        &self,
        app: &AppHandle,
        http: &Client,
        client: &str,
        v: Value,
        expected: Option<&str>,
    ) -> Result<Session> {
        let graph = Graph {
            http: http.clone(),
            token: value(&v, "access_token")?,
        };
        let drive = value(&graph.get("/me/drive?$select=id").await?, "id")?;
        if expected.is_some_and(|e| e != drive) {
            return Err("登录账号与清单所属账号不一致".into());
        }
        if let Some(refresh) = v["refresh_token"].as_str() {
            app.secure_keystore()
                .set_item(SetItemRequest {
                    key: key(&drive, client),
                    value: refresh.into(),
                })
                .map_err(err)?;
        } else if expected.is_none() {
            return Err("未获得离线访问授权，请重新登录".into());
        }
        Ok(Session {
            graph,
            drive,
            client: client.into(),
            tasks: v["scope"]
                .as_str()
                .unwrap_or("")
                .split_whitespace()
                .any(|s| s.eq_ignore_ascii_case("Tasks.Read")),
            expires: Instant::now()
                + Duration::from_secs(v["expires_in"].as_u64().unwrap_or(300).saturating_sub(60)),
        })
    }
    pub async fn graph(
        &mut self,
        app: &AppHandle,
        http: &Client,
        b: &Binding,
        tasks: bool,
    ) -> Result<Graph> {
        if let Some(s) = &self.session {
            if s.drive == b.drive
                && s.client == b.client_id
                && s.expires > Instant::now()
                && (!tasks || s.tasks)
            {
                return Ok(s.graph.clone());
            }
        }
        let rt = app
            .secure_keystore()
            .get_item(ItemKey {
                key: key(&b.drive, &b.client_id),
            })
            .map_err(err)?
            .value
            .ok_or("请重新登录 OneDrive，本机修改已保留")?;
        let scope = if tasks {
            "Files.ReadWrite Tasks.Read offline_access"
        } else {
            "Files.ReadWrite offline_access"
        };
        let v = grant(
            http,
            "token",
            &[
                ("client_id", &b.client_id),
                ("grant_type", "refresh_token"),
                ("scope", scope),
                ("refresh_token", &rt),
            ],
        )
        .await?;
        check(&v)?;
        let s = self
            .accept(app, http, &b.client_id, v, Some(&b.drive))
            .await?;
        let g = s.graph.clone();
        // A directory picker may be browsing another signed-in account.
        if self
            .session
            .as_ref()
            .is_none_or(|other| other.drive == b.drive && other.client == b.client_id)
        {
            self.session = Some(s);
        }
        Ok(g)
    }
}
