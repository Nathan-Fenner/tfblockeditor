use bevy::prelude::*;

#[derive(Copy, Clone, Debug)]
pub struct CuttingPlane {
    pub point: Vec3,
    /// An outward-facing normal.
    pub normal: Vec3,
}

/// An infinite bidirectional line.
#[derive(Copy, Clone, Debug)]
pub struct InfiniteLine {
    pub point: Vec3,
    pub direction: Vec3,
}

const EPSILON: f32 = 0.0001;

impl CuttingPlane {
    /// Flips the normal of the plane.
    pub fn flipped(self) -> Self {
        Self {
            point: self.point,
            normal: -self.normal,
        }
    }
    /// Signed distance to a point
    pub fn signed_distance(&self, point: Vec3) -> f32 {
        (point - self.point).dot(self.normal)
    }

    /// Returns a plane containing the three points, if it is not degenerate.
    pub fn from_triangle(points: [Vec3; 3]) -> Option<Self> {
        let d1 = points[1] - points[0];
        let d2 = points[2] - points[0];
        if d1.length() < EPSILON || d2.length() < EPSILON {
            return None;
        }
        let d1 = d1.normalize();
        let d2 = d2.normalize();
        let normal = d1.cross(d2);
        if normal.length() < EPSILON {
            return None;
        }

        Some(Self {
            point: points[0],
            normal: normal.normalize(),
        })
    }

    /// Computes the intersection of two planes.
    /// If the planes are parallel or coincide, returns `None`.
    pub fn intersection_plane(&self, other: &CuttingPlane) -> Option<InfiniteLine> {
        let perp = self.normal.cross(other.normal);
        if perp.length() < EPSILON {
            return None;
        }
        // A point X lies on the plane if (X - P1) dot N1 = 0
        // We want this to hold simultaneously for both planes, so:
        // (X - P1) dot N1 = 0
        // (X - P2) dot N2 = 0

        // We know that the solution line lies on both planes, so its direction is
        // perp to both normals. So, `perp` is the line's direction.

        // The vectors {N1, N2, perp} span the plane, since it's non-degenerate.

        // So a solution point X has the form `a * N1 + b * N2 + c * perp`.
        // Since movement in the `perp` direction doesn't change the solution,
        // we can ignore the parameter `c` and solve only for `a` and `b`.

        // (a N1 + b N2 - P1) dot N1 = 0
        // (a N1 + b N2 - P2) dot N2 = 0

        // Which expands to

        // a (N1 dot N1) + b (N2 dot N1) - (P1 dot N1) = 0
        // a (N1 dot N2) + b (N2 dot N2) - (P2 dot N2) = 0

        let intersection_point = (other.normal.dot(other.point) * self.normal
            - self.normal.dot(self.point) * other.normal)
            .cross(-perp)
            / perp.length_squared();

        assert!(self.signed_distance(intersection_point).abs() <= EPSILON);
        assert!(other.signed_distance(intersection_point).abs() <= EPSILON);

        Some(InfiniteLine {
            point: intersection_point,
            direction: perp.normalize(),
        })
    }

    /// Returns the intersection with the line.
    /// If the line lies within the plane or never intersects it, returns `None`.
    pub fn intersection_line(&self, line: &InfiniteLine) -> Option<Vec3> {
        if self.normal.dot(line.direction).abs() < EPSILON {
            return None;
        }

        // (p + t * d - p1) dot n1 = 0
        // p dot n1 + t * d dot n1 - p1 dot n1 = 0
        // t = (p1 dot n1 - p dot n1) / d dot n1

        let t = (self.point.dot(self.normal) - line.point.dot(self.normal))
            / line.direction.dot(self.normal);

        let intersection = line.point + line.direction * t;
        assert!(self.signed_distance(intersection).abs() < EPSILON);
        Some(intersection)
    }
}

#[derive(Clone, Debug)]
pub struct ConvexHull {
    pub planes: Vec<CuttingPlane>,
}

impl ConvexHull {
    /// The signed distance to the voxel hull.
    /// Negative if in interior.
    /// Positive if in exterior.
    /// Zero if on edge or face.
    pub fn signed_distance(&self, point: Vec3) -> f32 {
        self.planes
            .iter()
            .map(|plane| plane.signed_distance(point))
            .fold(f32::NEG_INFINITY, |a, b| a.max(b))
    }

    /// This slow method computes the vertices of the convex hull.
    pub fn vertices(&self) -> Vec<Vec3> {
        let mut candidates: Vec<Vec3> = Vec::new();
        for (index1, plane1) in self.planes.iter().enumerate() {
            for (index2, plane2) in self.planes.iter().enumerate() {
                if index2 >= index1 {
                    break;
                }
                for (index3, plane3) in self.planes.iter().enumerate() {
                    if index3 >= index2 {
                        break;
                    }

                    let Some(line) = plane1.intersection_plane(plane2) else {
                        continue;
                    };

                    let Some(point) = plane3.intersection_line(&line) else {
                        continue;
                    };

                    if self.signed_distance(point).abs() < EPSILON {
                        candidates.push(point);
                    }
                }
            }
        }

        candidates
    }

    /// Removes redundant planes.
    /// If this leaves a degenerate, zero-volume area, returns `None`.
    pub fn simplify(mut self) -> Option<Self> {
        let vertices = self.vertices();

        self.planes.retain(|plane| {
            let touching = vertices
                .iter()
                .filter(|v| plane.signed_distance(**v).abs() <= EPSILON)
                .count();
            touching >= 3
        });

        if self.planes.len() <= 3 {
            return None;
        }

        Some(self)
    }

    pub fn from_points(points: &[Vec3]) -> Option<Self> {
        let mut planes: Vec<CuttingPlane> = Vec::new();
        for (ai, a) in points.iter().enumerate() {
            for (bi, b) in points.iter().enumerate() {
                if bi >= ai {
                    break;
                }
                for (ci, c) in points.iter().enumerate() {
                    if ci >= bi {
                        break;
                    }

                    if let Some(plane) = CuttingPlane::from_triangle([*a, *b, *c]) {
                        planes.push(plane);
                    }
                }
            }
        }

        planes.retain_mut(|plane| {
            // Find the plane's distance to points.
            let mut min_distance: f32 = 0.0;
            let mut max_distance: f32 = 0.0;
            for p in points.iter() {
                let dist = plane.signed_distance(*p);
                min_distance = min_distance.min(dist);
                max_distance = max_distance.max(dist);
            }

            if min_distance >= -EPSILON && max_distance <= EPSILON {
                // All points lie on this plane.
                return false;
            }

            if min_distance < -EPSILON && max_distance > EPSILON {
                // Points lie on both sides of this plane.
                return false;
            }

            if max_distance > EPSILON {
                // The plane is backward.
                *plane = plane.flipped();
            }

            true
        });

        Self { planes }.simplify()
    }
}

#[test]
fn test_convex_hull() {
    let points = [
        Vec3::new(3., 2., 1.),
        Vec3::new(7., 17., 12.),
        Vec3::new(5., 4., 7.),
        Vec3::new(5., 5., 5.),
    ];
    let hull = ConvexHull::from_points(&points);
    let hull = hull.unwrap();

    for p in &points {
        assert!(hull.signed_distance(*p).abs() < EPSILON);
    }
}
