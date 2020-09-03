use nalgebra::{Matrix4, Point3, Vector3, Vector4};

pub type Matrix4f = Matrix4<f32>;
pub type Point3f = Point3<f32>;
pub type Vector3f = Vector3<f32>;
pub type Vector4f = Vector4<f32>;

pub fn direction_to_pitch_yaw(dir: &Vector3f) -> (f32, f32) {
    let xz = (dir.x * dir.x + dir.z * dir.z).sqrt();
    let pitch = f32::atan2(dir.y, xz);
    let yaw = f32::atan2(dir.x, dir.z);
    (pitch, yaw)
}

pub fn pitch_yaw_to_direction(pitch: f32, yaw: f32) -> Vector3f {
    Vector3f::new(
        pitch.cos() * yaw.sin(),
        pitch.sin(),
        pitch.cos() * yaw.cos(),
    )
}

pub fn point_f32_to_f64(point: Point3f) -> Point3<f64> {
    Point3::new(point.x as f64, point.y as f64, point.z as f64)
}

pub fn point_f64_to_f32(point: Point3<f64>) -> Point3f {
    Point3::new(point.x as f32, point.y as f32, point.z as f32)
}
