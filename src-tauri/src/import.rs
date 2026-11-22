use crate::{
    graph::{segment, Graph},
    store::*,
};
use chrono::{NaiveDateTime, TimeZone, Utc};
use serde_json::Value;
use uuid::Uuid;
fn string(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or("").into()
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
pub async fn fetch(graph: &Graph, drive: &str, list: &Value) -> Result<Document> {
    let key = list["id"].as_str().ok_or("清单缺少 ID")?;
    let id = list_id(drive, key);
    let mut doc = Document::new(id, string(list, "displayName"));
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
