use crate::{
    graph::{Cloud, Item},
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
    if let Some(c) = &r.conflict {
        if c.remote.as_ref().map(|v| (&v.base.id, &v.base.etag)) == item.map(|i| (&i.id, &i.etag)) {
            return Ok(());
        }
    }
    let base = r.base.as_ref().map(|b| (&b.id, &b.etag));
    if base != item.map(|i| (&i.id, &i.etag)) || r.conflict.is_some() {
        let remote = match item {
            Some(i) => Some(cloud.download(i, id).await?),
            None => None,
        };
        let s = guard(store)?;
        let Some(mut current) = s.get(id)? else {
            return Ok(());
        };
        if current.deleted && remote.is_none() {
            return s.remove(id);
        }
        if let Some(v) = &remote {
            let was_sent = current
                .sending
                .as_ref()
                .is_some_and(|h| v.doc.bytes().is_ok_and(|b| hash(b) == *h));
            if !current.deleted && current.doc == v.doc || was_sent {
                current.acknowledge(v.base.clone(), v.doc.clone());
                return s.save(&current);
            }
        }
        if current.dirty {
            current.conflict = Some(Conflict { remote });
            current.revision += 1;
            return s.save(&current);
        }
        return match remote {
            Some(v) => {
                let mut next = Record::adopt(v);
                next.revision = current.revision + 1;
                s.save(&next)
            }
            None => s.remove(id),
        };
    }
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
