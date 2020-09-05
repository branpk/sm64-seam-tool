use crate::{
    edge::ProjectedPoint,
    geo::{Point3f, Vector3f},
    seam::{RangeStatus, Seam},
};
use nalgebra::{Point3, Vector3};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub enum Scene {
    GameView(GameViewScene),
    SeamView(SeamViewScene),
}

#[derive(Debug, Clone)]
pub struct GameViewScene {
    pub viewport: Viewport,
    pub camera: Camera,
    pub surfaces: Vec<Surface>,
    pub wall_hitbox_radius: f32,
    pub hovered_surface: Option<usize>,
    pub hidden_surfaces: HashSet<usize>,
    pub seams: Vec<SeamInfo>,
    pub hovered_seam: Option<Seam>,
}

#[derive(Debug, Clone)]
pub struct SeamViewScene {
    pub viewport: Viewport,
    pub camera: SeamViewCamera,
    pub seam: SeamInfo,
    pub vertical_grid_lines: Vec<Point3<f64>>,
    pub horizontal_grid_lines: Vec<Point3<f64>>,
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

impl RotateCamera {
    pub fn pos(&self) -> Point3f {
        Point3f::new(self.pos[0], self.pos[1], self.pos[2])
    }

    pub fn target(&self) -> Point3f {
        Point3f::new(self.target[0], self.target[1], self.target[2])
    }
}

#[derive(Debug, Clone)]
pub struct BirdsEyeCamera {
    pub pos: [f32; 3],
    pub span_y: f32,
}

#[derive(Debug, Clone)]
pub struct SeamViewCamera {
    pub pos: Point3<f64>,
    pub span_y: f64,
    pub right_dir: Vector3<f64>,
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

#[derive(Debug, Clone)]
pub struct SeamInfo {
    pub seam: Seam,
    pub segments: Vec<SeamSegment>,
}

#[derive(Debug, Clone)]
pub struct SeamSegment {
    pub endpoint1: [f32; 3],
    pub endpoint2: [f32; 3],
    pub proj_endpoint1: ProjectedPoint<f32>,
    pub proj_endpoint2: ProjectedPoint<f32>,
    pub status: RangeStatus,
}

impl SeamSegment {
    pub fn endpoint1(&self) -> Point3f {
        Point3f::new(self.endpoint1[0], self.endpoint1[1], self.endpoint1[2])
    }

    pub fn endpoint2(&self) -> Point3f {
        Point3f::new(self.endpoint2[0], self.endpoint2[1], self.endpoint2[2])
    }
}
