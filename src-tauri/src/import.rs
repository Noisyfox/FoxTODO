use crate::sync::guard;
use crate::{
    graph::{segment, Graph},
    store::*,
};
use chrono::{NaiveDateTime, TimeZone, Utc};
use serde_json::Value;
use std::{collections::HashMap, sync::Mutex};
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;
fn string(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or("").into()
}
fn list_label(display_name: &str) -> (Option<&str>, &str) {
    let trimmed = display_name.trim_start();
    if let Some(first) = trimmed.graphemes(true).next() {
        let rest = trimmed[first.len()..].trim_start();
        if !rest.is_empty() && emojis::get(first).is_some() {
            return (Some(first), rest);
        }
    }
    (None, display_name)
}

pub fn list_preview(list: &Value) -> Value {
    let (emoji, name) = list_label(list["displayName"].as_str().unwrap_or_default());
    serde_json::json!({ "id": list["id"], "displayName": name, "emoji": emoji })
}

// Keep implicit drag-sort anchors based on the old name, just as a rename in the UI does.
fn repair_label(doc: &mut Document) -> bool {
    let (Some(emoji), name) = list_label(&doc.name) else {
        return false;
    };
    doc.emoji = Some(emoji.into());
    let name = name.to_owned();
    let old_name = std::mem::replace(&mut doc.name, name);
    let anchor = |index| Position {
        list_name: old_name.clone(),
        list_id: doc.id,
        index,
        rank: "a0".into(),
    };
    doc.position.get_or_insert_with(|| anchor(0));
    for (index, task) in doc.tasks.iter_mut().enumerate() {
        task.position.get_or_insert_with(|| anchor(index));
    }
    true
}

// A legacy import has no list-level source field. Verify a task's deterministic
// import ID before interpreting the prefix offline, so local lists retain their names.
pub fn show_legacy_icon(doc: &mut Document) {
    if doc.emoji.is_some() || doc.id.get_version_num() != 5 {
        return;
    }
    if doc.tasks.iter().any(|task| {
        task.imported
            .as_ref()
            .is_some_and(|raw| imported_id(doc.id, raw) == Ok(task.id))
    }) {
        repair_label(doc);
    }
}
fn imported_id(scope: Uuid, v: &Value) -> Result<Uuid> {
    let id = v["id"].as_str().ok_or("微软任务缺少 ID")?;
    Ok(Uuid::new_v5(&scope, id.as_bytes()))
}
fn reminder(v: &Value) -> Result<String> {
    if v.is_null() {
        return Ok(String::new());
    }
    let dt = string(v, "dateTime");
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&dt) {
        return Ok(dt.with_timezone(&Utc).to_rfc3339());
    }
    let t = NaiveDateTime::parse_from_str(&dt, "%Y-%m-%dT%H:%M:%S%.f").map_err(err)?;
    let name = string(v, "timeZone");
    let tz = name.parse::<chrono_tz::Tz>().or_else(|_| {
        name.parse::<windows_timezones::WindowsTimezone>()
            .map_err(err)
            .and_then(|w| w.tzdb_id().parse::<chrono_tz::Tz>().map_err(err))
    })?;
    tz.from_local_datetime(&t)
        .earliest()
        .map(|x| x.with_timezone(&Utc).to_rfc3339())
        .ok_or("提醒时间无效".into())
}
pub fn list_id(drive: &str, id: &str) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("ms-todo:{drive}:{id}").as_bytes(),
    )
}
// An older import used the Microsoft list's derived ID for the default list too.
// Keep those task IDs and any local edits when promoting that list on re-import.
fn add_default_tasks(store: &Store, source: &Document) -> Result<bool> {
    let mut tasks = store
        .get(DEFAULT_LIST_ID)?
        .unwrap_or_else(|| Record::new(Document::new(DEFAULT_LIST_ID, "任务".into())));
    if tasks.conflict.is_some() || tasks.deleted {
        return Err("请先处理固定任务清单的同步冲突".into());
    }
    let imported = tasks.doc.default_imports.contains(&source.id);
    let existing: HashMap<_, _> = tasks.doc.tasks.iter().map(|t| (t.id, t)).collect();
    let mut additions = Vec::new();
    for t in &source.tasks {
        match existing.get(&t.id) {
            Some(saved) if *saved == t => {}
            None if !imported => additions.push(t.clone()),
            _ => return Err("两份任务清单中的同一任务有不同修改，已保留两份清单".into()),
        }
    }
    if imported {
        return Ok(false);
    }
    tasks.doc.tasks.extend(additions);
    tasks.doc.default_imports.push(source.id);
    tasks.dirty = true;
    tasks.revision += 1;
    store.save(&tasks)?;
    Ok(true)
}

