#![allow(clippy::too_many_arguments)]

use bevy::{prelude::*, render::mesh::PlaneMeshBuilder};
use building::Building;
use common_assets::Common;
use flycam::CameraControls;

use voxels::{VOXEL_SIZE, Voxels};

pub mod building;
pub mod common_assets;
pub mod flycam;
pub mod voxel_editor;
pub mod voxels;

#[derive(Resource)]
struct EditorWorld {
    buildings: Vec<Building>,
}

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
        .add_systems(
            Update,
            (
                draw_grid_system,
                edit_polygon_system,
                draw_building_outlines_system,
            )
                .chain(),
        )
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

fn draw_grid_system(mut gizmos: Gizmos) {
    for x in -20..=20 {
        for z in -20..=20 {
            let p = Vec3::splat(VOXEL_SIZE) * Vec3::new(x as f32, 0.0, z as f32);
            let k = 8.;
            gizmos.line(
                p - k * Vec3::X,
                p + k * Vec3::X,
                Color::linear_rgba(1., 1., 1., 0.5),
            );
            gizmos.line(
                p - k * Vec3::Z,
                p + k * Vec3::Z,
                Color::linear_rgba(1., 1., 1., 0.5),
            );
        }
    }
}

fn grid_to_world(p: IVec3) -> Vec3 {
    p.as_vec3() * Vec3::splat(VOXEL_SIZE)
}
fn world_to_grid(p: Vec3) -> IVec3 {
    (p / VOXEL_SIZE).round().as_ivec3()
}

fn from_flat(p: IVec2, y: i32) -> IVec3 {
    IVec3::new(p.x, y, p.y)
}
fn to_flat(p: IVec3) -> IVec2 {
    p.xz()
}

fn draw_building_outlines_system(mut gizmos: Gizmos, editor_world: Res<EditorWorld>) {
    let color_active = Color::linear_rgb(1., 1., 0.);

    for building in editor_world.buildings.iter() {
        let points = building.points();
        let floor_y = building.floor_y();

        for i in 0..points.len() {
            let point_a = grid_to_world(from_flat(points[i], floor_y));
            let point_b = grid_to_world(from_flat(points[(i + 1) % points.len()], floor_y));
            gizmos.sphere(point_a, 12., color_active);
            gizmos.line(point_a, point_b, color_active);
        }
    }
}

fn edit_polygon_system(
    mut gizmos: Gizmos,
    mut points: Local<Vec<IVec2>>,
    ray_map: Res<bevy::picking::backend::ray::RayMap>,
    mouse_button: Res<ButtonInput<MouseButton>>,

    mut editor_world: ResMut<EditorWorld>,
) {
    let color_active = Color::linear_rgb(1., 1., 0.);
    let color_speculative = Color::linear_rgb(0., 0., 1.);

    let mouse_ray = ray_map.iter().next().map(|r| *r.1);

    let max_pick_distance = 10_000.0;

    let editing_plane_y = 0;

    let mouse_point: Option<Vec3> = (|| {
        let mouse_ray = mouse_ray?;
        let intersection_distance = mouse_ray.intersect_plane(
            grid_to_world(IVec3::Y * editing_plane_y),
            InfinitePlane3d::new(Vec3::Y),
        )?;

        if intersection_distance > max_pick_distance {
            return None;
        }

        Some(mouse_ray.get_point(intersection_distance))
    })();

    let mouse_point_grid = mouse_point.map(world_to_grid);

    if let Some(mouse_point_grid) = mouse_point_grid {
        gizmos.sphere(grid_to_world(mouse_point_grid), 8., color_speculative);

        if mouse_button.just_pressed(MouseButton::Left) {
            if points.contains(&to_flat(mouse_point_grid)) {
                if points.len() >= 3 {
                    // Complete the shape.
                    editor_world
                        .buildings
                        .push(Building::new(editing_plane_y, std::mem::take(&mut *points)))
                } else {
                    points.clear();
                }
            } else {
                points.push(to_flat(mouse_point_grid));
            }
        }
    }

    for i in 0..points.len() {
        let point_a = grid_to_world(from_flat(points[i], 0));
        gizmos.sphere(point_a, 12., color_active);
        let point_b = if i == points.len() - 1 {
            // From the last point, draw a line to the cursor.
            let Some(mouse_point_grid) = mouse_point_grid else {
                continue;
            };
            grid_to_world(mouse_point_grid)
        } else {
            grid_to_world(from_flat(points[i + 1], 0))
        };

        gizmos.line(
            point_a,
            point_b,
            if i == points.len() - 1 {
                color_speculative
            } else {
                color_active
            },
        );
    }
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

    commands.insert_resource(EditorWorld {
        buildings: Vec::new(),
    });

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
