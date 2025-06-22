use bevy::prelude::*;

pub trait As2d {
    fn coord_x(&self) -> f32;
    fn coord_y(&self) -> f32;
}

impl As2d for Vec2 {
    fn coord_x(&self) -> f32 {
        self.x
    }
    fn coord_y(&self) -> f32 {
        self.y
    }
}
impl As2d for IVec2 {
    fn coord_x(&self) -> f32 {
        self.x as f32
    }
    fn coord_y(&self) -> f32 {
        self.y as f32
    }
}

pub fn signed_polygon_area_2d(points: &[impl As2d]) -> f32 {
    let mut sum = 0.0;
    if points.len() <= 2 {
        return 0.0;
    }
    for i in 0..points.len() {
        let j = (i + 1) % points.len();
        sum += (points[i].coord_y() + points[j].coord_y())
            * (points[i].coord_x() - points[j].coord_x());
    }
    sum * 0.5
}
pub trait BevyToNalgebra {
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

pub fn project_onto_v2(a: Vec2, (p, q): (Vec2, Vec2)) -> Vec2 {
    (a - p).dot((q - p).normalize()) * (q - p).normalize() + p
}

pub fn project_onto_i2(a: IVec2, (p, q): (IVec2, IVec2)) -> Vec2 {
    project_onto_v2(a.as_vec2(), (p.as_vec2(), q.as_vec2()))
}
pub fn point_closest_to_segment(p: Vec2, line: (Vec2, Vec2)) -> Vec2 {
    // Project p onto the line.
    let p_on_line = project_onto_v2(p, line);

    let d = (line.1 - line.0).normalize();

    let t = (p_on_line - line.0).dot(d) / (line.0.distance(line.1));
    let t = t.clamp(0.0, 1.0);

    line.0.lerp(line.1, t)
}
pub fn segments_cross(a: (Vec2, Vec2), b: (Vec2, Vec2)) -> bool {
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
