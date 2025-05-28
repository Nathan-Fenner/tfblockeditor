use bevy::prelude::*;

#[allow(unused)]
#[derive(Resource)]
pub struct Common {
    pub cube_mesh: Handle<Mesh>,
    pub gray_material: Handle<StandardMaterial>,
    pub blue_material: Handle<StandardMaterial>,
    pub red_material: Handle<StandardMaterial>,
    pub outside_material: Handle<StandardMaterial>,
}
