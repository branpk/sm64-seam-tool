use crate::geo::{Point3f, Vector3f};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Scene {
    pub viewport: Viewport,
    pub camera: Camera,
    pub surfaces: Vec<Surface>,
    pub wall_hitbox_radius: f32,
    pub hovered_surface: Option<usize>,
    pub hidden_surfaces: HashSet<usize>,
}

#[derive(Debug, Clone)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
pub enum Camera {
    Rotate(RotateCamera),
    BirdsEye(BirdsEyeCamera),
}

#[derive(Debug, Clone)]
pub struct RotateCamera {
    pub pos: [f32; 3],
    pub target: [f32; 3],
    pub fov_y: f32,
}

#[derive(Debug, Clone)]
pub struct BirdsEyeCamera {
    pub pos: [f32; 3],
    pub span_y: f32,
}

#[derive(Debug, Clone)]
pub struct Surface {
    pub ty: SurfaceType,
    pub vertices: [[f32; 3]; 3],
    pub normal: [f32; 3],
}

impl Surface {
    pub fn normal(&self) -> Vector3f {
        Vector3f::from_row_slice(&self.normal)
    }

    pub fn vertices(&self) -> [Point3f; 3] {
        [
            Point3f::from_slice(&self.vertices[0]),
            Point3f::from_slice(&self.vertices[1]),
            Point3f::from_slice(&self.vertices[2]),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceType {
    Floor,
    Ceiling,
    WallXProj,
    WallZProj,
}
