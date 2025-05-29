#![allow(clippy::too_many_arguments)]

use bevy::{
    picking::backend::ray::RayMap,
    platform::collections::{HashMap, HashSet},
    prelude::*,
    render::mesh::PlaneMeshBuilder,
};
use common_assets::Common;
use flycam::CameraControls;
use std::hash::Hash;
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
                editor_visualize_area_system,
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
    mouse_button: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut selected: ResMut<EditorSelected>,
) {
    if mouse_button.just_pressed(MouseButton::Left)
        && !keys.pressed(KeyCode::ShiftLeft)
        && !keys.pressed(KeyCode::ShiftRight)
    {
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
    if keys.just_pressed(KeyCode::Digit4) {
        current_material.0 = common.outside_material.clone();
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
    let new_commited_state = Some(CommittedEditorState {
        selection: editor_selected.0.iter().copied().collect(),
    });
    if voxels.editor_state_before != new_commited_state {
        voxels.editor_state_before = new_commited_state;
    }
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

#[derive(Clone, Component, Eq, PartialEq, Hash, Debug)]
struct VizuSky {
    voxel: IVec3,
}

struct VizuMap<C: Component + Eq + Hash + Clone> {
    existing: HashMap<C, Option<Entity>>,
    to_spawn: Vec<Box<dyn FnOnce(&mut Commands, &Common)>>,
    refreshed: HashSet<C>,
}

impl<C: Component + Eq + Hash + Clone> VizuMap<C> {
    pub fn new(query: &Query<(Entity, &C)>) -> Self {
        Self {
            existing: query
                .iter()
                .map(|(entity, key)| (key.clone(), Some(entity)))
                .collect(),
            to_spawn: Vec::new(),
            refreshed: HashSet::new(),
        }
    }

    /// Adds an item to be spawned.
    pub fn add(&mut self, key: C, f: impl FnOnce(&mut EntityCommands, &Common) + 'static) {
        self.refreshed.insert(key.clone());
        if self.existing.contains_key(&key) {
            return;
        }
        self.existing.insert(key.clone(), None);
        self.to_spawn.push(Box::new(move |commands, common| {
            let spawn_entity = commands
                .spawn((Transform::IDENTITY, Visibility::default(), key))
                .id();

            f(&mut commands.entity(spawn_entity), common);
        }));
    }

    /// Draws all of the items.
    pub fn draw(mut self, commands: &mut Commands, common: &Common) {
        for (existing_key, existing_entity) in self.existing.iter() {
            if let Some(existing_entity) = existing_entity {
                if !self.refreshed.contains(existing_key) {
                    commands.entity(*existing_entity).despawn();
                }
            }
        }
        for to_spawn in std::mem::take(&mut self.to_spawn) {
            to_spawn(commands, common);
        }
    }
}

/// Visualizes the play area.
fn editor_visualize_area_system(
    mut commands: Commands,
    voxels: Res<Voxels>,
    common: Res<Common>,

    vizus: Query<(Entity, &VizuSky)>,

    sky_visible: Local<bool>,
) {
    if !voxels.is_changed() {
        return;
    }

    let sky_height_voxels = 10;
    let min_sky_height_voxels = 6;

    let mut vizu_map = VizuMap::new(&vizus);

    let mut reachable: HashSet<IVec3> = HashSet::new();
    for (p, voxel) in voxels.iter_voxels() {
        if voxel.material != common.outside_material {
            reachable.insert(p);
        }
    }

    if *sky_visible {
        // Stores a mapping from voxel position to height above floor.
        let mut reachable_extended: HashMap<IVec3, i32> =
            reachable.iter().copied().map(|p| (p, 0)).collect();
        for &p in &reachable {
            for y in 1..=sky_height_voxels {
                let p_up = p + IVec3::new(0, y, 0);
                if reachable_extended.contains_key(&p_up) || voxels.has_voxel(p_up) {
                    break;
                }
                reachable_extended.insert(p_up, y);
            }
        }

        // Remove solid cells.
        reachable_extended.retain(|_, v| *v != 0);

        // Smooth out the sky ceiling a bit.
        // Start at the highest cells - if an adjacent cell has a lower ceiling, delete this cell.
        for _ in 0..10 {
            let mut sky_cells = reachable_extended.keys().copied().collect::<Vec<_>>();
            sky_cells.sort_by_key(|p| -p.y);
            for p in sky_cells {
                if reachable_extended.contains_key(&(p + IVec3::Y)) {
                    // Cannot remove a cell below another cell.
                    continue;
                }
                if reachable_extended[&p] <= min_sky_height_voxels {
                    // Cannot lower the ceiling past this point.
                    continue;
                }

                let mut is_bump = false;
                for d in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
                    let n = p + d;
                    if reachable_extended.contains_key(&n) || voxels.has_voxel(n) {
                        continue;
                    }
                    if (1..=5).any(|dy| reachable_extended.contains_key(&(n + IVec3::NEG_Y * dy))) {
                        is_bump = true;
                        break;
                    }
                }

                if is_bump {
                    reachable_extended.remove(&p);
                }
            }
        }

        for &p in reachable_extended.keys() {
            for d in [
                IVec3::X,
                IVec3::Y,
                IVec3::Z,
                IVec3::NEG_X,
                IVec3::NEG_Y,
                IVec3::NEG_Z,
            ] {
                let q = p + d;
                if !reachable_extended.contains_key(&q) && !voxels.has_voxel(q) {
                    let center = VoxelMarker(p)
                        .center()
                        .lerp(VoxelMarker(p + d).center(), 0.5);

                    vizu_map.add(VizuSky { voxel: p * 3 + d }, move |commands, common| {
                        commands.with_child((
                            Mesh3d(common.plane_mesh.clone()),
                            MeshMaterial3d(common.sky_material.clone()),
                            Transform::from_translation(center)
                                .looking_to(d.as_vec3(), if d.y == 0 { Vec3::Y } else { Vec3::X })
                                .with_scale(Vec3::splat(VOXEL_SIZE * 0.9)),
                        ));
                    });
                }
            }
        }
    }

    vizu_map.draw(&mut commands, &common);
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
