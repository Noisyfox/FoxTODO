use crate::{
    graph::{Cloud, Item},
    store::*,
    sync,
};
use async_trait::async_trait;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};
use tempfile::TempDir;
use uuid::Uuid;

#[derive(Default)]
struct FakeDrive {
    items: Mutex<HashMap<Uuid, Remote>>,
    lose_reply: AtomicBool,
    pause: AtomicBool,
    started: tokio::sync::Notify,
    proceed: tokio::sync::Notify,
}
#[async_trait]
impl Cloud for FakeDrive {
    async fn list(&self, _: &str) -> Result<Vec<Item>> {
        Ok(self
            .items
            .lock()
            .unwrap()
            .values()
            .map(|r| Item {
                id: r.base.id.clone(),
                etag: r.base.etag.clone(),
                name: format!("{}.json", r.doc.id),
                folder: None,
            })
            .collect())
    }
    async fn download(&self, _: &Item, id: Uuid) -> Result<Remote> {
        self.items
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or("404".into())
    }
    async fn upload(&self, _: &str, doc: &Document, base: Option<&Base>) -> Result<Base> {
        if self.pause.swap(false, Ordering::SeqCst) {
            self.started.notify_one();
            self.proceed.notified().await
        }
        let mut items = self.items.lock().unwrap();
        if items.get(&doc.id).map(|i| &i.base) != base {
            return Err("412".into());
        }
        let b = Base {
            id: doc.id.to_string(),
            etag: Uuid::new_v4().to_string(),
        };
        items.insert(
            doc.id,
            Remote {
                base: b.clone(),
                doc: doc.clone(),
            },
        );
        if self.lose_reply.swap(false, Ordering::SeqCst) {
            return Err("timeout".into());
        }
        Ok(b)
    }
    async fn delete(&self, b: &Base) -> Result<()> {
        let mut items = self.items.lock().unwrap();
        let id = b.id.parse::<Uuid>().unwrap();
        if items.get(&id).map(|i| &i.base) != Some(b) {
            return Err("412".into());
        }
        items.remove(&id);
        Ok(())
    }
}
fn device() -> (TempDir, Mutex<Store>) {
    let tmp = TempDir::new().unwrap();
    let store = Store::open(tmp.path().into()).unwrap();
    (tmp, Mutex::new(store))
}
fn add(s: &Mutex<Store>, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    s.lock()
        .unwrap()
        .edit(Document::new(id, name.into()), 0, false)
        .unwrap();
    id
}
fn edit(s: &Mutex<Store>, id: Uuid, name: &str, del: bool) {
    let s = s.lock().unwrap();
    let r = s.get(id).unwrap().unwrap();
    let mut d = r.doc;
    d.name = name.into();
    s.edit(d, r.revision, del).unwrap()
}
fn get(s: &Mutex<Store>, id: Uuid) -> Record {
    s.lock().unwrap().get(id).unwrap().unwrap()
}
async fn ok_sync(c: &FakeDrive, s: &Mutex<Store>) {
    let errors = sync::run(c, s, "root").await.unwrap();
    assert!(errors.is_empty(), "{errors:?}")
}
#[tokio::test]
async fn first_edit_keeps_acknowledged_content_across_reopen() {
    let (dir, phone) = device();
    let drive = FakeDrive::default();
    let id = add(&phone, "原名");
    ok_sync(&drive, &phone).await;
    assert!(get(&phone, id).base_doc.is_none());
    edit(&phone, id, "草稿", false);
    edit(&phone, id, "定稿", false);
    drop(phone);
    let phone = Mutex::new(Store::open(dir.path().into()).unwrap());
    let record = get(&phone, id);
    assert_eq!(record.base_doc.as_ref().unwrap().name, "原名");
    let mut legacy = serde_json::to_value(record).unwrap();
    legacy.as_object_mut().unwrap().remove("base_doc");
    assert!(serde_json::from_value::<Record>(legacy)
        .unwrap()
        .base_doc
        .is_none());
    ok_sync(&drive, &phone).await;
    assert!(get(&phone, id).base_doc.is_none());
}
#[test]
fn offline_reopen_and_workspace_isolation() {
    let (dir, s) = device();
    let id = add(&s, "购物");
    edit(&s, id, "新名称", false);
    drop(s);
    let mut s = Store::open(dir.path().into()).unwrap();
    let r = s.get(id).unwrap().unwrap();
    assert!(r.dirty);
    assert_eq!(r.doc.name, "新名称");
    let a = Binding {
        drive: "A".into(),
        folder: "x".into(),
        ..Binding::default()
    };
    s.bind(a.clone()).unwrap();
    assert!(s.get(id).unwrap().is_some());
    s.bind(Binding {
        drive: "B".into(),
        ..a.clone()
    })
    .unwrap();
    assert!(s.get(id).unwrap().is_none());
    s.bind(a).unwrap();
    assert!(s.get(id).unwrap().is_some())
}
#[tokio::test]
async fn conflicts_and_independent_lists_survive_restart() {
    let c = FakeDrive::default();
    let (_a, a) = device();
    let (dir, b) = device();
    let x = add(&a, "购物");
    let y = add(&a, "工作");
    ok_sync(&c, &a).await;
    ok_sync(&c, &b).await;
    edit(&a, x, "电脑", false);
    edit(&b, x, "手机", false);
    edit(&b, y, "工作更新", false);
    ok_sync(&c, &a).await;
    ok_sync(&c, &b).await;
    drop(b);
    let b = Mutex::new(Store::open(dir.path().into()).unwrap());
    let r = get(&b, x);
    assert_eq!(r.conflict.unwrap().remote.unwrap().doc.name, "电脑");
    assert_eq!(r.doc.name, "手机");
    assert!(!get(&b, y).dirty);
    b.lock().unwrap().resolve(x, r.revision, "both").unwrap();
    ok_sync(&c, &b).await;
    assert_eq!(c.items.lock().unwrap().len(), 3);
    assert_eq!(get(&b, x).doc.name, "电脑");
}
#[tokio::test]
async fn lost_response_after_commit_handles_later_edits() {
    let c = FakeDrive::default();
    let (_d, s) = device();
    let id = add(&s, "v1");
    c.lose_reply.store(true, Ordering::SeqCst);
    assert!(!sync::run(&c, &s, "root").await.unwrap().is_empty());
    edit(&s, id, "v2", false);
    ok_sync(&c, &s).await;
    assert_eq!(get(&s, id).base_doc.unwrap().name, "v1");
    ok_sync(&c, &s).await;
    assert_eq!(c.items.lock().unwrap()[&id].doc.name, "v2");
    assert_eq!(c.items.lock().unwrap().len(), 1)
}
#[tokio::test]
async fn edit_during_upload_stays_pending() {
    let c = FakeDrive::default();
    let (_d, s) = device();
    let id = add(&s, "v1");
    c.pause.store(true, Ordering::SeqCst);
    tokio::join!(ok_sync(&c, &s), async {
        c.started.notified().await;
        edit(&s, id, "v2", false);
        c.proceed.notify_one()
    });
    assert_eq!(get(&s, id).base_doc.unwrap().name, "v1");
    assert_eq!(get(&s, id).doc.name, "v2");
    ok_sync(&c, &s).await;
    assert!(!get(&s, id).dirty)
}
#[tokio::test]
async fn edit_vs_deletion_both_directions() {
    let c = FakeDrive::default();
    let (_a, a) = device();
    let (_b, b) = device();
    let x = add(&a, "一");
    let y = add(&a, "二");
    ok_sync(&c, &a).await;
    ok_sync(&c, &b).await;
    edit(&a, x, "一", true);
    edit(&b, x, "保留", false);
    edit(&a, y, "新版", false);
    edit(&b, y, "二", true);
    ok_sync(&c, &a).await;
    ok_sync(&c, &b).await;
    assert!(get(&b, x).conflict.unwrap().remote.is_none());
    assert!(get(&b, y).conflict.unwrap().remote.is_some());
    let rev = get(&b, x).revision;
    b.lock().unwrap().resolve(x, rev, "local").unwrap();
    ok_sync(&c, &b).await;
    assert_eq!(c.items.lock().unwrap()[&x].doc.name, "保留")
}
#[tokio::test]
async fn racing_commits_and_stale_conflict_decisions() {
    let c = FakeDrive::default();
    let (_a, a) = device();
    let (_b, b) = device();
    let id = add(&a, "initial");
    ok_sync(&c, &a).await;
    ok_sync(&c, &b).await;
    edit(&a, id, "A", false);
    edit(&b, id, "B", false);
    c.pause.store(true, Ordering::SeqCst);
    let (slow, _) = tokio::join!(sync::run(&c, &a, "root"), async {
        c.started.notified().await;
        ok_sync(&c, &b).await;
        c.proceed.notify_one()
    });
    assert_eq!(slow.unwrap(), vec!["412"]);
    ok_sync(&c, &a).await;
    let r = get(&a, id);
    assert!(r.conflict.is_some());
    a.lock().unwrap().resolve(id, r.revision, "local").unwrap();
    edit(&b, id, "B2", false);
    ok_sync(&c, &b).await;
    ok_sync(&c, &a).await;
    assert_eq!(get(&a, id).conflict.unwrap().remote.unwrap().doc.name, "B2")
}

