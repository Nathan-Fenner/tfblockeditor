#![allow(clippy::too_many_arguments)]

use bevy::{picking::backend::ray::RayMap, platform::collections::HashSet, prelude::*};
use common_assets::Common;
use flycam::CameraControls;
use voxels::{CommittedEditorState, VOXEL_SIZE, VoxelMarker, Voxels};

pub mod common_assets;
pub mod flycam;
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
        .add_systems(
            Update,
            (
                editor_record_system,
                editor_select_system,
                editor_select_preview_system,
                editor_undo_system,
            )
                .chain(),
        )
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
            base_color_texture: Some(grid_texture),
            base_color: Color::linear_rgb(0.4, 0.5, 0.96),
            perceptual_roughness: 1.0,
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

    commands.insert_resource(EditorCurrentMaterial(common.gray_material.clone()));
    commands.insert_resource(voxels);
    commands.insert_resource(common);
    commands.insert_resource(EditorSelected(HashSet::new()));

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

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct SelectedFace {
    pub voxel: IVec3,
    pub normal: IVec3,
}

#[derive(Resource)]
struct EditorSelected(HashSet<SelectedFace>);

#[derive(Resource)]
struct EditorCurrentMaterial(Handle<StandardMaterial>);

fn editor_select_system(
    // mut commands: Commands,
    // common: Res<Common>,
    mut gizmos: Gizmos,
    mut cast: MeshRayCast,
    ray_map: Res<RayMap>, // The ray map stores rays cast by the cursor
    voxel_marker: Query<&VoxelMarker>,
    // mut voxels: ResMut<Voxels>,
    mouse_button: Res<ButtonInput<MouseButton>>,

    mut selected: ResMut<EditorSelected>,
) {
    if mouse_button.just_pressed(MouseButton::Left) {
        selected.0.clear();
    }

    let Some((_, mouse_ray)) = ray_map.iter().next() else {
        return;
    };
    let hit = cast
        .cast_ray(
            *mouse_ray,
            &MeshRayCastSettings::default()
                .with_filter(&|hit_entity| voxel_marker.contains(hit_entity)),
        )
        .first()
        .cloned();

    let Some((hit_entity, hit_info)) = hit else {
        return;
    };
    let marked_voxel = voxel_marker.get(hit_entity).expect("must exist");
    let hit_normal = hit_info.normal.normalize();

    for s in [0.9, 0.5] {
        gizmos.cuboid(
            Transform::from_translation(
                marked_voxel.center() + Vec3::splat(VOXEL_SIZE) * 0.5 * hit_normal,
            )
            .with_scale(VOXEL_SIZE * s * (Vec3::splat(1.) - hit_normal.abs() * 0.9)),
            Color::linear_rgb(0., 0., 1.),
        );
    }

    if mouse_button.pressed(MouseButton::Left) {
        selected.0.insert(SelectedFace {
            voxel: marked_voxel.0,
            normal: hit_normal.round().as_ivec3(),
        });
    }
}

