use crate::{
    edge::Edge,
    game_state::GameState,
    geo::{direction_to_pitch_yaw, pitch_yaw_to_direction, Point3f, Vector3f},
    scene::RotateCamera,
    seam::Seam,
};
use std::f32::consts::PI;

pub fn get_mouse_ray(
    mouse_pos: [f32; 2],
    window_pos: [f32; 2],
    window_size: [f32; 2],
    camera: &RotateCamera,
) -> Option<(Point3f, Vector3f)> {
    let rel_mouse_pos = (
        mouse_pos[0] - window_pos[0],
        window_size[1] - mouse_pos[1] + window_pos[1],
    );
    let norm_mouse_pos = (
        2.0 * rel_mouse_pos.0 / window_size[0] - 1.0,
        2.0 * rel_mouse_pos.1 / window_size[1] - 1.0,
    );
    if norm_mouse_pos.0.abs() > 1.0 || norm_mouse_pos.1.abs() > 1.0 {
        return None;
    }

    let forward_dir = (camera.target() - camera.pos()).normalize();
    let (pitch, yaw) = direction_to_pitch_yaw(&forward_dir);
    let up_dir = pitch_yaw_to_direction(pitch + PI / 2.0, yaw);
    let right_dir = pitch_yaw_to_direction(0.0, yaw - PI / 2.0);

    let top = (camera.fov_y / 2.0).tan();
    let right = top * window_size[0] / window_size[1];

    let mouse_dir =
        (forward_dir + top * norm_mouse_pos.1 * up_dir + right * norm_mouse_pos.0 * right_dir)
            .normalize();

    Some((camera.pos(), mouse_dir))
}

pub fn ray_surface_intersection(
    state: &GameState,
    ray: (Point3f, Vector3f),
) -> Option<(usize, Point3f)> {
    let mut nearest: Option<(f32, (usize, Point3f))> = None;

    for (i, surface) in state.surfaces.iter().enumerate() {
        let normal = surface.normal();
        let vertices = surface.vertices();

        let t = -normal.dot(&(ray.0 - vertices[0])) / normal.dot(&ray.1);
        if t <= 0.0 {
            continue;
        }

        let p = ray.0 + t * ray.1;

        let mut interior = true;
        for k in 0..3 {
            let edge = vertices[(k + 1) % 3] - vertices[k];
            if normal.dot(&edge.cross(&(p - vertices[k]))) < 0.0 {
                interior = false;
                break;
            }
        }
        if !interior {
            continue;
        }

        if nearest.is_none() || t < nearest.unwrap().0 {
            nearest = Some((t, (i, p)));
        }
    }

    nearest.map(|(_, result)| result)
}

pub fn find_hovered_seam(
    state: &GameState,
    active_seams: &[Seam],
    mouse_ray: (Point3f, Vector3f),
) -> Option<Seam> {
    let (surface_index, point) = ray_surface_intersection(state, mouse_ray)?;
    let surface = &state.surfaces[surface_index];
    let vertices = [surface.vertex1, surface.vertex2, surface.vertex3];

    let mut nearest_edge: Option<(f32, Edge)> = None;
    for k in 0..3 {
        let fvertex1 = surface.vertices()[k];
        let fvertex2 = surface.vertices()[(k + 1) % 3];
        let edge_dir = (fvertex2 - fvertex1).normalize();
        let inward_dir = surface.normal().cross(&edge_dir);
        let distance = inward_dir.dot(&(point - fvertex1)).abs();

        if nearest_edge.is_none() || distance < nearest_edge.unwrap().0 {
            let edge = Edge::new((vertices[k], vertices[(k + 1) % 3]), surface.normal);
            nearest_edge = Some((distance, edge));
        }
    }

    let (_, edge) = nearest_edge?;
    active_seams
        .iter()
        .filter(|seam| seam.edge1 == edge || seam.edge2 == edge)
        .cloned()
        .next()
}
