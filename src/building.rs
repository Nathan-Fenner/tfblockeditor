use bevy::{platform::collections::HashSet, prelude::*};

use crate::geometry_utils::{point_closest_to_segment, segments_cross, signed_polygon_area_2d};

#[derive(Clone, Debug)]
pub struct Building {
    /// The y position of the base of the building.
    pub floor_y: i32,

    /// The points making up the building.
    pub outline: Vec<IVec2>,
}

#[derive(Default)]
pub struct BuildingValidity {
    /// Allow a building with only 1 point.
    allow_one_point: bool,
    /// Allow a building with only 2 points.
    allow_two_points: bool,
}

pub const BUILDING_WALL_THICKNESS: f32 = 0.125;
pub const MIN_EXTENDED: f32 = 0.45;
pub const MIN_INTERIOR_THICKNESS: f32 = 0.5;

#[derive(Copy, Clone, Debug)]
pub struct Corner {
    pub a: IVec2,
    pub pivot: IVec2,
    pub b: IVec2,
}

/// If a corner is too sharp, then it will lead to degenerate solids.
/// Do not allow this to happen.
pub fn is_corner_too_sharp(corner: Corner) -> bool {
    let a = corner.a;
    let b = corner.b;
    let pivot = corner.pivot;

    // If the corner is too sharp, then we have a problem.
    let angle = (a - pivot)
        .as_vec2()
        .normalize()
        .dot((b - pivot).as_vec2().normalize())
        .acos();

    let max_movement = BUILDING_WALL_THICKNESS / (angle / 2.0).sin();

    max_movement >= MIN_EXTENDED
}

impl Building {
    pub fn new(floor_y: i32, outline: Vec<IVec2>) -> Self {
        assert!(
            outline.len() >= 3,
            "floor outline must contain at least 3 points"
        );
        assert!(
            outline.iter().copied().collect::<HashSet<_>>().len() == outline.len(),
            "floor outline must have no duplicate points"
        );
        Self { floor_y, outline }
    }

    pub fn floor_y(&self) -> i32 {
        self.floor_y
    }

    /// Returns the points making up the building.
    pub fn points(&self) -> &[IVec2] {
        &self.outline
    }

    pub fn points_mut(&mut self) -> &mut Vec<IVec2> {
        &mut self.outline
    }

    /// Returns whether the arrangement of points in this building is valid.
    pub fn is_valid(&self, options: BuildingValidity) -> bool {
        let len = self.outline.len();
        if len == 1 && !options.allow_one_point {
            return false;
        }
        if len == 2 && !options.allow_two_points {
            return false;
        }

        if len >= 3 {
            for pivot_index in 0..len {
                let a = self.outline[(pivot_index + len - 1) % len];
                let pivot = self.outline[pivot_index];
                let b = self.outline[(pivot_index + 1) % len];

                if is_corner_too_sharp(Corner { a, pivot, b }) {
                    return false;
                }
            }
        }

        for i in 0..len {
            for j in 0..i {
                if self.outline[i] == self.outline[j] {
                    return false;
                }
            }
        }

        // Look for points (nearly) coincident to other segments.
        for &p in self.outline.iter() {
            for i in 0..len {
                let a = self.outline[i];
                let b = self.outline[(i + 1) % len];
                if a == p || b == p {
                    continue;
                }

                // Project p onto the line.
                let p_on_line = point_closest_to_segment(p.as_vec2(), (a.as_vec2(), b.as_vec2()));

                if p_on_line.distance(p.as_vec2()) < MIN_INTERIOR_THICKNESS {
                    // This point is too close to the line.
                    return false;
                }
            }
        }

        let points = &self.outline;

        // Forbid crossing segments.
        for i in 0..points.len() {
            let a1 = points[i];
            let b1 = points[(i + 1) % points.len()];
            for j in 0..points.len() {
                let a2 = points[j];
                let b2 = points[(j + 1) % points.len()];

                if a1 == a2 || a1 == b2 || b1 == a2 || b1 == b2 {
                    continue;
                }
                if segments_cross((a1.as_vec2(), b1.as_vec2()), (a2.as_vec2(), b2.as_vec2())) {
                    return false;
                }
            }
        }

        if signed_polygon_area_2d(&self.outline) <= 0.0 {
            return false;
        }

        true
    }
}
