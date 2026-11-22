use crate::store::*;
use async_trait::async_trait;
use reqwest::{Client, Method, RequestBuilder, Response, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;
pub const GRAPH: &str = "https://graph.microsoft.com/v1.0";
pub fn segment(v: &str) -> String {
    percent_encoding::utf8_percent_encode(v, percent_encoding::NON_ALPHANUMERIC).to_string()
}
pub async fn response(q: RequestBuilder) -> Result<Response> {
    let r = crate::network::send(q, "Microsoft Graph").await?;
    checked_response(r).await
}
async fn checked_response(r: Response) -> Result<Response> {
    if r.status().is_success() {
        return Ok(r);
    }
    let status = r.status();
    let message = r
        .json::<Value>()
        .await
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();
    Err(format!("OneDrive {status} {message}"))
}
#[derive(Clone)]
pub struct Graph {
    pub http: Client,
    pub token: String,
}
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct Item {
    pub id: String,
    pub name: String,
    #[serde(default, rename = "eTag")]
    pub etag: String,
    #[serde(default)]
    pub folder: Option<Value>,
}
impl Graph {
    pub fn request(&self, m: Method, path: &str) -> RequestBuilder {
        self.http
            .request(m, format!("{GRAPH}{path}"))
            .bearer_auth(&self.token)
    }
    fn replace_content(&self, base: &Base, bytes: Vec<u8>) -> RequestBuilder {
        let path = format!("/me/drive/items/{}/content", segment(&base.id));
        self.request(Method::PUT, &path)
            .header("If-Match", &base.etag)
            .header("Content-Type", "application/json")
            .body(bytes)
    }
    pub async fn get(&self, path: &str) -> Result<Value> {
        response(self.request(Method::GET, path))
            .await?
            .json()
            .await
            .map_err(err)
    }
    pub async fn create_folder(&self, parent: &str, name: &str) -> Result<Item> {
        let name = name.trim();
        if name.is_empty() {
            return Err("请输入文件夹名称".into());
        }
        if name.ends_with('.')
            || name.contains(['"', '*', ':', '<', '>', '?', '/', '\\', '|'])
            || name.chars().any(char::is_control)
        {
            return Err("文件夹名称包含不可用的字符或以句点结尾".into());
        }
        let path = format!("/me/drive/items/{}/children", segment(parent));
        let request = self.request(Method::POST, &path).json(
            &json!({"name": name, "folder": {}, "@microsoft.graph.conflictBehavior": "fail"}),
        );
        let reply = crate::network::send(request, "OneDrive 创建文件夹").await?;
        if reply.status() == reqwest::StatusCode::CONFLICT {
            return Err("创建冲突，请检查当前目录是否已有同名文件或文件夹".into());
        }
        checked_response(reply).await?.json().await.map_err(err)
    }
    pub async fn pages(&self, path: &str) -> Result<Vec<Value>> {
        let mut url = format!("{GRAPH}{path}");
        let mut all = vec![];
        loop {
            let u = Url::parse(&url).map_err(err)?;
            if u.origin() != Url::parse(GRAPH).unwrap().origin() {
                return Err("Graph 分页地址无效".into());
            }
            let val: Value = response(self.http.get(u).bearer_auth(&self.token))
                .await?
                .json()
                .await
                .map_err(err)?;
            all.extend(
                val["value"]
                    .as_array()
                    .ok_or("Graph 集合无效")?
                    .iter()
                    .cloned(),
            );
            match val["@odata.nextLink"].as_str() {
                Some(s) => url = s.to_owned(),
                None => return Ok(all),
            }
        }
    }
    pub async fn items(&self, parent: &str) -> Result<Vec<Item>> {
        let path = if parent.is_empty() {
            "/me/drive/root/children".to_owned()
        } else {
            format!("/me/drive/items/{}/children", segment(parent))
        };
        self.pages(&format!("{path}?$select=id,name,eTag,folder"))
            .await?
            .into_iter()
            .map(|v| serde_json::from_value(v).map_err(err))
            .collect()
    }
}
#[async_trait]
pub trait Cloud {
    async fn list(&self, parent: &str) -> Result<Vec<Item>>;
    async fn download(&self, item: &Item, id: Uuid) -> Result<Remote>;
    async fn upload(&self, parent: &str, doc: &Document, base: Option<&Base>) -> Result<Base>;
    async fn delete(&self, b: &Base) -> Result<()>;
}
#[async_trait]
impl Cloud for Graph {
    async fn list(&self, parent: &str) -> Result<Vec<Item>> {
        self.items(parent).await
    }
    async fn download(&self, item: &Item, id: Uuid) -> Result<Remote> {
        let path = format!("/me/drive/items/{}", segment(&item.id));
        let doc: Document = response(self.request(Method::GET, &format!("{path}/content")))
            .await?
            .json()
            .await
            .map_err(err)?;
        doc.validate(id)?;
        let meta = self.get(&format!("{path}?$select=eTag")).await?;
        if item.etag.is_empty() || meta["eTag"].as_str() != Some(&item.etag) {
            return Err("读取期间云端版本变化，请重试".into());
        }
        Ok(Remote {
            base: Base {
                id: item.id.clone(),
                etag: item.etag.clone(),
            },
            doc,
        })
    }
    async fn upload(&self, parent: &str, doc: &Document, base: Option<&Base>) -> Result<Base> {
        let bytes = doc.bytes()?;
        // Graph accepts a single PUT of up to 250 MB. Keep the version condition
        // on the content write itself when replacing an existing list.
        if let Some(b) = base {
            if bytes.len() <= 250_000_000 {
                return saved_version(response(self.replace_content(b, bytes)).await?).await;
            }
        }

        let request = match base {
            Some(b) => self
                .request(
                    Method::POST,
                    &format!("/me/drive/items/{}/createUploadSession", segment(&b.id)),
                )
                .header("If-Match", &b.etag),
            None => {
                let path = format!(
                    "/me/drive/items/{}:/{}.json:/createUploadSession",
                    segment(parent),
                    doc.id
                );
                self.request(Method::POST, &path)
            }
        }
        .json(&json!({"item": {"@microsoft.graph.conflictBehavior": "fail"}}));
        let session: Value = response(request).await?.json().await.map_err(err)?;
        let upload = session["uploadUrl"]
            .as_str()
            .ok_or("OneDrive 未返回上传地址")?;
        let url = Url::parse(upload).map_err(err)?;
        if url.scheme() != "https" {
            return Err("上传地址无效".into());
        }
        let chunk = 10 * 320 * 1024;
        for (i, part) in bytes.chunks(chunk).enumerate() {
            let start = i * chunk;
            let is_last = start + part.len() == bytes.len();
            if is_last {
                if let Some(b) = base {
                    let path = format!("/me/drive/items/{}?$select=eTag", segment(&b.id));
                    if self.get(&path).await?["eTag"].as_str() != Some(&b.etag) {
                        return Err("上传期间云端清单已变化，将在下次同步时核对冲突".into());
                    }
                }
            }
            let reply = crate::network::send(
                self.http
                    .put(url.clone())
                    .header(
                        "Content-Range",
                        format!("bytes {}-{}/{}", start, start + part.len() - 1, bytes.len()),
                    )
                    .body(part.to_vec()),
                "OneDrive 文件上传",
            )
            .await?;
            let reply = checked_response(reply).await?;
            if is_last {
                return saved_version(reply).await;
            }
        }
        Err("清单内容为空".into())
    }
    async fn delete(&self, b: &Base) -> Result<()> {
        response(
            self.request(
                Method::DELETE,
                &format!("/me/drive/items/{}", segment(&b.id)),
            )
            .header("If-Match", &b.etag),
        )
        .await?;
        Ok(())
    }
}
async fn saved_version(reply: Response) -> Result<Base> {
    let saved: Item = reply.json().await.map_err(err)?;
    if saved.etag.is_empty() {
        return Err("OneDrive 未返回文件版本，将在下次刷新时核对".into());
    }
    Ok(Base {
        id: saved.id,
        etag: saved.etag,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_puts_raw_json_to_item_content_with_the_saved_version() {
        let graph = Graph {
            http: Client::new(),
            token: "test".into(),
        };
        let base = Base {
            id: "drive!item".into(),
            etag: r#""old-version""#.into(),
        };
        let json = br#"{"name":"list","tasks":[]}"#;
        let request = graph.replace_content(&base, json.to_vec()).build().unwrap();
        assert_eq!(request.method(), Method::PUT);
        assert_eq!(
            request.url().as_str(),
            format!("{GRAPH}/me/drive/items/drive%21item/content")
        );
        assert_eq!(request.headers()["If-Match"], base.etag);
        assert_eq!(request.headers()["Content-Type"], "application/json");
        assert_eq!(request.body().unwrap().as_bytes(), Some(json.as_slice()));
    }
}