fn editor_select_preview_system(
    mut commands: Commands,
    common: Res<Common>,
    mut gizmos: Gizmos,
    mut voxels: ResMut<Voxels>,
    keys: Res<ButtonInput<KeyCode>>,
    mut selected: ResMut<EditorSelected>,
    mut current_material: ResMut<EditorCurrentMaterial>,
) {
    if keys.just_pressed(KeyCode::Digit1) {
        current_material.0 = common.gray_material.clone();
    }
    if keys.just_pressed(KeyCode::Digit2) {
        current_material.0 = common.red_material.clone();
    }
    if keys.just_pressed(KeyCode::Digit3) {
        current_material.0 = common.blue_material.clone();
    }

    for face in selected.0.iter() {
        for s in [0.8, 0.4] {
            gizmos.cuboid(
                Transform::from_translation(
                    VoxelMarker(face.voxel).center()
                        + Vec3::splat(VOXEL_SIZE) * 0.5 * face.normal.as_vec3(),
                )
                .with_scale(VOXEL_SIZE * s * (Vec3::splat(1.) - face.normal.as_vec3().abs() * 0.9)),
                Color::linear_rgb(1., 1., 0.3),
            );
        }
    }

    if keys.just_pressed(KeyCode::KeyE) {
        // Extrude all selected voxel faces.
        let mut new_selected: Vec<SelectedFace> = Vec::new();
        let mut new_extrusions: HashSet<IVec3> = HashSet::new();
        for face in selected.0.iter() {
            let new_voxel = face.voxel + face.normal;
            if voxels.has_voxel(new_voxel) && !new_extrusions.contains(&new_voxel) {
                // Drop this selection.
                continue;
            }
            new_extrusions.insert(new_voxel);
            new_selected.push(SelectedFace {
                voxel: new_voxel,
                normal: face.normal,
            });
            if !voxels.has_voxel(new_voxel) {
                voxels.add_voxel(
                    &mut commands,
                    &common,
                    new_voxel,
                    current_material.0.clone(),
                );
            }
        }
        selected.0 = new_selected.into_iter().collect();
    } else if keys.just_pressed(KeyCode::Delete) {
        // Delete all selected voxel faces, and back up to the faces behind them.
        let mut new_selected: Vec<SelectedFace> = Vec::new();
        for face in selected.0.iter() {
            voxels.remove_voxel(&mut commands, face.voxel);
            new_selected.push(SelectedFace {
                voxel: face.voxel - face.normal,
                normal: face.normal,
            });
        }

        selected.0 = new_selected
            .into_iter()
            .filter(|face| voxels.has_voxel(face.voxel))
            .collect();
    } else if keys.just_pressed(KeyCode::KeyQ) {
        // Depress the selected voxel faces, creating new geometry behind them if needed.
        // This will probably have some weird edge cases.

        // Delete the selected voxels.
        let mut new_selected: Vec<SelectedFace> = Vec::new();
        for face in selected.0.iter() {
            voxels.remove_voxel(&mut commands, face.voxel);
            new_selected.push(SelectedFace {
                voxel: face.voxel - face.normal,
                normal: face.normal,
            });
        }

        for new_face in new_selected.iter() {
            if !voxels.has_voxel(new_face.voxel) {
                voxels.add_voxel(
                    &mut commands,
                    &common,
                    new_face.voxel,
                    current_material.0.clone(),
                );
            }
        }

        // Add an edge around the selected voxels.
        for new_face in new_selected.iter() {
            for dir in [
                IVec3::X,
                IVec3::Y,
                IVec3::Z,
                IVec3::NEG_X,
                IVec3::NEG_Y,
                IVec3::NEG_Z,
            ] {
                if dir == new_face.normal || -dir == new_face.normal {
                    continue;
                }

                let edge = new_face.voxel + dir;
                if !voxels.has_voxel(edge) {
                    if let Some(above) = voxels.get_material(edge + new_face.normal) {
                        voxels.add_voxel(&mut commands, &common, edge, above);
                    }
                }
            }
        }

        selected.0 = new_selected
            .into_iter()
            .filter(|face| voxels.has_voxel(face.voxel))
            .collect();
    } else if keys.just_pressed(KeyCode::KeyF) {
        // Fill the selected voxels with the current color.

        for face in selected.0.iter() {
            voxels.add_voxel(
                &mut commands,
                &common,
                face.voxel,
                current_material.0.clone(),
            );
        }
    } else if keys.just_pressed(KeyCode::BracketLeft) || keys.just_pressed(KeyCode::BracketRight) {
        let column_shift = if keys.just_pressed(KeyCode::BracketLeft) {
            -32
        } else {
            32
        };
        let columns_to_shift = selected
            .0
            .iter()
            .map(|face| face.voxel.xz())
            .collect::<HashSet<IVec2>>();
        for column in columns_to_shift {
            voxels.shift_column(&mut commands, &common, column, column_shift);
        }
    }
}

fn editor_record_system(mut voxels: ResMut<Voxels>, editor_selected: ResMut<EditorSelected>) {
    voxels.editor_state_before = Some(CommittedEditorState {
        selection: editor_selected.0.iter().copied().collect(),
    });
}

fn editor_undo_system(
    mut commands: Commands,
    common: Res<Common>,
    mut voxels: ResMut<Voxels>,
    keys: Res<ButtonInput<KeyCode>>,
    mut editor_selected: ResMut<EditorSelected>,
) {
    if voxels.has_changes_to_commit() {
        let editor_state_before = voxels.editor_state_before.take().unwrap();
        voxels.commit_changes(editor_state_before);
    }

    if keys.just_pressed(KeyCode::KeyZ)
        && (keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight))
    {
        let undo_editor = voxels.undo_last_action(&mut commands, &common);

        // Revert the editor state to how it was before the action that was just undone.
        editor_selected.0 = undo_editor.selection.into_iter().collect();
    }
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
