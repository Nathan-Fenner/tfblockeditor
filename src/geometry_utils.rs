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