fn change(s: &Mutex<Store>, id: Uuid, f: impl FnOnce(&mut Document)) {
    let mut r = get(s, id);
    f(&mut r.doc);
    s.lock().unwrap().edit(r.doc, r.revision, false).unwrap();
}

#[tokio::test]
async fn two_offline_devices_merge_and_publish_in_one_sync() {
    let c = FakeDrive::default();
    let (_da, a) = device();
    let (_db, b) = device();
    let id = add(&a, "清单");
    change(&a, id, |d| d.tasks = vec![Task::default(), Task::default()]);
    ok_sync(&c, &a).await;
    ok_sync(&c, &b).await;
    change(&a, id, |d| d.tasks[0].done = true);
    change(&b, id, |d| d.tasks[1].notes = "保留".into());
    ok_sync(&c, &a).await;
    ok_sync(&c, &b).await;
    assert!(!get(&b, id).dirty);
    ok_sync(&c, &a).await;
    let doc = get(&a, id).doc;
    assert_eq!(doc, get(&b, id).doc);
    assert!(doc.tasks[0].done);
    assert_eq!(doc.tasks[1].notes, "保留");
}

#[tokio::test]
async fn mixed_conflict_decisions_after_restart_keep_independent_edits() {
    use crate::merge::{Key, Side};
    let c = FakeDrive::default();
    let (_da, a) = device();
    let (dir, b) = device();
    let id = add(&a, "清单");
    change(&a, id, |d| d.tasks = vec![Task::default(), Task::default()]);
    ok_sync(&c, &a).await;
    ok_sync(&c, &b).await;
    change(&a, id, |d| {
        d.name = "电脑".into();
        d.tasks[0].title = "电脑标题".into();
        d.tasks.pop();
    });
    change(&b, id, |d| {
        d.name = "手机".into();
        d.tasks[0].title = "手机标题".into();
        d.tasks.push(Task {
            title: "新增".into(),
            ..Default::default()
        });
    });
    ok_sync(&c, &a).await;
    ok_sync(&c, &b).await;
    let rev = get(&b, id).revision;
    drop(b);
    let b = Mutex::new(Store::open(dir.path().into()).unwrap());
    ok_sync(&c, &b).await;
    assert_eq!(get(&b, id).revision, rev);
    let decisions = [
        Decision {
            key: Key::Name,
            side: Side::Local,
        },
        Decision {
            key: Key::Task(get(&b, id).doc.tasks[0].id),
            side: Side::Cloud,
        },
    ];
    {
        let store = b.lock().unwrap();
        assert!(store.resolve_items(id, rev, &decisions[..1]).is_err());
        assert!(store.resolve_items(id, rev - 1, &decisions).is_err());
        store.resolve_items(id, rev, &decisions).unwrap();
    }
    ok_sync(&c, &b).await;
    ok_sync(&c, &a).await;
    let doc = get(&a, id).doc;
    assert_eq!(doc.name, "手机");
    assert_eq!(doc.tasks.len(), 2);
    assert_eq!(doc.tasks[0].title, "电脑标题");
    assert_eq!(doc.tasks[1].title, "新增");
}

