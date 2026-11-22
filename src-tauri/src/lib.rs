mod auth;
mod graph;
mod import;
mod reminders;
mod store;
mod sync;
use auth::Auth;
use graph::{segment, Graph, Item};
use reqwest::Client;
use serde_json::{json, Value};
use std::{sync::Mutex, time::Duration};
use store::*;
use sync::guard;
use tauri::{AppHandle, Manager, State};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;
struct App {
    store: Mutex<Store>,
    auth: AsyncMutex<Auth>,
    operation: AsyncMutex<()>,
    http: Client,
}
impl App {
    fn binding(&self) -> Result<Binding> {
        guard(&self.store)?
            .settings
            .binding
            .clone()
            .ok_or("请先选择 OneDrive 目录".into())
    }
    async fn graph(&self, handle: &AppHandle, b: &Binding, tasks: bool) -> Result<Graph> {
        self.auth
            .lock()
            .await
            .graph(handle, &self.http, b, tasks)
            .await
    }
}
// Existing bindings keep the registration that issued their refresh token.
fn login_client(binding: Option<&Binding>) -> Option<&str> {
    [
        binding.map(|b| b.client_id.as_str()),
        option_env!("FOXTODO_CLIENT_ID"),
        Some(include_str!("../microsoft-client-id.txt")),
    ]
    .into_iter()
    .flatten()
    .map(str::trim)
    .find(|id| Uuid::parse_str(id).is_ok_and(|uuid| !uuid.is_nil()))
}

#[tauri::command]
fn snapshot(app: State<App>) -> Result<Value> {
    let s = guard(&app.store)?;
    Ok(
        json!({"records":s.records()?,"binding":s.settings.binding,"oneDriveAvailable":login_client(s.settings.binding.as_ref()).is_some(),"platform":std::env::consts::OS}),
    )
}
#[tauri::command]
fn save_list(
    app: State<App>,
    doc: Document,
    revision: u64,
    deleted: bool,
    workspace: Option<Binding>,
) -> Result<Record> {
    let s = guard(&app.store)?;
    if s.settings.binding != workspace {
        return Err("当前账号或目录已切换".into());
    }
    let id = doc.id;
    s.edit(doc, revision, deleted)?;
    s.get(id)?.ok_or("保存失败".into())
}
#[tauri::command]
async fn resolve(app: State<'_, App>, id: Uuid, revision: u64, choice: String) -> Result<()> {
    let _op = app.operation.lock().await;
    guard(&app.store)?.resolve(id, revision, &choice)
}
#[tauri::command]
async fn login(app: State<'_, App>, import_tasks: bool) -> Result<Value> {
    let client = login_client(guard(&app.store)?.settings.binding.as_ref())
        .ok_or("此版本尚未启用 OneDrive 登录")?
        .to_owned();
    app.auth
        .lock()
        .await
        .begin(&app.http, client, import_tasks)
        .await
}
#[tauri::command]
async fn poll_login(handle: AppHandle, app: State<'_, App>) -> Result<Value> {
    app.auth.lock().await.poll(&handle, &app.http).await
}
#[tauri::command]
async fn folders(app: State<'_, App>, parent: Option<String>) -> Result<Value> {
    let a = app.auth.lock().await;
    let s = a.session.as_ref().ok_or("请先登录")?;
    let meta = if let Some(id) = parent {
        s.graph
            .get(&format!(
                "/me/drive/items/{}?$select=id,name,folder",
                segment(&id)
            ))
            .await?
    } else {
        s.graph.get("/me/drive/root?$select=id,name,folder").await?
    };
    let p = meta["id"].as_str().ok_or("目录无效")?;
    let children: Vec<Item> = s
        .graph
        .items(p)
        .await?
        .into_iter()
        .filter(|i| i.folder.is_some())
        .collect();
    Ok(json!({"id":p,"name":meta["name"],"children":children}))
}
#[tauri::command]
async fn bind(app: State<'_, App>, folder: String) -> Result<()> {
    let _op = app.operation.lock().await;
    let a = app.auth.lock().await;
    let s = a.session.as_ref().ok_or("请先登录")?;
    let meta = s
        .graph
        .get(&format!(
            "/me/drive/items/{}?$select=id,name,folder",
            segment(&folder)
        ))
        .await?;
    if !meta["folder"].is_object() {
        return Err("请选择文件夹".into());
    }
    let b = Binding {
        drive: s.drive.clone(),
        client_id: s.client.clone(),
        folder,
        label: meta["name"].as_str().unwrap_or("OneDrive").into(),
    };
    drop(a);
    guard(&app.store)?.bind(b)
}
#[tauri::command]
async fn synchronize(handle: AppHandle, app: State<'_, App>) -> Result<Vec<String>> {
    let _op = app.operation.lock().await;
    let b = app.binding()?;
    let g = app.graph(&handle, &b, false).await?;
    let errors = sync::run(&g, &app.store, &b.folder).await?;
    if errors
        .iter()
        .any(|e| e.contains("412") || e.contains("409"))
    {
        return sync::run(&g, &app.store, &b.folder).await;
    }
    Ok(errors)
}
#[tauri::command]
async fn import_lists(handle: AppHandle, app: State<'_, App>) -> Result<Value> {
    let b = app.binding()?;
    let g = app.graph(&handle, &b, true).await?;
    Ok(json!(g.pages("/me/todo/lists").await?))
}
#[tauri::command]
async fn import_selected(
    handle: AppHandle,
    app: State<'_, App>,
    ids: Vec<String>,
) -> Result<Value> {
    let _op = app.operation.lock().await;
    let b = app.binding()?;
    let g = app.graph(&handle, &b, true).await?;
    let mut imported = 0;
    let mut skipped = 0;
    let mut errors = vec![];
    for list in g.pages("/me/todo/lists").await? {
        let Some(id) = list["id"].as_str() else {
            continue;
        };
        if !ids.iter().any(|i| i == id) {
            continue;
        }
        if guard(&app.store)?
            .get(import::list_id(&b.drive, id))?
            .is_some()
        {
            skipped += 1;
            continue;
        }
        match import::fetch(&g, &b.drive, &list).await {
            Ok(doc) => {
                guard(&app.store)?.save(&Record::new(doc))?;
                imported += 1
            }
            Err(e) => errors.push(format!("{}：{e}", list["displayName"])),
        }
    }
    Ok(json!({"imported":imported,"skipped":skipped,"errors":errors}))
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_secure_keystore::init())
        .setup(|app| {
            let store = Store::open(app.path().app_data_dir()?).map_err(std::io::Error::other)?;
            app.manage(App {
                store: Mutex::new(store),
                auth: Default::default(),
                operation: Default::default(),
                http: Client::builder().timeout(Duration::from_secs(90)).build()?,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            save_list,
            resolve,
            login,
            poll_login,
            folders,
            bind,
            synchronize,
            import_lists,
            import_selected,
            reminders::schedule_desktop
        ])
        .run(tauri::generate_context!())
        .expect("FoxTODO runtime");
}
#[cfg(test)]
mod tests;
