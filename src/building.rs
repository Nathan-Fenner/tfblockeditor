use bevy::{platform::collections::HashSet, prelude::*};

#[derive(Clone, Debug)]
pub struct Building {
    /// The y position of the base of the building.
    floor_y: i32,

    /// The points making up the building.
    outline: Vec<IVec2>,
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
}