pub async fn selected(
    graph: &Graph,
    store: &Mutex<Store>,
    drive: &str,
    list: &Value,
    key: &str,
) -> Result<bool> {
    let source_id = list_id(drive, key);
    let default = list["wellknownListName"] == "defaultList";
    {
        let s = guard(store)?;
        if let Some(mut old) = s.get(source_id)? {
            if !default
                && !old.deleted
                && old.conflict.is_none()
                && old.doc.emoji.is_none()
                && list["displayName"].as_str() == Some(&old.doc.name)
                && repair_label(&mut old.doc)
            {
                s.edit(old.doc, old.revision, false)?;
                return Ok(true);
            }
            if !default || old.deleted {
                return Ok(false);
            }
            if old.conflict.is_some() {
                return Err("请先处理已导入清单的同步冲突".into());
            }
            // If an earlier promotion was interrupted, keep any independently
            // edited copy rather than deleting tasks that were never transferred.
            add_default_tasks(&s, &old.doc)?;
            s.edit(old.doc, old.revision, true)?;
            return Ok(true);
        }
        if default
            && s.get(DEFAULT_LIST_ID)?
                .is_some_and(|r| r.doc.default_imports.contains(&source_id))
        {
            return Ok(false);
        }
    }
    let doc = fetch(graph, drive, list).await?;
    let s = guard(store)?;
    if default {
        add_default_tasks(&s, &doc)
    } else {
        s.save(&Record::new(doc))?;
        Ok(true)
    }
}

