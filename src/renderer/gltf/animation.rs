use crate::renderer::gltf::Gltf;
use glam::{Mat4, Quat, Vec3};

pub struct Animation {
    pub name: String,
    pub nodes_animations: Vec<Vec<NodeAnimation>>,
    pub start: f32,
    pub length: f32,
}

#[derive(Clone)]
pub struct NodeAnimation {
    pub timestamps: Vec<f32>,
    pub keyframes: Keyframes,
    pub interpolation: Interpolation,
}

#[derive(Clone)]
pub enum Keyframes {
    Translation(Vec<Vec3>),
    Rotation(Vec<Quat>),
    Scale(Vec<Vec3>),
}

#[derive(Clone, Copy)]
pub enum Interpolation {
    Step,
    Linear,
    CubicSpline,
}

pub struct NodeTransform<'a> {
    pub name: &'a str,
    pub transform: Mat4,
}

impl Gltf {
    pub fn get_node_transforms(&self) -> Vec<NodeTransform> {
        self.nodes
            .iter()
            .map(|node| NodeTransform {
                name: &node.name,
                transform: node.transform,
            })
            .collect::<Vec<_>>()
    }
}

impl Animation {
    pub fn animate_transforms(&self, transforms: &mut [NodeTransform], time: f32) {
        assert_eq!(self.nodes_animations.len(), transforms.len());
        for (i, animations) in self.nodes_animations.iter().enumerate() {
            if animations.is_empty() {
                continue;
            }
            let (mut s, mut r, mut t) = transforms[i].transform.to_scale_rotation_translation();
            for animation in animations {
                match &animation.keyframes {
                    Keyframes::Translation(keyframes) => {
                        t = sample_vec3_keyframes(
                            &animation.timestamps,
                            keyframes,
                            animation.interpolation,
                            time,
                        )
                    }
                    Keyframes::Rotation(keyframes) => {
                        r = sample_quat_keyframes(
                            &animation.timestamps,
                            keyframes,
                            animation.interpolation,
                            time,
                        )
                    }
                    Keyframes::Scale(keyframes) => {
                        s = sample_vec3_keyframes(
                            &animation.timestamps,
                            keyframes,
                            animation.interpolation,
                            time,
                        )
                    }
                }
            }
            transforms[i].transform = Mat4::from_scale_rotation_translation(s, r, t);
        }
    }
}

fn sample_vec3_keyframes(
    timestamps: &[f32],
    keyframes: &[Vec3],
    interpolation: Interpolation,
    t: f32,
) -> Vec3 {
    let v = keyframes;
    let mut i = timestamps.len();
    let t = t % timestamps[i - 1];
    for (i_, timestamps) in timestamps.windows(2).enumerate() {
        let start = timestamps[0];
        let end = timestamps[1];
        if start <= t && t < end {
            i = i_;
            break;
        }
    }
    assert!(i < timestamps.len());
    let t_k = timestamps[i];
    let t_d = timestamps[i + 1] - t_k;
    let t = (t - t_k) / t_d;
    match interpolation {
        Interpolation::Step => v[i],
        Interpolation::Linear => (1.0 - t) * v[i] + t * v[i + 1],
        Interpolation::CubicSpline => {
            let a_k1 = v[(i + 1) * 3];
            let v_k = v[i * 3 + 1];
            let v_k1 = v[(i + 1) * 3 + 1];
            let b_k = v[i * 3 + 2];
            (2.0 * t.powi(3) - 3.0 * t.powi(2) + 1.0) * v_k
                + t_d * (t.powi(3) - 2.0 * t.powi(2) + t) * b_k
                + (-2.0 * t.powi(3) + 3.0 * t.powi(2)) * v_k1
                + t_d * (t.powi(3) - t.powi(2)) * a_k1
        }
    }
}

fn sample_quat_keyframes(
    timestamps: &[f32],
    keyframes: &[Quat],
    interpolation: Interpolation,
    t: f32,
) -> Quat {
    let v = keyframes;
    let mut i = timestamps.len();
    let t = t % timestamps[i - 1];
    for (i_, timestamps) in timestamps.windows(2).enumerate() {
        let start = timestamps[0];
        let end = timestamps[1];
        if start <= t && t < end {
            i = i_;
            break;
        }
    }
    assert!(i < timestamps.len());
    let t_k = timestamps[i];
    let t_d = timestamps[i + 1] - t_k;
    let t = (t - t_k) / t_d;
    match interpolation {
        Interpolation::Step => v[i],
        Interpolation::Linear => {
            let dot = v[i].dot(v[i + 1]);
            let a = dot.abs().acos();
            let s = dot.signum();
            v[i] * ((a * (1.0 - t)).sin() / a.sin()) + v[i + 1] * (s * ((a * t).sin() / a.sin()))
        }
        Interpolation::CubicSpline => {
            let a_k1 = v[(i + 1) * 3];
            let v_k = v[i * 3 + 1];
            let v_k1 = v[(i + 1) * 3 + 1];
            let b_k = v[i * 3 + 2];
            v_k * (2.0 * t.powi(3) - 3.0 * t.powi(2) + 1.0)
                + b_k * (t_d * (t.powi(3) - 2.0 * t.powi(2) + t))
                + v_k1 * (-2.0 * t.powi(3) + 3.0 * t.powi(2))
                + a_k1 * (t_d * (t.powi(3) - t.powi(2)))
        }
    }
}
