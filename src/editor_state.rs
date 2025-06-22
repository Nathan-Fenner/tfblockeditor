use bevy::prelude::*;

use crate::{building::Building, voxels::VOXEL_SIZE};

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

    /// Translate an existing building by the specified amount.
    pub fn translate_building(&mut self, building_index: usize, delta: IVec2) {
        if building_index >= self.buildings.len() {
            return;
        }

        for p in self.buildings[building_index].points_mut().iter_mut() {
            // TODO: Check for overflow
            *p += delta;
        }
    }
}

pub fn grid_to_world(p: IVec3) -> Vec3 {
    p.as_vec3() * Vec3::splat(VOXEL_SIZE)
}
pub fn world_to_grid(p: Vec3) -> IVec3 {
    (p / VOXEL_SIZE).round().as_ivec3()
}
pub fn from_flat(p: IVec2, y: i32) -> IVec3 {
    IVec3::new(p.x, y, p.y)
}
pub fn to_flat(p: IVec3) -> IVec2 {
    p.xz()
}
