use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::view::RenderLayers;

use crate::building::{
    Building, BuildingValidity, Corner, MIN_INTERIOR_THICKNESS, is_corner_too_sharp,
};
use crate::common_assets::Common;
use crate::editor_state::{
    EditorTool, EditorWorld, from_flat, grid_to_world, to_flat, world_to_grid,
};
use crate::geometry_utils::{point_closest_to_segment, segments_cross, signed_polygon_area_2d};
use crate::preview::Previewer;
use crate::voxels::VOXEL_SIZE;

pub struct EditorActionPlugin;

impl Plugin for EditorActionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                switch_tool_system,
                move_building_system,
                editor_insert_building_system,
                preview_xray_buildings_system,
            )
                .chain(),
        );
    }
}

pub fn switch_tool_system(mut editor_world: ResMut<EditorWorld>, keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::Digit1) {
        editor_world.set_tool(EditorTool::SelectBuilding);
    }
    if keys.just_pressed(KeyCode::Digit2) {
        editor_world.set_tool(EditorTool::CreateBuilding);
    }
}

struct DraggingState {
    building_index: usize,
    point_index: usize,
}

fn move_building_system(
    mouse_grid: MouseGrid,
    mut editor_world: ResMut<EditorWorld>,
    common: Res<Common>,
    mut preview: Local<Previewer<IVec3>>,
    mut commands: Commands,

    mut dragging: Local<Option<DraggingState>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
) {
    let mut preview = preview.collect_scope(&mut commands);

    if !matches!(editor_world.tool(), EditorTool::SelectBuilding) {
        if dragging.is_some() {
            *dragging = None;
        }
        return;
    }

    if dragging.is_some() && !mouse_button.pressed(MouseButton::Left) {
        *dragging = None;
    }

    let editing_plane_y = 0;

    let Some(mouse) = mouse_grid.pick_grid(editing_plane_y) else {
        return;
    };

    if mouse_button.just_pressed(MouseButton::Left) {
        // Find the selected point, if any.
        for (building_index, building) in editor_world.buildings().iter().enumerate() {
            for (point_index, point) in building.points().iter().enumerate() {
                if *point == mouse.xz() {
                    *dragging = Some(DraggingState {
                        building_index,
                        point_index,
                    });
                }
            }
        }
    }

    if let Some(dragging_state) = dragging.as_ref() {
        let building = &editor_world.buildings()[dragging_state.building_index];

        let mouse_point = mouse.xz();

        if building.points()[dragging_state.point_index] != mouse_point {
            let mut new_building = building.clone();
            new_building.outline[dragging_state.point_index] = mouse_point;
            if new_building.is_valid(BuildingValidity::default()) {
                editor_world.set_building_point(
                    dragging_state.building_index,
                    dragging_state.point_index,
                    mouse_point,
                );
            }
        }
    }

    let world_mouse = grid_to_world(mouse);

    preview.render(&mouse, |commands| {
        commands
            .spawn((
                Transform::from_translation(world_mouse)
                    .with_scale(Vec3::new(0.4, 0.005, 0.4) * VOXEL_SIZE),
                Mesh3d(common.cube_mesh.clone()),
                MeshMaterial3d(common.ui_gold_material.clone()),
                RenderLayers::layer(7),
            ))
            .id()
    });
}

/// A system parameter for getting the mouse position in the world grid.
#[derive(SystemParam)]
pub struct MouseGrid<'w> {
    ray_map: Res<'w, bevy::picking::backend::ray::RayMap>,
}

