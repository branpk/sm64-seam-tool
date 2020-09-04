use super::{
    pipelines::Pipelines,
    seam_view_world_to_screen,
    util::{birds_eye_transforms, seam_segment_color},
    SeamInfo, SeamViewCamera, SeamViewScene, Vertex,
};
use crate::geo::{point_f32_to_f64, Matrix4f, Point3f, Vector3f};
use bytemuck::cast_slice;
use nalgebra::{Point3, Vector3};
use wgpu::util::DeviceExt;

pub struct SeamViewSceneBundle<'a> {
    scene: &'a SeamViewScene,
    transform_bind_group: wgpu::BindGroup,
    seam_vertex_buffer: (usize, wgpu::Buffer),
}

impl<'a> SeamViewSceneBundle<'a> {
    pub fn build(
        scene: &'a SeamViewScene,
        device: &wgpu::Device,
        transform_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let proj_matrix_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: cast_slice(Matrix4f::identity().as_slice()),
            usage: wgpu::BufferUsage::UNIFORM,
        });
        let view_matrix_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: cast_slice(Matrix4f::identity().as_slice()),
            usage: wgpu::BufferUsage::UNIFORM,
        });
        let transform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &transform_bind_group_layout,
            entries: &[
                // u_Proj
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: proj_matrix_buffer.as_entire_binding(),
                },
                // u_View
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: view_matrix_buffer.as_entire_binding(),
                },
            ],
        });

        let seam_vertices = get_seam_vertices(scene);
        let seam_vertex_buffer = (
            seam_vertices.len(),
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: cast_slice(&seam_vertices),
                usage: wgpu::BufferUsage::VERTEX,
            }),
        );

        Self {
            scene,
            transform_bind_group,
            seam_vertex_buffer,
        }
    }

    pub fn draw<'p>(
        &'p self,
        render_pass: &mut wgpu::RenderPass<'p>,
        pipelines: &'p Pipelines,
        output_size: (u32, u32),
    ) {
        let mut viewport = self.scene.viewport.clone();
        viewport.width = viewport.width.min(output_size.0 as f32 - viewport.x);
        viewport.height = viewport.height.min(output_size.1 as f32 - viewport.y);

        render_pass.set_viewport(
            viewport.x,
            viewport.y,
            viewport.width,
            viewport.height,
            0.0,
            1.0,
        );
        render_pass.set_scissor_rect(
            viewport.x as u32,
            viewport.y as u32,
            viewport.width as u32,
            viewport.height as u32,
        );

        render_pass.set_bind_group(0, &self.transform_bind_group, &[]);

        render_pass.set_pipeline(&pipelines.seam);
        render_pass.set_vertex_buffer(0, self.seam_vertex_buffer.1.slice(..));
        render_pass.draw(0..self.seam_vertex_buffer.0 as u32, 0..1);
    }
}

fn get_seam_vertices(scene: &SeamViewScene) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    // let slope = scene.seam.seam.edge1.slope() as f64;
    // let thickness = 0.03 * (slope * slope + 1.0).sqrt();
    // let screen_thickness_offset = thickness * Vector3::y();
    let thickness_offset = 0.03 * Vector3::y() * scene.camera.span_y / 2.0; //screen_thickness_offset * scene.camera.span_y / 2.0;

    let vertex = |pos: Point3<f64>, color: [f32; 4]| -> Vertex {
        let screen_pos = seam_view_world_to_screen(&scene.camera, &scene.viewport, pos);
        Vertex::new(screen_pos, color)
    };

    for segment in &scene.seam.segments {
        let color = seam_segment_color(segment.status);

        let endpoint1 = point_f32_to_f64(segment.endpoint1());
        let endpoint2 = point_f32_to_f64(segment.endpoint2());

        vertices.extend_from_slice(&[
            vertex(endpoint1 - thickness_offset, color),
            vertex(endpoint2 - thickness_offset, color),
            vertex(endpoint1 + thickness_offset, color),
        ]);
        vertices.extend_from_slice(&[
            vertex(endpoint2 - thickness_offset, color),
            vertex(endpoint1 + thickness_offset, color),
            vertex(endpoint2 + thickness_offset, color),
        ]);
    }

    vertices
}
