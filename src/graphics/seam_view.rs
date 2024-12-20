use super::{
    FocusedSeamData, SeamSegment, SeamViewScene, Vertex, pipelines::Pipelines, seam_point_color,
    seam_view_world_to_screen, upload_vertex_buffer, util::seam_segment_color,
};
use crate::{
    geo::{Matrix4f, Point3f, Vector3f, point_f32_to_f64},
    seam::PointStatus,
};
use bytemuck::cast_slice;
use nalgebra::{Point3, Vector3};
use wgpu::util::DeviceExt;

pub struct SeamViewSceneBundle<'a> {
    scene: &'a SeamViewScene,
    transform_bind_group: wgpu::BindGroup,
    seam_segment_vertex_buffer: (usize, wgpu::Buffer),
    seam_point_vertex_buffer: (usize, wgpu::Buffer),
    grid_line_vertex_buffer: (usize, wgpu::Buffer),
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
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let view_matrix_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: cast_slice(Matrix4f::identity().as_slice()),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let transform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: transform_bind_group_layout,
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

        let seam_segment_vertices = if let FocusedSeamData::Segments(segments) = &scene.seam.data {
            get_seam_segment_vertices(scene, segments)
        } else {
            Vec::new()
        };
        let seam_segment_vertex_buffer = upload_vertex_buffer(device, &seam_segment_vertices);

        let seam_point_vertices = if let FocusedSeamData::Points(points) = &scene.seam.data {
            get_seam_point_vertices(scene, points)
        } else {
            Vec::new()
        };
        let seam_point_vertex_buffer = upload_vertex_buffer(device, &seam_point_vertices);

        let grid_line_vertices = get_grid_line_vertices(scene);
        let grid_line_vertex_buffer = upload_vertex_buffer(device, &grid_line_vertices);

        Self {
            scene,
            transform_bind_group,
            seam_segment_vertex_buffer,
            seam_point_vertex_buffer,
            grid_line_vertex_buffer,
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

        render_pass.set_pipeline(&pipelines.grid_line);
        render_pass.set_vertex_buffer(0, self.grid_line_vertex_buffer.1.slice(..));
        render_pass.draw(0..self.grid_line_vertex_buffer.0 as u32, 0..1);

        render_pass.set_pipeline(&pipelines.seam);
        render_pass.set_vertex_buffer(0, self.seam_segment_vertex_buffer.1.slice(..));
        render_pass.draw(0..self.seam_segment_vertex_buffer.0 as u32, 0..1);

        render_pass.set_pipeline(&pipelines.seam);
        render_pass.set_vertex_buffer(0, self.seam_point_vertex_buffer.1.slice(..));
        render_pass.draw(0..self.seam_point_vertex_buffer.0 as u32, 0..1);
    }
}

fn get_seam_segment_vertices(scene: &SeamViewScene, segments: &[SeamSegment]) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    // let slope = scene.seam.seam.edge1.slope() as f64;
    // let thickness = 0.03 * (slope * slope + 1.0).sqrt();
    // let screen_thickness_offset = thickness * Vector3::y();
    let thickness_offset = 0.03 * Vector3::y() * scene.camera.span_y / 2.0; //screen_thickness_offset * scene.camera.span_y / 2.0;

    let vertex = |pos: Point3<f64>, color: [f32; 4]| -> Vertex {
        let screen_pos = seam_view_world_to_screen(&scene.camera, &scene.viewport, pos);
        Vertex::new(screen_pos, color)
    };

    for segment in segments {
        let color = seam_segment_color(segment.status);

        let endpoint1 = point_f32_to_f64(segment.endpoint1());
        let endpoint2 = point_f32_to_f64(segment.endpoint2());

        vertices.extend(&[
            vertex(endpoint1 - thickness_offset, color),
            vertex(endpoint2 - thickness_offset, color),
            vertex(endpoint1 + thickness_offset, color),
        ]);
        vertices.extend(&[
            vertex(endpoint2 - thickness_offset, color),
            vertex(endpoint1 + thickness_offset, color),
            vertex(endpoint2 + thickness_offset, color),
        ]);
    }

    vertices
}

fn get_seam_point_vertices(
    scene: &SeamViewScene,
    points: &[(Point3f, PointStatus)],
) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    let radius = 0.015;
    let y_offset = radius * Vector3f::y();
    let x_offset = radius * Vector3f::x() * scene.viewport.height / scene.viewport.width;

    for (world_pos, status) in points {
        let color = seam_point_color(*status);

        let screen_pos =
            seam_view_world_to_screen(&scene.camera, &scene.viewport, point_f32_to_f64(*world_pos));

        vertices.extend(&[
            Vertex::new(screen_pos - x_offset - y_offset, color),
            Vertex::new(screen_pos + x_offset - y_offset, color),
            Vertex::new(screen_pos - x_offset + y_offset, color),
        ]);
        vertices.extend(&[
            Vertex::new(screen_pos + x_offset - y_offset, color),
            Vertex::new(screen_pos - x_offset + y_offset, color),
            Vertex::new(screen_pos + x_offset + y_offset, color),
        ]);
    }

    vertices
}

fn get_grid_line_vertices(scene: &SeamViewScene) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    let color = [0.4, 0.4, 0.4, 1.0];

    for &world_pos in &scene.vertical_grid_lines {
        let screen_pos = seam_view_world_to_screen(&scene.camera, &scene.viewport, world_pos);
        vertices.extend(&[
            Vertex::new(Point3f::new(screen_pos.x, -1.0, screen_pos.z), color),
            Vertex::new(Point3f::new(screen_pos.x, 1.0, screen_pos.z), color),
        ])
    }

    for &world_pos in &scene.horizontal_grid_lines {
        let screen_pos = seam_view_world_to_screen(&scene.camera, &scene.viewport, world_pos);
        vertices.extend(&[
            Vertex::new(Point3f::new(-1.0, screen_pos.y, screen_pos.z), color),
            Vertex::new(Point3f::new(1.0, screen_pos.y, screen_pos.z), color),
        ])
    }

    vertices
}
