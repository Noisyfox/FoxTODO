use crate::{
    graph::{Cloud, Item},
    merge::{lists_equivalent, Side},
    store::*,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};
use uuid::Uuid;
pub fn guard<T>(s: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>> {
    s.lock().map_err(|_| "本地存储锁不可用".into())
}
pub async fn run(
    cloud: &(impl Cloud + Sync),
    store: &Mutex<Store>,
    parent: &str,
) -> Result<Vec<String>> {
    let items = cloud.list(parent).await?;
    let mut by_name = HashMap::new();
    let mut by_id = HashMap::new();
    for item in &items {
        if item.folder.is_none() {
            by_id.insert(item.id.clone(), item);
            if let Some(id) = item
                .name
                .strip_suffix(".json")
                .and_then(|v| Uuid::parse_str(v).ok())
            {
                by_name.insert(id, item);
            }
        }
    }
    let records = guard(store)?.records()?;
    let mut errors = vec![];
    let mut seen = HashSet::new();
    for r in records {
        let id = r.doc.id;
        seen.insert(id);
        let remote = r
            .base
            .as_ref()
            .and_then(|b| by_id.get(&b.id).copied())
            .or_else(|| by_name.get(&id).copied());
        if let Err(e) = reconcile(cloud, store, parent, r, remote).await {
            errors.push(e)
        }
    }
    for (id, item) in by_name {
        if !seen.contains(&id) {
            match cloud.download(item, id).await {
                Ok(remote) => {
                    let s = guard(store)?;
                    if s.get(id)?.is_none() {
                        s.save(&Record::adopt(remote))?
                    }
                }
                Err(e) => errors.push(format!("{}：{e}", item.name)),
            }
        }
    }
    Ok(errors)
}
async fn reconcile(
    cloud: &(impl Cloud + Sync),
    store: &Mutex<Store>,
    parent: &str,
    r: Record,
    item: Option<&Item>,
) -> Result<()> {
    let id = r.doc.id;
    let version = item.map(|i| (&i.id, &i.etag));
    let base = r.base.as_ref().map(|b| (&b.id, &b.etag));
    let pending = if base != version || r.conflict.is_some() {
        // Re-evaluate a cached conflict too: the user can keep editing offline.
        let cached = r
            .conflict
            .as_ref()
            .filter(|c| c.remote.as_ref().map(|v| (&v.base.id, &v.base.etag)) == version);
        let remote = match (cached, item) {
            (Some(c), _) => c.remote.clone(),
            (None, Some(i)) => Some(cloud.download(i, id).await?),
            (None, None) => None,
        };
        let s = guard(store)?;
        let Some(current) = s.get(id)? else {
            return Ok(());
        };
        let Some(pending) = integrate(&s, current, remote, cached.is_some())? else {
            return Ok(());
        };
        pending
    } else {
        r
    };
    push(cloud, store, parent, pending).await
}

// Return a pending write only after its new ancestor is durable, so a crash
// or another conditional-write failure resumes the same three-way merge.
fn integrate(
    store: &Store,
    mut r: Record,
    remote: Option<Remote>,
    cached: bool,
) -> Result<Option<Record>> {
    match remote {
        None if !r.dirty
            || r.deleted
            || r.ancestor()
                .is_some_and(|base| lists_equivalent(base, &r.doc)) =>
        {
            store.remove(r.doc.id)?;
            return Ok(None);
        }
        Some(v) => {
            let was_sent = r
                .sending
                .as_ref()
                .is_some_and(|h| v.doc.bytes().is_ok_and(|b| hash(b) == *h));
            if (!r.deleted && r.doc == v.doc) || was_sent {
                if r.conflict.is_some() {
                    r.revision += 1;
                }
                r.acknowledge(v.base, v.doc);
                store.save(&r)?;
                return Ok(None);
            }
            if !r.dirty {
                let mut next = Record::adopt(v);
                next.revision = r.revision + 1;
                store.save(&next)?;
                return Ok(None);
            }
            if r.deleted
                && r.base.as_ref().is_some_and(|b| b.id == v.base.id)
                && r.ancestor()
                    .is_some_and(|base| lists_equivalent(base, &v.doc))
            {
                r.acknowledge(v.base, v.doc);
                r.revision += 1;
                store.save(&r)?;
                return Ok(Some(r));
            }
            if let Some(result) = r.merge_with(&v, |_| Side::Local) {
                if result.issues.is_empty() {
                    r.accept_merge(result.doc, v);
                    store.save(&r)?;
                    return Ok(Some(r));
                }
            }
            r.conflict = Some(Conflict { remote: Some(v) });
        }
        None => r.conflict = Some(Conflict { remote: None }),
    }
    if !cached {
        r.revision += 1;
        store.save(&r)?;
    }
    Ok(None)
}

async fn push(
    cloud: &(impl Cloud + Sync),
    store: &Mutex<Store>,
    parent: &str,
    r: Record,
) -> Result<()> {
    let id = r.doc.id;
    if !r.dirty {
        return Ok(());
    }
    if r.deleted {
        if let Some(b) = &r.base {
            cloud.delete(b).await?
        }
        let s = guard(store)?;
        if let Some(mut current) = s.get(id)? {
            if current.revision == r.revision {
                s.remove(id)?
            } else {
                current.base = None;
                current.base_doc = None;
                s.save(&current)?
            }
        }
    } else {
        {
            let s = guard(store)?;
            let Some(mut current) = s.get(id)? else {
                return Ok(());
            };
            if current.revision != r.revision {
                return Ok(());
            }
            current.sending = Some(hash(r.doc.bytes()?));
            s.save(&current)?;
        }
        let saved = cloud.upload(parent, &r.doc, r.base.as_ref()).await?;
        let s = guard(store)?;
        if let Some(mut current) = s.get(id)? {
            current.acknowledge(saved, r.doc);
            s.save(&current)?;
        }
    }
    Ok(())
}
