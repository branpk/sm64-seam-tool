use crate::geo::Point3f;
use bytemuck::{Pod, Zeroable};

pub use imgui_renderer::*;
pub use renderer::*;
pub use scene::*;
pub use util::*;

mod game_view;
mod imgui_renderer;
mod pipelines;
mod renderer;
mod scene;
mod seam_view;
mod util;

const NUM_OUTPUT_SAMPLES: u32 = 4;
const DEPTH_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

#[derive(Debug, Clone, Copy, Default)]
struct Vertex {
    pos: [f32; 3],
    color: [f32; 4],
}

impl Vertex {
    fn new(pos: Point3f, color: [f32; 4]) -> Self {
        Self {
            pos: [pos.x, pos.y, pos.z],
            color,
        }
    }
}

unsafe impl Zeroable for Vertex {}
unsafe impl Pod for Vertex {}