impl MouseGrid<'_> {
    fn pick_grid(&self, editing_plane_y: i32) -> Option<IVec3> {
        let mouse_ray = self.ray_map.iter().next().map(|r| *r.1);

        let max_pick_distance = 10_000.0;

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

        mouse_point.map(world_to_grid)
    }
}
/// Runs the `EditorTool::CreateBuilding` tool.
pub fn editor_insert_building_system(
    mut gizmos: Gizmos,
    mouse_grid: MouseGrid,
    mouse_button: Res<ButtonInput<MouseButton>>,

    mut editor_world: ResMut<EditorWorld>,
    keys: Res<ButtonInput<KeyCode>>,
    mut points: Local<Vec<IVec2>>,
) {
    if !matches!(editor_world.tool(), EditorTool::CreateBuilding) {
        if !points.is_empty() {
            points.clear();
        }
        return;
    }

    let color_active = Color::linear_rgb(1., 1., 0.);
    let color_speculative = Color::linear_rgb(0., 0., 1.);
    let color_invalid = Color::linear_rgb(1., 0., 0.);

    if keys.just_pressed(KeyCode::Escape) {
        points.clear();
    }

    let editing_plane_y = 0;
    let mouse_point_grid = mouse_grid.pick_grid(editing_plane_y);

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
            if is_corner_too_sharp(Corner {
                a: points[1],
                pivot: points[0],
                b: points[points.len() - 1],
            }) {
                return false;
            }
        }

        if mouse_point_grid != points[0] && points.contains(&mouse_point_grid) {
            return false;
        }

        let new_line: (IVec2, IVec2) = (points.last().copied().unwrap(), mouse_point_grid);
        // If this line crosses any existing line, it is invalid.
        for p in points.iter().copied() {
            if p == new_line.0 || p == new_line.1 {
                continue;
            }
            // Project p onto the line.
            let p_on_line =
                point_closest_to_segment(p.as_vec2(), (new_line.0.as_vec2(), new_line.1.as_vec2()));

            if p_on_line.distance(p.as_vec2()) < MIN_INTERIOR_THICKNESS {
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
                if mouse_point_grid.as_vec2().distance(near) < MIN_INTERIOR_THICKNESS {
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
            let b = mouse_point_grid;

            if is_corner_too_sharp(Corner { a, pivot, b }) {
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
                // Create the new shape and insert it into the editor.

                let mut points = std::mem::take(&mut *points);

                if signed_polygon_area_2d(&points) < 0.0 {
                    points.reverse();
                }

                editor_world.insert_building(Building::new(editing_plane_y, points))
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

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
enum XrayPreview {
    Segment(IVec3, IVec3),
    Point(IVec3),
}

fn preview_xray_buildings_system(
    mut commands: Commands,
    mut preview: Local<Previewer<XrayPreview>>,
    common: Res<Common>,
    editor_world: Res<EditorWorld>,
) {
    if !editor_world.is_changed() {
        return;
    }

    let mut preview = preview.collect_scope(&mut commands);

    for building in editor_world.buildings() {
        let points = building.points();

        for i in 0..points.len() {
            let p = points[i];
            let q = points[(i + 1) % points.len()];

            let p = from_flat(p, 0);
            let q = from_flat(q, 0);

            let world_p = grid_to_world(p);
            let world_q = grid_to_world(q);

            preview.render(&XrayPreview::Segment(p, q), |commands| {
                commands
                    .spawn((
                        Transform::from_translation((world_p + world_q) / 2.)
                            .with_scale(Vec3::new(
                                0.1 * VOXEL_SIZE,
                                0.005,
                                world_p.distance(world_q),
                            ))
                            .looking_at(world_p, Vec3::Y),
                        Mesh3d(common.cube_mesh.clone()),
                        MeshMaterial3d(common.xray_blue_material.clone()),
                        RenderLayers::layer(7),
                    ))
                    .id()
            });
        }
        for &p in points.iter() {
            let p = from_flat(p, 0);
            preview.render(&XrayPreview::Point(p), |commands| {
                commands
                    .spawn((
                        Transform::from_translation(grid_to_world(p))
                            .with_scale(Vec3::new(0.2, 0.01, 0.2) * VOXEL_SIZE),
                        Mesh3d(common.cube_mesh.clone()),
                        MeshMaterial3d(common.xray_blue_material.clone()),
                        RenderLayers::layer(7),
                    ))
                    .id()
            });
        }
    }
}
