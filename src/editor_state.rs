use bevy::prelude::*;

use crate::building::Building;

#[derive(Resource)]
pub struct EditorWorld {
    buildings: Vec<Building>,
}

impl Default for EditorWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorWorld {
    /// Create a new empty editor.
    pub fn new() -> Self {
        Self {
            buildings: Vec::new(),
        }
    }

    /// Get the current buildings in the editor.
    pub fn buildings(&self) -> &[Building] {
        &self.buildings
    }

    /// Add a new building to the editor.
    pub fn insert_building(&mut self, building: Building) {
        self.buildings.push(building);
    }
}
