#![allow(clippy::too_many_arguments)]

use bevy::{prelude::*, render::mesh::PlaneMeshBuilder};
use common_assets::Common;
use flycam::CameraControls;

use voxels::Voxels;

pub mod common_assets;
pub mod flycam;
pub mod voxel_editor;
pub mod voxels;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            // Wasm builds will check for meta files (that don't exist) if this isn't set.
            // This causes errors and even panics in web builds on itch.
            // See https://github.com/bevyengine/bevy_github_ci_template/issues/48.
            meta_check: bevy::asset::AssetMetaCheck::Never,
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_plugins(flycam::FlyCameraPlugin)
        // .add_systems(
        //     Update,
        //     (
        //         editor_record_system,
        //         editor_select_system,
        //         editor_select_preview_system,
        //         editor_undo_system,
        //         editor_visualize_area_system,
        //     )
        //         .chain(),
        // )
        .run();
}

fn setup(
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

    let mut voxels = Voxels::new_empty();
    voxels.add_voxel(
        &mut commands,
        &common,
        IVec3::new(0, 0, 0),
        common.gray_material.clone(),
    );

    // commands.insert_resource(EditorCurrentMaterial(common.gray_material.clone()));
    commands.insert_resource(voxels);
    commands.insert_resource(common);
    // commands.insert_resource(EditorSelected(HashSet::new()));

    // Transform for the camera and lighting, looking at (0,0,0) (the position of the mesh).
    let camera_and_light_transform =
        Transform::from_xyz(786., 768., 900.).looking_at(Vec3::ZERO, Vec3::Y);

    // Camera in 3D space.
    commands.spawn((
        Camera3d::default(),
        camera_and_light_transform,
        CameraControls::default(),
    ));

    // Light up the scene.
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(1786., 768., 900.).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

static EDITABLE_LEVEL: std::sync::Mutex<Option<vmf_forge::VmfFile>> = std::sync::Mutex::new(None);

#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    /// Send a message to the client.
    pub fn tfbe_ffi_alert(s: &str);
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn tfbe_ffi_load_file(file_contents: &str) {
    tfbe_ffi_alert(&format!("Loading file with {} bytes", file_contents.len()));

    let parsed_file: Result<vmf_forge::VmfFile, _> = vmf_forge::VmfFile::parse(file_contents);

    match parsed_file {
        Ok(parsed_file) => {
            *EDITABLE_LEVEL.lock().unwrap() = Some(parsed_file);
            tfbe_ffi_alert("Loaded file!");
        }
        Err(err) => {
            tfbe_ffi_alert(&format!("Failed to parse file: {err}"));
        }
    }
}