async fn fetch(graph: &Graph, drive: &str, list: &Value) -> Result<Document> {
    let key = list["id"].as_str().ok_or("清单缺少 ID")?;
    let id = list_id(drive, key);
    let (emoji, name) = list_label(list["displayName"].as_str().unwrap_or_default());
    let mut doc = Document::new(id, name.into());
    // An explicit empty icon also distinguishes new imports from legacy documents.
    doc.emoji = Some(emoji.unwrap_or_default().into());
    let path = format!("/me/todo/lists/{}/tasks", segment(key));
    for v in graph.pages(&path).await? {
        let task_id = imported_id(id, &v)?;
        let sub = format!("{path}/{}", segment(&string(&v, "id")));
        let steps = graph
            .pages(&format!("{sub}/checklistItems"))
            .await?
            .into_iter()
            .map(|s| {
                Ok(Step {
                    id: imported_id(task_id, &s)?,
                    title: string(&s, "displayName"),
                    done: s["isChecked"] == true,
                })
            })
            .collect::<Result<_>>()?;
        let mut attachments = vec![];
        if v["hasAttachments"] == true {
            for f in graph.pages(&format!("{sub}/attachments")).await? {
                let attachment = graph
                    .get(&format!("{sub}/attachments/{}", segment(&string(&f, "id"))))
                    .await?;
                if attachment["contentBytes"].as_str().is_none() {
                    return Err("附件缺少文件内容".into());
                }
                attachments.push(serde_json::from_value(attachment).map_err(err)?)
            }
        }
        let body = string(&v["body"], "content");
        let notes = if string(&v["body"], "contentType").eq_ignore_ascii_case("html") {
            html2text::from_read(body.as_bytes(), 120)
                .map_err(err)?
                .trim()
                .into()
        } else {
            body
        };
        let t = Task {
            id: task_id,
            title: string(&v, "title"),
            done: v["status"] == "completed",
            important: v["importance"] == "high",
            notes,
            steps,
            due: string(&v["dueDateTime"], "dateTime")
                .chars()
                .take(10)
                .collect(),
            reminder: if v["isReminderOn"] == true {
                reminder(&v["reminderDateTime"])?
            } else {
                String::new()
            },
            repeat: if v["recurrence"].is_object() {
                "custom".into()
            } else {
                String::new()
            },
            categories: v["categories"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            attachments,
            imported: Some(v),
            ..Default::default()
        };
        doc.tasks.push(t)
    }
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn separates_only_a_complete_leading_emoji_when_the_name_has_text() {
        for icon in ["📚", "👩🏽‍💻", "🇨🇳", "1️⃣", "☕️", "❤️"] {
            for separator in ["", " ", "　"] {
                let raw = format!("{icon}{separator}读书📖");
                assert_eq!(list_label(&raw), (Some(icon), "读书📖"));
            }
        }
        for name in ["读书📖", "👩🏽‍💻", "2026 计划", "# 计划", "普通清单"] {
            assert_eq!(list_label(name), (None, name));
        }
    }

    #[test]
    fn offline_legacy_display_preserves_order_and_requires_import_provenance() {
        let id = list_id("drive", "books");
        let mut doc = Document::new(id, "📚 阅读".into());
        let raw = json!({"id":"first"});
        doc.tasks.push(Task {
            id: imported_id(id, &raw).unwrap(),
            imported: Some(raw),
            ..Default::default()
        });
        let mut fork = doc.clone();
        fork.id = list_id("another", "books");
        show_legacy_icon(&mut fork);
        assert_eq!(fork.name, "📚 阅读");
        show_legacy_icon(&mut doc);
        assert_eq!(doc.name, "阅读");
        assert_eq!(doc.emoji.as_deref(), Some("📚"));
        assert_eq!(doc.tasks[0].position.as_ref().unwrap().list_name, "📚 阅读");
        assert_eq!(
            serde_json::from_slice::<Document>(&doc.bytes().unwrap()).unwrap(),
            doc
        );
    }

    #[tokio::test]
    async fn reimport_repairs_empty_legacy_list_once_and_preserves_renames() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Mutex::new(Store::open(tmp.path().into()).unwrap());
        let graph = Graph {
            http: reqwest::Client::new(),
            token: String::new(),
        };
        let list = json!({"id":"L","displayName":"📚 阅读"});
        let id = list_id("drive", "L");
        let mut doc = Document::new(id, "本机名称".into());
        store.lock().unwrap().edit(doc.clone(), 0, false).unwrap();
        assert!(!selected(&graph, &store, "drive", &list, "L").await.unwrap());
        doc.name = "📚 阅读".into();
        let revision = store.lock().unwrap().get(id).unwrap().unwrap().revision;
        store.lock().unwrap().edit(doc, revision, false).unwrap();
        assert!(selected(&graph, &store, "drive", &list, "L").await.unwrap());
        assert!(!selected(&graph, &store, "drive", &list, "L").await.unwrap());
        let record = store.lock().unwrap().get(id).unwrap().unwrap();
        assert_eq!(record.doc.name, "阅读");
        assert_eq!(record.doc.emoji.as_deref(), Some("📚"));
        assert!(record.dirty);
    }

    #[tokio::test]
    async fn promotes_only_the_builtin_import_and_keeps_local_edits() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Mutex::new(Store::open(tmp.path().into()).unwrap());
        let default = json!({"id":"L","displayName":"Aufgaben","wellknownListName":"defaultList"});
        let custom = json!({"id":"C","displayName":"任务","wellknownListName":"none"});
        let graph = Graph {
            http: reqwest::Client::new(),
            token: String::new(),
        };
        let fixed = Document::new(DEFAULT_LIST_ID, "任务".into());
        store.lock().unwrap().edit(fixed.clone(), 0, false).unwrap();
        assert!(store.lock().unwrap().edit(fixed, 2, true).is_err());
        for source in [&default, &custom] {
            let mut doc = Document::new(
                list_id("drive", source["id"].as_str().unwrap()),
                source["displayName"].as_str().unwrap().into(),
            );
            doc.tasks.push(Task {
                title: "本机编辑".into(),
                ..Default::default()
            });
            store.lock().unwrap().edit(doc, 0, false).unwrap();
        }
        assert!(!selected(&graph, &store, "drive", &custom, "C")
            .await
            .unwrap());
        assert!(selected(&graph, &store, "drive", &default, "L")
            .await
            .unwrap());
        assert!(!selected(&graph, &store, "drive", &default, "L")
            .await
            .unwrap());
        let s = store.lock().unwrap();
        let fixed = s.get(DEFAULT_LIST_ID).unwrap().unwrap();
        assert_eq!(fixed.doc.tasks.len(), 1);
        assert_eq!(fixed.doc.tasks[0].title, "本机编辑");
        assert!(s.get(list_id("drive", "L")).unwrap().unwrap().deleted);
        assert!(!s.get(list_id("drive", "C")).unwrap().unwrap().deleted);
    }

    #[test]
    fn converts_windows_and_iana_reminder_times_to_the_same_instant() {
        for zone in ["China Standard Time", "Asia/Shanghai"] {
            let date = json!({"dateTime":"2026-11-21T09:30:00.0000000","timeZone":zone});
            assert_eq!(reminder(&date).unwrap(), "2026-11-21T01:30:00+00:00");
        }
        assert!(reminder(&json!({"dateTime":"invalid","timeZone":"UTC"})).is_err());
    }

    #[test]
    fn importing_again_preserves_identity_and_isolates_accounts() {
        assert_eq!(list_id("one", "L"), list_id("one", "L"));
        assert_ne!(list_id("one", "L"), list_id("two", "L"));
    }
}
