#![allow(clippy::too_many_arguments)]

use bevy::{
    prelude::*,
    render::{mesh::Indices, view::RenderLayers},
};
use common_assets::Common;
use csgrs::{csg::CSG as GenericCSG, polygon::Polygon};
use flycam::CameraControls;

pub type CSG = GenericCSG<SurfaceDetail>;

use voxels::VOXEL_SIZE;

use crate::{
    editor_state::{EditorWorld, from_flat, grid_to_world},
    geometry_utils::BevyToNalgebra,
};
pub mod building;
pub mod common_assets;
pub mod editor_actions;
pub mod editor_state;
pub mod flycam;
pub mod geometry_utils;
pub mod js_ffi;
pub mod preview;
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
        .add_plugins(common_assets::CommonPlugin)
        .add_plugins(crate::editor_actions::EditorActionPlugin)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                draw_grid_system,
                draw_building_outlines_system,
                render_world_system,
                debug_csg_system,
            )
                .chain(),
        )
        .add_plugins(flycam::FlyCameraPlugin)
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

fn draw_building_outlines_system(mut gizmos: Gizmos, editor_world: Res<EditorWorld>) {
    let color_active = Color::linear_rgb(1., 1., 0.5);

    for building in editor_world.buildings().iter() {
        let points = building.points();
        let floor_y = building.floor_y();

        for i in 0..points.len() {
            let point_a = grid_to_world(from_flat(points[i], floor_y));
            let point_b = grid_to_world(from_flat(points[(i + 1) % points.len()], floor_y));
            let mut point_mark = Isometry3d::from_translation(point_a);
            point_mark.rotation *= Quat::from_rotation_x(std::f32::consts::PI / 2.);
            gizmos.rect(point_mark, Vec2::splat(12.), color_active);
            gizmos.line(point_a, point_b, color_active);
        }
    }
}
#[derive(Resource)]
struct RenderedCsg(CSG);

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct SurfaceDetail {
    pub outside: bool,
}

fn render_world_system(world: Res<EditorWorld>, mut rendered_csg: ResMut<RenderedCsg>) {
    if !world.is_changed() {
        return;
    }

    let mut out_buffer_csg: Vec<CSG> = Vec::new();
    let mut room_interior_csg: Vec<CSG> = Vec::new();

    struct RoomLayer<'a> {
        shift_y_floor: f64,
        shift_y_ceiling: f64,
        outside: bool,
        wall_width: f32,
        out: &'a mut Vec<CSG>,
    }

    let layers = [
        RoomLayer {
            shift_y_floor: -0.1,
            shift_y_ceiling: 0.1,
            wall_width: 0.,
            outside: true,
            out: &mut out_buffer_csg,
        },
        RoomLayer {
            shift_y_floor: 0.0,
            shift_y_ceiling: 0.0,
            wall_width: -0.1,
            outside: false,
            out: &mut room_interior_csg,
        },
    ];

    for layer in layers {
        for room in world.buildings().iter() {
            let y_top = (room.floor_y() + 2) as f64 + layer.shift_y_ceiling;
            let y_bot = room.floor_y() as f64 + layer.shift_y_floor;
            let points = room.points();
            let mut polygons: Vec<csgrs::polygon::Polygon<SurfaceDetail>> = Vec::new();

            fn from_flat(v: Vec2, y: f64) -> Vec3 {
                Vec3::new(v.x, y as f32, v.y)
            }

            let shifted_points: Vec<Vec2> = points
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let center = p.as_vec2();
                    let next = points[(i + 1) % points.len()].as_vec2();
                    let prev = points[(i + points.len() - 1) % points.len()].as_vec2();

                    let delta_next = next - center;
                    let delta_prev = prev - center;
                    // let angle_next = delta_next
                    let angle_next = delta_next.to_angle();
                    let mut angle_prev = delta_prev.to_angle();
                    if angle_prev < angle_next {
                        angle_prev += std::f32::consts::PI * 2.;
                    }

                    let angle_middle = (angle_next + angle_prev) / 2.;
                    let offset = Vec2::from_angle(angle_middle) * layer.wall_width
                        / ((angle_next - angle_prev) / 2.).sin();

                    center + offset
                })
                .collect::<Vec<Vec2>>();

            for (y, flip) in [(y_bot, false), (y_top, true)] {
                let mut vertices: Vec<csgrs::vertex::Vertex> = shifted_points
                    .iter()
                    .map(|p: &Vec2| {
                        csgrs::vertex::Vertex::new(
                            from_flat(*p, y).to_point(),
                            if flip { Vec3::Y } else { Vec3::NEG_Y }.to_vector(),
                        )
                    })
                    .collect();

                if flip {
                    vertices.reverse();
                }

                polygons.push(Polygon::new(
                    vertices,
                    Some(SurfaceDetail {
                        outside: layer.outside,
                    }),
                ));
            }

            for i in 0..shifted_points.len() {
                let a = shifted_points[i];
                let b = shifted_points[(i + 1) % shifted_points.len()];

                let a0 = from_flat(a, y_bot);
                let b0 = from_flat(b, y_bot);
                let a1 = from_flat(a, y_top);
                let b1 = from_flat(b, y_top);

                let normal = (b0 - a0).cross(a1 - a0).normalize().to_vector();

                polygons.push(Polygon::new(
                    vec![
                        csgrs::vertex::Vertex::new(a0.to_point(), normal),
                        csgrs::vertex::Vertex::new(a1.to_point(), normal),
                        csgrs::vertex::Vertex::new(b1.to_point(), normal),
                        csgrs::vertex::Vertex::new(b0.to_point(), normal),
                    ],
                    Some(SurfaceDetail {
                        outside: layer.outside,
                    }),
                ));
            }

            layer.out.push(CSG::from_polygons(&polygons));
        }
    }

    let mut world_csg: CSG = CSG::new();
    for outer_csg in &out_buffer_csg {
        world_csg = world_csg.union(&outer_csg.tessellate());
    }

    for inner_csg in &room_interior_csg {
        world_csg = world_csg.difference(&inner_csg.tessellate());
    }

    rendered_csg.0 =
        world_csg
            .tessellate()
            .scale(VOXEL_SIZE as f64, VOXEL_SIZE as f64, VOXEL_SIZE as f64);
}
fn debug_csg_system(
    mut commands: Commands,
    mut gizmos: Gizmos,
    world_csg: Res<RenderedCsg>,
    mut meshes: ResMut<Assets<Mesh>>,
    common: Res<Common>,

    mut rendered: Local<Option<Entity>>,
) {
    if !world_csg.is_changed() {
        return;
    }
    let world_csg = &world_csg.0;
    for poly in world_csg.polygons.iter() {
        let center: Vec3 = poly
            .vertices
            .iter()
            .map(|v| Vec3::new(v.pos.x as f32, v.pos.y as f32, v.pos.z as f32))
            .fold(Vec3::ZERO, |a, b| a + b)
            / poly.vertices.len() as f32;
        for edge in poly.edges() {
            let (a, b) = edge;

            let a = a.pos;
            let b = b.pos;
            let a = Vec3::new(a.x as f32, a.y as f32, a.z as f32);
            let b = Vec3::new(b.x as f32, b.y as f32, b.z as f32);

            let a = a.lerp(center, 0.1);
            let b = b.lerp(center, 0.1);
            // let a = a.lerp(center, 0.1);
            // let b = b.lerp(center, 0.1);
            gizmos.line(a, a.lerp(b, 0.5), Color::linear_rgb(1., 0., 0.));
            gizmos.line(a.lerp(b, 0.5), b, Color::linear_rgb(0., 0., 1.));
        }
    }

    if let Some(rendered) = rendered.take() {
        commands.entity(rendered).despawn();
    }

    let rendered_group = commands
        .spawn((Transform::IDENTITY, Visibility::Inherited))
        .id();
    *rendered = Some(rendered_group);

    let mesh_inside = to_bevy_mesh(world_csg, |face| !face.outside);
    let mesh_inside_handle = meshes.add(mesh_inside);

    commands.entity(rendered_group).with_children(|children| {
        children.spawn((
            Mesh3d(mesh_inside_handle),
            MeshMaterial3d(common.red_material.clone()),
            Transform::from_scale(Vec3::splat(1.)),
        ));
    });
}