#[tokio::test]
async fn first_default_list_uses_empty_origin_but_legacy_dirty_record_needs_review() {
    let c = FakeDrive::default();
    let (_da, a) = device();
    let (_db, b) = device();
    let (_dc, old) = device();
    for s in [&a, &b, &old] {
        let mut doc = Document::new(DEFAULT_LIST_ID, "任务".into());
        doc.tasks.push(Task::default());
        s.lock().unwrap().edit(doc, 0, false).unwrap();
    }
    let mut legacy = get(&old, DEFAULT_LIST_ID);
    legacy.base_doc = None;
    old.lock().unwrap().save(&legacy).unwrap();
    ok_sync(&c, &a).await;
    ok_sync(&c, &b).await;
    ok_sync(&c, &a).await;
    assert_eq!(get(&a, DEFAULT_LIST_ID).doc.tasks.len(), 2);
    ok_sync(&c, &old).await;
    assert!(get(&old, DEFAULT_LIST_ID).conflict.is_some());
    assert_eq!(get(&old, DEFAULT_LIST_ID).doc.tasks.len(), 1);
}

#[tokio::test]
async fn cached_conflict_can_be_resolved_by_later_local_edits() {
    let c = FakeDrive::default();
    let (_da, a) = device();
    let (_db, b) = device();
    let id = add(&a, "原名");
    ok_sync(&c, &a).await;
    ok_sync(&c, &b).await;
    edit(&a, id, "电脑", false);
    edit(&b, id, "手机", false);
    ok_sync(&c, &a).await;
    ok_sync(&c, &b).await;
    assert!(get(&b, id).conflict.is_some());
    change(&b, id, |d| {
        d.name = "电脑".into();
        d.tasks.push(Task::default());
    });
    ok_sync(&c, &b).await;
    assert!(get(&b, id).conflict.is_none());
    assert_eq!(c.items.lock().unwrap()[&id].doc.tasks.len(), 1);
}

