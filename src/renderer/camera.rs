use std::f32::consts::{FRAC_PI_2, TAU};

use glam::{Mat4, Quat, Vec3};

pub struct Camera {
    /// Distance from the focus point.
    pub distance: f32,
    /// The rotation of the camera around the focus point, in radians.
    pub yaw: f32,
    /// The rotation of the camera around the x-axis, i.e. how up/down it looks.
    pub pitch: f32,
    /// The point at the center of the screen.
    pub focus: Vec3,
}

impl Camera {
    pub fn new() -> Camera {
        Camera {
            distance: 30.0,
            yaw: TAU * 0.25,
            pitch: FRAC_PI_2 * 0.7,
            focus: Vec3::Y * 1.5,
        }
    }

    pub fn view_matrix(&self) -> Mat4 {
        let camera_rot =
            Quat::from_rotation_x(-self.pitch) * Quat::from_rotation_y(self.yaw + TAU / 2.0);
        let camera_pos = self.focus
            + Quat::from_rotation_y(-self.yaw)
                * Quat::from_rotation_x(-self.pitch)
                * Vec3::Z
                * self.distance;
        Mat4::from_quat(-camera_rot) * Mat4::from_translation(-camera_pos)
    }
}