fn to_bevy_mesh(csg: &CSG, mut filter_faces: impl FnMut(&SurfaceDetail) -> bool) -> Mesh {
    let tessellated_csg = &csg.tessellate();
    let polygons = &tessellated_csg.polygons;

    // Prepare buffers
    let mut positions_32 = Vec::new();
    let mut normals_32 = Vec::new();
    let mut indices = Vec::with_capacity(polygons.len() * 3);

    let mut index_start = 0u32;

    // Each polygon is assumed to have exactly 3 vertices after tessellation.
    for poly in polygons {
        // skip any degenerate polygons
        if poly.vertices.len() != 3 {
            continue;
        }

        let Some(r) = poly.metadata.as_ref() else {
            continue;
        };
        if !filter_faces(r) {
            continue;
        }

        // push 3 positions/normals
        for v in &poly.vertices {
            positions_32.push([v.pos.x as f32, v.pos.y as f32, v.pos.z as f32]);
            normals_32.push([v.normal.x as f32, v.normal.y as f32, v.normal.z as f32]);
        }

        // triangle indices
        indices.push(index_start);
        indices.push(index_start + 1);
        indices.push(index_start + 2);
        index_start += 3;
    }

    // Create the mesh with the new 2-argument constructor
    let mut mesh = Mesh::new(
        bevy::render::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );

    // Insert attributes. Note the `<Vec<[f32; 3]>>` usage.
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions_32);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals_32);

    // Insert triangle indices
    mesh.insert_indices(Indices::U32(indices));

    mesh
}

#[derive(Component)]
struct XRayCamera;

fn setup(mut commands: Commands) {
    commands.insert_resource(EditorWorld::new());
    commands.insert_resource(RenderedCsg(CSG::new()));

    // Transform for the camera and lighting, looking at (0,0,0) (the position of the mesh).
    let camera_and_light_transform =
        Transform::from_xyz(786., 768., 900.).looking_at(Vec3::ZERO, Vec3::Y);

    // Camera in 3D space.
    commands.spawn((
        Camera3d::default(),
        camera_and_light_transform,
        CameraControls::default(),
        children![
            // Insert a child camera which shows x-ray mode
            (
                Camera3d::default(),
                XRayCamera,
                RenderLayers::layer(7),
                Camera {
                    order: 1,
                    clear_color: ClearColorConfig::None,
                    ..default()
                },
            ),
        ],
    ));

    // Light up the scene.
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_xyz(1786., 768., 900.).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Secondary light
    commands.spawn((
        DirectionalLight {
            color: Color::linear_rgb(0.5, 0.6, 1.0),
            ..default()
        },
        Transform::from_xyz(-1786. / 3., 768. / 2., 900.).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