#[tokio::test]
async fn reverted_edit_allows_cloud_deletion_but_uncertain_upload_preserves_undo() {
    let c = FakeDrive::default();
    let (_da, a) = device();
    let (_db, b) = device();
    let id = add(&a, "原名");
    ok_sync(&c, &a).await;
    ok_sync(&c, &b).await;
    edit(&a, id, "一", false);
    c.lose_reply.store(true, Ordering::SeqCst);
    assert!(!sync::run(&c, &a, "root").await.unwrap().is_empty());
    ok_sync(&c, &b).await;
    change(&b, id, |d| d.tasks.push(Task::default()));
    ok_sync(&c, &b).await;
    edit(&a, id, "原名", false);
    ok_sync(&c, &a).await;
    assert!(get(&a, id).conflict.is_some());
    let rev = get(&a, id).revision;
    a.lock().unwrap().resolve(id, rev, "cloud").unwrap();
    edit(&a, id, "临时名", false);
    edit(&a, id, "一", false);
    edit(&b, id, "一", true);
    ok_sync(&c, &b).await;
    ok_sync(&c, &a).await;
    assert!(a.lock().unwrap().get(id).unwrap().is_none());
}

#[tokio::test]
async fn third_edit_during_merged_upload_survives_conditional_retry_and_restart() {
    let c = FakeDrive::default();
    let (_da, a) = device();
    let (dir, b) = device();
    let id = add(&a, "清单");
    change(&a, id, |d| {
        d.tasks = vec![Task::default(), Task::default(), Task::default()]
    });
    ok_sync(&c, &a).await;
    ok_sync(&c, &b).await;
    change(&a, id, |d| d.tasks[0].done = true);
    change(&b, id, |d| d.tasks[1].done = true);
    ok_sync(&c, &a).await;
    c.pause.store(true, Ordering::SeqCst);
    let (slow, _) = tokio::join!(sync::run(&c, &b, "root"), async {
        c.started.notified().await;
        change(&a, id, |d| d.tasks[2].done = true);
        ok_sync(&c, &a).await;
        c.proceed.notify_one();
    });
    assert_eq!(slow.unwrap(), vec!["412"]);
    drop(b);
    let b = Mutex::new(Store::open(dir.path().into()).unwrap());
    ok_sync(&c, &b).await;
    assert!(c.items.lock().unwrap()[&id]
        .doc
        .tasks
        .iter()
        .all(|t| t.done));
    assert!(!get(&b, id).dirty);
}

#[test]
fn conflict_view_exposes_item_keys_but_keeps_ancestor_out_of_ipc() {
    let mut doc = Document::new(Uuid::new_v4(), "原名".into());
    doc.tasks.push(Task::default());
    let task = doc.tasks[0].id;
    let mut remote = Remote {
        doc,
        base: Base {
            id: "item".into(),
            etag: "1".into(),
        },
    };
    let mut r = Record::adopt(remote.clone());
    r.begin_edit();
    r.doc.tasks[0].title = "手机".into();
    remote.doc.tasks[0].title = "电脑".into();
    remote.base.etag = "2".into();
    r.conflict = Some(Conflict {
        remote: Some(remote),
    });
    let view = crate::row(r.clone());
    assert!(view.get("base_doc").is_none());
    assert_eq!(
        view["conflict"]["issues"],
        serde_json::json!([{"key":{"kind":"task","id":task}}])
    );
    r.base_doc = None;
    assert!(crate::row(r)["conflict"]["issues"].is_null());
}

