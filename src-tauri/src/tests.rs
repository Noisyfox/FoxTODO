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
