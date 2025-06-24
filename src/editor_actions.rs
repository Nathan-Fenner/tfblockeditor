use bevy::prelude::*;
use bevy::render::view::RenderLayers;

use crate::building::Building;
use crate::common_assets::Common;
use crate::editor_state::{EditorWorld, from_flat, grid_to_world, to_flat, world_to_grid};
use crate::geometry_utils::{point_closest_to_segment, segments_cross, signed_polygon_area_2d};
use crate::preview::Previewer;
use crate::voxels::VOXEL_SIZE;

pub struct EditorActionPlugin;

impl Plugin for EditorActionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (editor_insert_building_system, preview_xray_buildings_system).chain(),
        );
    }
}

pub fn editor_insert_building_system(
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
    println!("rescan buildings");

    for building in editor_world.buildings() {
        let points = building.points();

        for i in 0..points.len() {
            let p = points[i];
            let q = points[(i + 1) % points.len()];

            let p = from_flat(p, 0);
            let q = from_flat(q, 0);

            let world_p = grid_to_world(p);
            let world_q = grid_to_world(q);

            preview.render(&XrayPreview::Segment(p, q), || {
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
            preview.render(&XrayPreview::Point(p), || {
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
