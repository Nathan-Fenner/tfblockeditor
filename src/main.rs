#![allow(clippy::too_many_arguments)]

use bevy::{
    prelude::*,
    render::mesh::{Indices, PlaneMeshBuilder},
};
use building::Building;
use common_assets::Common;
use csgrs::{csg::CSG as GenericCSG, polygon::Polygon};
use flycam::CameraControls;

pub type CSG = GenericCSG<SurfaceDetail>;

use voxels::VOXEL_SIZE;

pub mod building;
pub mod common_assets;
pub mod csg;
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
                render_world_system,
                debug_csg_system,
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
    let color_active = Color::linear_rgb(1., 1., 0.5);

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

fn project_onto_v2(a: Vec2, (p, q): (Vec2, Vec2)) -> Vec2 {
    (a - p).dot((q - p).normalize()) * (q - p).normalize() + p
}

#[allow(unused)]
fn project_onto_i2(a: IVec2, (p, q): (IVec2, IVec2)) -> Vec2 {
    project_onto_v2(a.as_vec2(), (p.as_vec2(), q.as_vec2()))
}

fn point_closest_to_segment(p: Vec2, line: (Vec2, Vec2)) -> Vec2 {
    // Project p onto the line.
    let p_on_line = project_onto_v2(p, line);

    let d = (line.1 - line.0).normalize();

    let t = (p_on_line - line.0).dot(d) / (line.0.distance(line.1));
    let t = t.clamp(0.0, 1.0);

    line.0.lerp(line.1, t)
}

fn segments_cross(a: (Vec2, Vec2), b: (Vec2, Vec2)) -> bool {
    let (p1, p2) = a;
    let (q1, q2) = b;

    let r = p2 - p1;
    let s = q2 - q1;
    let pq = q1 - p1;
    let rxs = r.perp_dot(s);

    if rxs == 0.0 {
        // Lines are parallel (or colinear)
        return false;
    }

    let t = pq.perp_dot(s) / rxs;
    let u = pq.perp_dot(r) / rxs;

    (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)
}

fn edit_polygon_system(
    mut gizmos: Gizmos,
    mut points: Local<Vec<IVec2>>,
    ray_map: Res<bevy::picking::backend::ray::RayMap>,
    mouse_button: Res<ButtonInput<MouseButton>>,

    mut editor_world: ResMut<EditorWorld>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let wall_thickness = 0.125;
    // How far we're allowed to eat into a wall segment at a corner due to the wall thickness.
    let min_extended = 0.45;

    let color_active = Color::linear_rgb(1., 1., 0.);
    let color_speculative = Color::linear_rgb(0., 0., 1.);
    let color_invalid = Color::linear_rgb(1., 0., 0.);

    if keys.just_pressed(KeyCode::Escape) {
        points.clear();
    }

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

    let new_point_is_valid = (|| {
        let Some(mouse_point_grid) = mouse_point_grid else {
            return false;
        };

        let mouse_point_grid = to_flat(mouse_point_grid);

        if points.len() <= 1 {
            // No possible invalid states.
            return true;
        }

        if points.len() == 2 && mouse_point_grid == points[0] {
            return false;
        }

        if points.len() >= 3 && mouse_point_grid == points[0] {
            // If the corner is too sharp, then we have a problem.
            let a = points[1];
            let pivot = points[0];
            let b = points[points.len() - 1];

            let angle = (a - pivot)
                .as_vec2()
                .normalize()
                .dot((b - pivot).as_vec2().normalize())
                .acos();

            let max_movement = wall_thickness / (angle / 2.0).sin();

            if max_movement >= min_extended {
                return false;
            }
        }

        if mouse_point_grid != points[0] && points.contains(&mouse_point_grid) {
            return false;
        }

        let min_thickness = 0.5;

        let new_line: (IVec2, IVec2) = (points.last().copied().unwrap(), mouse_point_grid);
        // If this line crosses any existing line, it is invalid.
        for p in points.iter().copied() {
            if p == new_line.0 || p == new_line.1 {
                continue;
            }
            // Project p onto the line.
            let p_on_line =
                point_closest_to_segment(p.as_vec2(), (new_line.0.as_vec2(), new_line.1.as_vec2()));

            if p_on_line.distance(p.as_vec2()) < min_thickness {
                // This point is too close to the line.
                return false;
            }
        }

        if points.len() >= 2 {
            for i in 0..points.len() - 1 {
                if points[i] == mouse_point_grid || points[i + 1] == mouse_point_grid {
                    continue;
                }
                let existing_line = (points[i].as_vec2(), points[i + 1].as_vec2());
                let near = point_closest_to_segment(mouse_point_grid.as_vec2(), existing_line);
                if mouse_point_grid.as_vec2().distance(near) < min_thickness {
                    return false;
                }

                if i + 2 < points.len()
                    && segments_cross(existing_line, (new_line.0.as_vec2(), new_line.1.as_vec2()))
                {
                    return false;
                }
            }
        }

        if points.len() >= 2 {
            // If the corner is too sharp, then we have a problem.
            let a = points[points.len() - 2];
            let pivot = points[points.len() - 1];

            let angle = (a - pivot)
                .as_vec2()
                .normalize()
                .dot((mouse_point_grid - pivot).as_vec2().normalize())
                .acos();

            let max_movement = wall_thickness / (angle / 2.0).sin();

            if max_movement >= min_extended {
                return false;
            }
        }

        true
    })();

    // Place the point, if it is valid.
    if let Some(mouse_point_grid) = mouse_point_grid {
        gizmos.sphere(
            grid_to_world(mouse_point_grid),
            8.,
            if new_point_is_valid {
                color_speculative
            } else {
                color_invalid
            },
        );

        if mouse_button.just_pressed(MouseButton::Left) {
            if points.len() >= 3 && to_flat(mouse_point_grid) == points[0] && new_point_is_valid {
                // Complete the shape.
                editor_world
                    .buildings
                    .push(Building::new(editing_plane_y, std::mem::take(&mut *points)))
            } else if new_point_is_valid {
                points.push(to_flat(mouse_point_grid));
            } else {
                points.clear();
            }
        }
    }

    for i in 0..points.len() {
        let point_a = grid_to_world(from_flat(points[i], 0));
        gizmos.sphere(point_a, 12., color_active);
        let (point_b, color) = if i == points.len() - 1 {
            // From the last point, draw a line to the cursor.
            let Some(mouse_point_grid) = mouse_point_grid else {
                continue;
            };
            (
                grid_to_world(mouse_point_grid),
                if new_point_is_valid {
                    color_speculative
                } else {
                    color_invalid
                },
            )
        } else {
            (grid_to_world(from_flat(points[i + 1], 0)), color_active)
        };

        gizmos.line(point_a, point_b, color);
    }
}

