use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetHandle {
    pub slot: u32,
    pub generation: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetKind {
    Texture,
    Mesh,
    Shader,
    Material,
    Scene,
    Font,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetState {
    Unloaded,
    Loading,
    Loaded,
    Failed,
    Reloading,
    Evicted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetRecord {
    pub id: String,
    pub kind: AssetKind,
    pub state: AssetState,
    pub path: Option<PathBuf>,
    pub ref_count: u32,
}

#[derive(Debug, Default)]
pub struct AssetManager {
    next_slot: u32,
    generations: HashMap<u32, u32>,
    by_key: HashMap<(AssetKind, String), AssetHandle>,
    records: HashMap<AssetHandle, AssetRecord>,
}

impl AssetManager {
    pub fn register(
        &mut self,
        kind: AssetKind,
        id: impl Into<String>,
        path: Option<PathBuf>,
    ) -> AssetHandle {
        let id = id.into();
        if let Some(handle) = self.by_key.get(&(kind, id.clone())).copied() {
            if let Some(record) = self.records.get_mut(&handle) {
                record.ref_count = record.ref_count.saturating_add(1);
            }
            return handle;
        }

        self.next_slot = self.next_slot.wrapping_add(1);
        let slot = self.next_slot;
        let generation = *self.generations.entry(slot).or_insert(1);
        let handle = AssetHandle { slot, generation };
        self.by_key.insert((kind, id.clone()), handle);
        self.records.insert(
            handle,
            AssetRecord {
                id,
                kind,
                state: AssetState::Unloaded,
                path,
                ref_count: 1,
            },
        );
        handle
    }

    pub fn get(&self, handle: AssetHandle) -> Option<&AssetRecord> {
        self.records.get(&handle)
    }

    pub fn get_by_id(&self, kind: AssetKind, id: &str) -> Option<(AssetHandle, &AssetRecord)> {
        let handle = self.by_key.get(&(kind, id.to_string())).copied()?;
        Some((handle, self.records.get(&handle)?))
    }

    pub fn is_valid(&self, handle: AssetHandle) -> bool {
        self.records.contains_key(&handle)
    }

    pub fn mark_state(&mut self, handle: AssetHandle, state: AssetState) -> bool {
        let Some(record) = self.records.get_mut(&handle) else {
            return false;
        };
        record.state = state;
        true
    }

    pub fn retain(&mut self, handle: AssetHandle) -> bool {
        let Some(record) = self.records.get_mut(&handle) else {
            return false;
        };
        record.ref_count = record.ref_count.saturating_add(1);
        true
    }

    pub fn release(&mut self, handle: AssetHandle) -> Option<AssetRecord> {
        let should_remove = {
            let record = self.records.get_mut(&handle)?;
            if record.ref_count > 1 {
                record.ref_count -= 1;
                return None;
            }
            true
        };

        if should_remove {
            let record = self.records.remove(&handle)?;
            self.by_key.remove(&(record.kind, record.id.clone()));
            let generation = self
                .generations
                .entry(handle.slot)
                .or_insert(handle.generation);
            *generation = generation.wrapping_add(1).max(1);
            return Some(record);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_manager_returns_stable_handle_for_same_asset() {
        let mut manager = AssetManager::default();
        let a = manager.register(
            AssetKind::Texture,
            "crate",
            Some(PathBuf::from("crate.png")),
        );
        let b = manager.register(
            AssetKind::Texture,
            "crate",
            Some(PathBuf::from("crate.png")),
        );
        assert_eq!(a, b);
        assert_eq!(manager.get(a).expect("asset").ref_count, 2);
    }

    #[test]
    fn asset_manager_invalidates_handle_generation_after_release() {
        let mut manager = AssetManager::default();
        let first = manager.register(AssetKind::Mesh, "cube", None);
        let removed = manager.release(first).expect("asset should be removed");
        assert_eq!(removed.id, "cube");
        let second = manager.register(AssetKind::Mesh, "cube", None);
        assert_ne!(first, second);
        assert!(!manager.is_valid(first));
        assert!(manager.is_valid(second));
    }

    #[test]
    fn asset_manager_tracks_state() {
        let mut manager = AssetManager::default();
        let handle = manager.register(AssetKind::Shader, "lit.vert", None);
        assert!(manager.mark_state(handle, AssetState::Loading));
        assert_eq!(
            manager.get(handle).expect("asset").state,
            AssetState::Loading
        );
    }
}
