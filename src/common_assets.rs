use bevy::{prelude::*, render::mesh::PlaneMeshBuilder};

#[allow(unused)]
#[derive(Resource)]
pub struct Common {
    pub cube_mesh: Handle<Mesh>,
    pub plane_mesh: Handle<Mesh>,

    pub gray_material: Handle<StandardMaterial>,
    pub xray_blue_material: Handle<StandardMaterial>,
    pub blue_material: Handle<StandardMaterial>,
    pub red_material: Handle<StandardMaterial>,

    pub sky_material: Handle<StandardMaterial>,
    pub outside_material: Handle<StandardMaterial>,
}

pub struct CommonPlugin;

impl Plugin for CommonPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_common);
    }
}

pub fn setup_common(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    let grid_texture: Handle<Image> = asset_server.load("grid.png");
    let common = Common {
        cube_mesh: meshes.add(Cuboid::new(1., 1., 1.).mesh()),
        plane_mesh: meshes.add(PlaneMeshBuilder::default().normal(Dir3::Z).build()),

        gray_material: materials.add(StandardMaterial {
            base_color_texture: Some(grid_texture.clone()),
            perceptual_roughness: 1.0,
            ..default()
        }),
        red_material: materials.add(StandardMaterial {
            base_color_texture: Some(grid_texture.clone()),
            base_color: Color::linear_rgb(0.95, 0.5, 0.4),
            perceptual_roughness: 1.0,
            ..default()
        }),
        xray_blue_material: materials.add(StandardMaterial {
            base_color: Color::linear_rgb(0.4, 0.5, 0.96),
            perceptual_roughness: 1.0,
            unlit: true,
            ..default()
        }),
        blue_material: materials.add(StandardMaterial {
            base_color_texture: Some(grid_texture.clone()),
            base_color: Color::linear_rgb(0.4, 0.5, 0.96),
            perceptual_roughness: 1.0,
            ..default()
        }),
        outside_material: materials.add(StandardMaterial {
            base_color: Color::linear_rgb(0.3, 0.3, 0.3),
            base_color_texture: Some(grid_texture.clone()),
            perceptual_roughness: 1.0,
            ..default()
        }),
        sky_material: materials.add(StandardMaterial {
            base_color_texture: Some(asset_server.load("grid.png")),
            base_color: Color::linear_rgb(0.3, 0.7, 0.9),
            perceptual_roughness: 1.0,
            emissive: LinearRgba::new(0.1, 0.2, 0.3, 1.0),

            alpha_mode: AlphaMode::Mask(0.5),
            ..default()
        }),
    };
    commands.insert_resource(common);
}
