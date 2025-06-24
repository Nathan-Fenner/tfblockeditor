use bevy::prelude::*;

use crate::{
    building::{Building, BuildingValidity},
    voxels::VOXEL_SIZE,
};

#[derive(Resource)]
pub struct EditorWorld {
    buildings: Vec<Building>,
    editor_tool: EditorTool,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum EditorTool {
    /// Create a new building
    CreateBuilding,
    /// Select a building
    SelectBuilding,
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
            editor_tool: EditorTool::SelectBuilding,
        }
    }

    /// Get the current tool.
    pub fn tool(&self) -> &EditorTool {
        &self.editor_tool
    }

    /// Sets the current tool.
    pub fn set_tool(&mut self, tool: EditorTool) {
        self.editor_tool = tool;
    }

    /// Get the current buildings in the editor.
    pub fn buildings(&self) -> &[Building] {
        &self.buildings
    }

    /// Add a new building to the editor.
    pub fn insert_building(&mut self, building: Building) {
        self.buildings.push(building);
    }

    /// Changes the position of a point in a building.
    /// Panics if the resulting building is invalid.
    pub fn set_building_point(&mut self, building: usize, point: usize, p: IVec2) {
        self.buildings[building].outline[point] = p;
        assert!(self.buildings[building].is_valid(BuildingValidity::default()));
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
