use super::{BirdsEyeCamera, RotateCamera, Viewport};
use crate::{
    edge::{Orientation, ProjectionAxis},
    geo::{direction_to_pitch_yaw, Matrix4f, Point3f, Vector3f, Vector4f},
    seam::RangeStatus,
};
use nalgebra::distance;
use std::f32::consts::PI;

pub fn rotate_transforms(camera: &RotateCamera, viewport: &Viewport) -> (Matrix4f, Matrix4f) {
    let camera_pos = Point3f::new(camera.pos[0], camera.pos[1], camera.pos[2]);
    let target_pos = Point3f::new(camera.target[0], camera.target[1], camera.target[2]);

    let dist_to_target = distance(&camera_pos, &target_pos);
    let dist_to_far_corner = distance(
        &Point3f::from(camera_pos.coords.abs()),
        &Point3f::new(-8191.0, -8191.0, -8191.0),
    );
    let far = dist_to_far_corner * 0.95;
    let near = (dist_to_target * 0.1).min(1000.0);
    let proj_matrix =
        Matrix4f::new_perspective(viewport.width / viewport.height, camera.fov_y, near, far);

    let (pitch, yaw) = direction_to_pitch_yaw(&(target_pos - camera_pos));

    let view_matrix = Matrix4f::new_rotation(PI * Vector3f::y())
        * Matrix4f::new_rotation(pitch * Vector3f::x())
        * Matrix4f::new_rotation(-yaw * Vector3f::y())
        * Matrix4f::new_translation(&-camera_pos.coords);

    (proj_matrix, view_matrix)
}

pub fn birds_eye_transforms(camera: &BirdsEyeCamera, viewport: &Viewport) -> (Matrix4f, Matrix4f) {
    // world x = screen up, world z = screen right
    let rotation =
        Matrix4f::from_columns(&[Vector4f::y(), -Vector4f::z(), Vector4f::x(), Vector4f::w()]);
    let scaling = Matrix4f::new_nonuniform_scaling(&Vector3f::new(
        2.0 / (camera.span_y * viewport.width / viewport.height),
        2.0 / camera.span_y,
        1.0 / 40_000.0,
    ));
    let proj_matrix = scaling * rotation;

    let view_matrix = Matrix4f::new_translation(&-Vector3f::from_row_slice(&camera.pos));

    (proj_matrix, view_matrix)
}

pub fn seam_transforms(
    camera: &BirdsEyeCamera,
    viewport: &Viewport,
    projection_axis: ProjectionAxis,
    orientation: Orientation,
) -> (Matrix4f, Matrix4f) {
    let w_axis = match projection_axis {
        ProjectionAxis::X => Vector4f::z(),
        ProjectionAxis::Z => Vector4f::x(),
    };
    let screen_right = match orientation {
        Orientation::Positive => -w_axis,
        Orientation::Negative => w_axis,
    };

    let rotation =
        Matrix4f::from_columns(&[screen_right, Vector4f::y(), nalgebra::zero(), Vector4f::w()])
            .transpose();
    let scaling = Matrix4f::new_nonuniform_scaling(&Vector3f::new(
        2.0 / (camera.span_y * viewport.width / viewport.height),
        2.0 / camera.span_y,
        1.0,
    ));
    let proj_matrix = scaling * rotation;

    let view_matrix = Matrix4f::new_translation(&-Vector3f::from_row_slice(&camera.pos));

    (proj_matrix, view_matrix)
}

pub fn seam_segment_color(status: RangeStatus) -> [f32; 4] {
    match status {
        RangeStatus::Checked {
            has_gap: false,
            has_overlap: false,
        } => [1.0, 1.0, 1.0, 1.0],
        RangeStatus::Checked {
            has_gap: true,
            has_overlap: false,
        } => [0.0, 1.0, 0.0, 1.0],
        RangeStatus::Checked {
            has_gap: false,
            has_overlap: true,
        } => [0.0, 0.0, 1.0, 1.0],
        RangeStatus::Checked {
            has_gap: true,
            has_overlap: true,
        } => [0.0, 1.0, 1.0, 1.0],
        RangeStatus::Unchecked => [0.1, 0.1, 0.1, 1.0],
        RangeStatus::Skipped => [1.0, 0.0, 0.0, 1.0],
    }
}
