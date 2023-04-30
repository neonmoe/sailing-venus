//! Structs and funcs glam doesn't provide.

use glam::Vec2;

#[derive(Clone, Copy, PartialEq)]
pub struct Aabb2 {
    /// Inclusive minimum coordinate
    pub min: Vec2,
    /// Exlucisve maximum coordinate
    pub max: Vec2,
}

impl Aabb2 {
    #[track_caller]
    pub fn new(min: Vec2, max: Vec2) -> Aabb2 {
        debug_assert!(min.cmple(max).all());
        Aabb2 { min, max }
    }

    pub fn contains(&self, point: Vec2) -> bool {
        self.min.cmple(point).all() && self.max.cmpgt(point).all()
    }

    pub fn offset(&self, by: Vec2) -> Aabb2 {
        Aabb2::new(self.min + by, self.max + by)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounds() {
        let bounds = Aabb2::new(Vec2::new(1.0, 1.0), Vec2::new(2.0, 2.0));
        assert!(bounds.contains(Vec2::new(1.0, 1.0)));
        assert!(bounds.contains(Vec2::new(1.5, 1.5)));
        assert!(!bounds.contains(Vec2::new(2.0, 2.0)));
        assert!(!bounds.contains(Vec2::new(2.5, 0.5)));
        assert!(!bounds.contains(Vec2::new(1.5, 2.0)));
    }
}