trait BevyToNalgebra {
    type Point;
    fn to_point(&self) -> Self::Point;
    type Vector;
    fn to_vector(&self) -> Self::Vector;
}

impl BevyToNalgebra for Vec2 {
    type Point = nalgebra::Point2<f64>;

    fn to_point(&self) -> Self::Point {
        nalgebra::Point2::new(self.x as f64, self.y as f64)
    }

    type Vector = nalgebra::Vector2<f64>;

    fn to_vector(&self) -> Self::Vector {
        nalgebra::Vector2::new(self.x as f64, self.y as f64)
    }
}
impl BevyToNalgebra for Vec3 {
    type Point = nalgebra::Point3<f64>;

    fn to_point(&self) -> Self::Point {
        nalgebra::Point3::new(self.x as f64, self.y as f64, self.z as f64)
    }
    type Vector = nalgebra::Vector3<f64>;

    fn to_vector(&self) -> Self::Vector {
        nalgebra::Vector3::new(self.x as f64, self.y as f64, self.z as f64)
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

    fn grid_to_world(p: IVec3) -> Vec3 {
        p.as_vec3()
    }

    let mut out_buffer_csg: Vec<CSG> = Vec::new();
    let mut room_interior_csg: Vec<CSG> = Vec::new();

    struct RoomLayer<'a> {
        lift: f64,
        wall_width: f32,
        out: &'a mut Vec<CSG>,
    }

    let layers = [
        RoomLayer {
            lift: 0.,
            wall_width: 0.,
            out: &mut out_buffer_csg,
        },
        RoomLayer {
            lift: 0.4,
            wall_width: -0.1,
            out: &mut room_interior_csg,
        },
    ];

    for layer in layers {
        for room in world.buildings.iter() {
            let y_top = room.floor_y() + 2;
            let y_bot = room.floor_y();
            let points = room.points();
            let mut polygons: Vec<csgrs::polygon::Polygon<SurfaceDetail>> = Vec::new();

            fn from_flat(v: Vec2, y: i32) -> Vec3 {
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
                    Some(SurfaceDetail { outside: true }),
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
                    Some(SurfaceDetail { outside: true }),
                ));
            }

            layer.out.push(CSG::from_polygons(&polygons));
        }
    }

    println!("combining:");
    for outer in out_buffer_csg.iter() {
        println!("{:?}", outer);
    }
    for inner in room_interior_csg.iter() {
        println!("{:?}", inner);
    }

    let mut world_csg: CSG = CSG::new();
    for outer_csg in &out_buffer_csg {
        world_csg = world_csg.union(&outer_csg.tessellate());
    }

    for inner_csg in &room_interior_csg {
        world_csg = world_csg.difference(&inner_csg.tessellate());
    }

    println!("prepare to tesselate");

    rendered_csg.0 =
        world_csg
            .tessellate()
            .scale(VOXEL_SIZE as f64, VOXEL_SIZE as f64, VOXEL_SIZE as f64);

    println!("tesselated");
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
        let center = Vec3::new(center.x as f32, center.y as f32, center.z as f32);
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

    let mesh = to_bevy_mesh(world_csg, |face| true);
    let mesh_handle = meshes.add(mesh);

    if let Some(rendered) = rendered.take() {
        commands.entity(rendered).despawn();
    }

    *rendered = Some(
        commands
            .spawn((
                Mesh3d(mesh_handle),
                MeshMaterial3d(common.red_material.clone()),
                Transform::from_scale(Vec3::splat(1.)),
            ))
            .id(),
    );
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

    commands.insert_resource(common);

    commands.insert_resource(RenderedCsg(CSG::new()));

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
