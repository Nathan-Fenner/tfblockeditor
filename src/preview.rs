use std::hash::Hash;

use std::collections::HashMap;

use bevy::prelude::*;

struct PreviewState {
    epoch: u64,
    entity: Entity,
}

pub struct Previewer<K> {
    epoch: u64,
    cache: HashMap<K, PreviewState>,
}

impl<K> Previewer<K> {
    /// Create a new empty previewer.
    pub fn new() -> Self {
        Self {
            epoch: 0,
            cache: HashMap::new(),
        }
    }

    /// If the `key` is not present in the cache, run `render` and track the returned entity.
    /// If the `key` is already present in the cache, refresh it without running the provided function.
    ///
    /// Call `collect_garbage()` to increment the epoch and remove all out-of-date rendered objects.
    pub fn render(&mut self, key: &K, render: impl FnOnce() -> Entity)
    where
        K: Eq + Hash + Clone,
    {
        if !self.cache.contains_key(key) {
            let new_entity = render();
            self.cache.insert(
                key.clone(),
                PreviewState {
                    epoch: self.epoch + 1,
                    entity: new_entity,
                },
            );
            return;
        }
        // Refresh the epoch of the existing entry.
        self.cache.get_mut(key).unwrap().epoch = self.epoch + 1;
    }

    /// Despawn all of the entites not refreshed in the last epoch.
    pub fn collect_garbage(&mut self, commands: &mut Commands) {
        self.epoch += 1;
        let keep_epoch = self.epoch;
        self.cache.retain(|_, value| {
            if value.epoch == keep_epoch {
                true
            } else {
                commands.entity(value.entity).despawn();
                false
            }
        });
    }
}

impl<K> Default for Previewer<K> {
    fn default() -> Self {
        Self::new()
    }
}