#[tokio::test]
async fn keeping_both_copies_preserves_independent_changes_in_each() {
    let c = FakeDrive::default();
    let (_da, a) = device();
    let (_db, b) = device();
    let id = add(&a, "清单");
    change(&a, id, |d| d.tasks = vec![Task::default(), Task::default()]);
    ok_sync(&c, &a).await;
    ok_sync(&c, &b).await;
    change(&a, id, |d| {
        d.tasks[0].notes = "云端".into();
        d.tasks.pop();
    });
    change(&b, id, |d| {
        d.tasks[0].notes = "本机".into();
        d.tasks.push(Task {
            title: "新增".into(),
            ..Default::default()
        });
    });
    ok_sync(&c, &a).await;
    ok_sync(&c, &b).await;
    let rev = get(&b, id).revision;
    b.lock().unwrap().resolve(id, rev, "both").unwrap();
    ok_sync(&c, &b).await;
    let docs = c.items.lock().unwrap();
    assert_eq!(docs.len(), 2);
    for record in docs.values() {
        assert_eq!(record.doc.tasks.len(), 2);
        assert_eq!(record.doc.tasks[1].title, "新增");
        let expected = if record.doc.id == id {
            "云端"
        } else {
            "本机"
        };
        assert_eq!(record.doc.tasks[0].notes, expected);
    }
}

#[tokio::test]
async fn moving_a_task_to_the_default_list_syncs_both_lists() {
    let task = Task {
        title: "带附件的任务".into(),
        done: true,
        notes: "保留备注".into(),
        attachments: vec![Attachment {
            name: "note.txt".into(),
            content_bytes: "YQ==".into(),
        }],
        ..Default::default()
    };
    let (_dir, phone) = device();
    let (_other_dir, other) = device();
    let drive = FakeDrive::default();
    let source = add(&phone, "来源");
    let mut r = get(&phone, source);
    r.doc.tasks = vec![task.clone(), Task::default()];
    phone
        .lock()
        .unwrap()
        .edit(r.doc, r.revision, false)
        .unwrap();
    ok_sync(&drive, &phone).await;
    ok_sync(&drive, &other).await;

    {
        let s = phone.lock().unwrap();
        assert!(s
            .move_task(source, DEFAULT_LIST_ID, task.id, 9, None)
            .is_err());
        let [from, to] = s
            .move_task(source, DEFAULT_LIST_ID, task.id, 0, None)
            .unwrap();
        assert!(from.dirty && to.dirty);
        assert_eq!(from.doc.tasks[0].position.as_ref().unwrap().index, 1);
        assert_eq!(from.base_doc.unwrap().tasks[0], task);
        assert_eq!(to.doc.tasks, [task.clone()]);
        assert!(s
            .move_task(source, DEFAULT_LIST_ID, task.id, to.revision, None)
            .is_err());
    }
    ok_sync(&drive, &phone).await;
    ok_sync(&drive, &other).await;
    assert_eq!(get(&other, source).doc.tasks.len(), 1);
    assert_eq!(get(&other, DEFAULT_LIST_ID).doc.tasks, [task]);
}

#[test]
fn interrupted_move_is_replayed_before_the_next_edit() {
    let (dir, device) = device();
    let source = add(&device, "A");
    let destination = add(&device, "B");
    let before = get(&device, destination);
    let mut from = get(&device, source);
    let task = Task::default();
    from.doc.tasks.push(task.clone());
    let store = device.into_inner().unwrap();
    store.edit(from.doc, from.revision, false).unwrap();
    let moved = store
        .move_task(source, destination, task.id, before.revision, None)
        .unwrap();
    drop(store);

    // Recreate a crash after only the source file was installed.
    let workspace = dir.path().join("workspaces/local");
    atomic(&workspace.join(format!("{destination}.json")), &before).unwrap();
    atomic(&workspace.join("pending-move"), &moved).unwrap();
    let store = Store::open(dir.path().into()).unwrap();
    assert!(store.edit(before.doc, before.revision, false).is_err());
    assert!(store.get(source).unwrap().unwrap().doc.tasks.is_empty());
    assert_eq!(store.get(destination).unwrap().unwrap().doc.tasks, [task]);
    assert!(!workspace.join("pending-move").exists());
}
