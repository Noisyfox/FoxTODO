mod auth;
mod graph;
mod import;
mod merge;
mod network;
mod oauth;
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
    login_attempt: Mutex<Option<(Uuid, tokio::sync::oneshot::Sender<()>)>>,
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

// The merge ancestor can contain large attachments and belongs only in local storage.
fn row(r: &Record) -> Value {
    json!({"doc":r.doc,"revision":r.revision,"dirty":r.dirty,"deleted":r.deleted,"conflict":r.conflict})
}
#[tauri::command]
fn snapshot(app: State<App>) -> Result<Value> {
    let s = guard(&app.store)?;
    let mut records = s.records()?;
    for r in &mut records {
        import::show_legacy_icon(&mut r.doc);
    }
    Ok(
        json!({"defaultListId":DEFAULT_LIST_ID,"records":records.iter().map(row).collect::<Vec<_>>(),"binding":s.settings.binding,"oneDriveAvailable":login_client(s.settings.binding.as_ref()).is_some(),"platform":std::env::consts::OS}),
    )
}
#[tauri::command]
fn save_list(
    app: State<App>,
    doc: Document,
    revision: u64,
    deleted: bool,
    workspace: Option<Binding>,
) -> Result<Value> {
    let s = guard(&app.store)?;
    if s.settings.binding != workspace {
        return Err("当前账号或目录已切换".into());
    }
    let id = doc.id;
    s.edit(doc, revision, deleted)?;
    Ok(row(&s.get(id)?.ok_or("保存失败")?))
}
#[tauri::command]
async fn resolve(app: State<'_, App>, id: Uuid, revision: u64, choice: String) -> Result<()> {
    let _op = app.operation.lock().await;
    guard(&app.store)?.resolve(id, revision, &choice)
}
#[tauri::command]
async fn login(
    handle: AppHandle,
    app: State<'_, App>,
    import_tasks: bool,
    attempt: Uuid,
) -> Result<String> {
    let binding = guard(&app.store)?.settings.binding.clone();
    let client = login_client(binding.as_ref())
        .ok_or("此版本尚未启用 OneDrive 登录")?
        .to_owned();
    let (cancel, cancelled) = tokio::sync::oneshot::channel();
    *app.login_attempt.lock().map_err(err)? = Some((attempt, cancel));
    let sign_in = async {
        let grant = oauth::authorize(&handle, &client, import_tasks).await?;
        let expected = binding
            .as_ref()
            .filter(|_| import_tasks)
            .map(|b| b.drive.as_str());
        app.auth
            .lock()
            .await
            .exchange(&handle, &app.http, &client, grant, expected)
            .await
    };
    let result = tokio::select! {
        biased;
        _ = cancelled => Err("已取消登录".into()),
        result = sign_in => result,
    };
    let mut active = app.login_attempt.lock().map_err(err)?;
    if active.as_ref().is_some_and(|(id, _)| *id == attempt) {
        active.take();
    }
    result
}
#[tauri::command]
fn cancel_login(app: State<App>, attempt: Uuid) -> Result<()> {
    let mut active = app.login_attempt.lock().map_err(err)?;
    if active.as_ref().is_some_and(|(id, _)| *id == attempt) {
        active.take();
    }
    Ok(())
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
async fn create_folder(app: State<'_, App>, parent: String, name: String) -> Result<Item> {
    let auth = app.auth.lock().await;
    let session = auth.session.as_ref().ok_or("请先登录")?;
    session.graph.create_folder(&parent, &name).await
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
// A first import can write to the offline workspace before a sync folder is chosen.
async fn import_graph(
    app: &App,
    handle: &AppHandle,
    drive: &str,
    workspace: Option<&Binding>,
) -> Result<Graph> {
    if guard(&app.store)?.settings.binding.as_ref() != workspace {
        return Err("当前工作区已切换".into());
    }
    if let Some(b) = workspace {
        if b.drive != drive {
            return Err("请选择数据目录所属的微软账号".into());
        }
        return app.graph(handle, b, true).await;
    }
    let auth = app.auth.lock().await;
    let session = auth
        .session
        .as_ref()
        .filter(|s| s.drive == drive && s.tasks)
        .ok_or("请重新登录以读取 Microsoft To Do")?;
    Ok(session.graph.clone())
}
#[tauri::command]
async fn import_lists(
    handle: AppHandle,
    app: State<'_, App>,
    drive: String,
    workspace: Option<Binding>,
) -> Result<Value> {
    let g = import_graph(&app, &handle, &drive, workspace.as_ref()).await?;
    let lists = g.pages("/me/todo/lists").await?;
    Ok(json!(lists
        .iter()
        .map(import::list_preview)
        .collect::<Vec<_>>()))
}
#[tauri::command]
async fn import_selected(
    handle: AppHandle,
    app: State<'_, App>,
    ids: Vec<String>,
    drive: String,
    workspace: Option<Binding>,
) -> Result<Value> {
    let _op = app.operation.lock().await;
    let g = import_graph(&app, &handle, &drive, workspace.as_ref()).await?;
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
        match import::selected(&g, &app.store, &drive, &list, id).await {
            Ok(true) => imported += 1,
            Ok(false) => skipped += 1,
            Err(e) => errors.push(format!("{}：{e}", list["displayName"])),
        }
    }
    Ok(json!({"imported":imported,"skipped":skipped,"errors":errors}))
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    let builder = tauri::Builder::default();
    #[cfg(windows)]
    let (builder, context) = {
        use tauri::webview::{Color, PageLoadEvent};

        // Let window-state restore the main window while WebView2 loads offscreen.
        let mut context = context;
        let main = context
            .config_mut()
            .app
            .windows
            .iter_mut()
            .find(|w| w.label == "main")
            .expect("main window configuration");
        main.visible = false;
        main.background_color = Some(Color(250, 249, 246, 255));

        let first_load = std::sync::Once::new();
        let builder = builder.on_page_load(move |webview, load| {
            if webview.label() == "main" && load.event() == PageLoadEvent::Finished {
                first_load.call_once(|| {
                    if let Err(err) = webview.window().show() {
                        eprintln!("Failed to show the main window: {err}");
                    }
                });
            }
        });
        (builder, context)
    };
    #[cfg(desktop)]
    let builder = {
        use tauri_plugin_window_state::{Builder, StateFlags};
        let flags = StateFlags::POSITION
            | StateFlags::SIZE
            | StateFlags::MAXIMIZED
            | StateFlags::FULLSCREEN;
        builder.plugin(Builder::default().with_state_flags(flags).build())
    };
    #[cfg(target_os = "android")]
    let builder = builder.plugin(tauri_plugin_deep_link::init());
    let app = builder
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
                login_attempt: Default::default(),
                http: Client::builder().timeout(Duration::from_secs(90)).build()?,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            save_list,
            resolve,
            login,
            cancel_login,
            folders,
            create_folder,
            bind,
            synchronize,
            import_lists,
            import_selected,
            reminders::schedule_desktop
        ])
        .build(context)
        .expect("FoxTODO runtime");
    #[cfg(target_os = "macos")]
    app.run(|app, event| match event {
        tauri::RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::CloseRequested { api, .. },
            ..
        } if label == "main" => {
            if let Some(window) = app.get_webview_window(&label) {
                if window.hide().is_ok() {
                    api.prevent_close();
                }
            }
        }
        tauri::RunEvent::Reopen {
            has_visible_windows: false,
            ..
        } => {
            if let Some(window) = app.get_webview_window("main") {
                if let Err(err) = window.show().and_then(|_| window.set_focus()) {
                    eprintln!("Failed to reopen the main window: {err}");
                }
            }
        }
        _ => {}
    });
    #[cfg(not(target_os = "macos"))]
    app.run(|_, _| {});
}
#[cfg(test)]
mod tests;
