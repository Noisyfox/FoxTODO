use crate::store::Result;
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
pub struct Alarm {
    pub id: String,
    pub title: String,
    pub at: i64,
}
#[tauri::command]
pub async fn schedule_desktop(alarms: Vec<Alarm>) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || schedule(alarms))
        .await
        .map_err(crate::store::err)?
}
#[cfg(target_os = "macos")]
fn schedule(alarms: Vec<Alarm>) -> Result<()> {
    extern "C" {
        fn fox_schedule(json: *const std::ffi::c_char) -> i32;
    }
    let bytes = serde_json::to_vec(&alarms).map_err(crate::store::err)?;
    let s = std::ffi::CString::new(bytes).map_err(crate::store::err)?;
    match unsafe { fox_schedule(s.as_ptr()) } {
        0 => Ok(()),
        1 => Err("请在系统设置中允许 FoxTODO 通知".into()),
        3 => Err("请从 FoxTODO.app 启动以安排系统提醒".into()),
        _ => Err("系统未能安排提醒".into()),
    }
}
#[cfg(windows)]
fn schedule(alarms: Vec<Alarm>) -> Result<()> {
    use crate::store::err;
    use windows::{
        core::HSTRING,
        Data::Xml::Dom::XmlDocument,
        Foundation::DateTime,
        UI::Notifications::{ScheduledToastNotification, ToastNotificationManager},
    };
    let notifier =
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from("io.noisyfox.foxtodo"))
            .map_err(err)?;
    let mut existing = std::collections::HashSet::new();
    for n in notifier.GetScheduledToastNotifications().map_err(err)? {
        let id = n.Id().map_err(err)?.to_string();
        if alarms.iter().any(|r| r.id == id) {
            existing.insert(id);
        } else {
            notifier.RemoveFromSchedule(&n).map_err(err)?
        }
    }
    for a in alarms {
        if existing.contains(&a.id) || a.at <= chrono::Utc::now().timestamp_millis() {
            continue;
        }
        let xml = XmlDocument::new().map_err(err)?;
        let title = a
            .title
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        xml.LoadXml(&HSTRING::from(format!("<toast><visual><binding template=\"ToastGeneric\"><text>FoxTODO</text><text>{title}</text></binding></visual></toast>"))).map_err(err)?;
        let n = ScheduledToastNotification::CreateScheduledToastNotification(
            &xml,
            DateTime {
                UniversalTime: (a.at + 11644473600000) * 10000,
            },
        )
        .map_err(err)?;
        n.SetId(&HSTRING::from(a.id)).map_err(err)?;
        notifier.AddToSchedule(&n).map_err(err)?;
    }
    Ok(())
}
#[cfg(not(any(target_os = "macos", windows)))]
fn schedule(_: Vec<Alarm>) -> Result<()> {
    Err("请使用移动端提醒接口".into())
}
