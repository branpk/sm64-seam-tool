use super::{
    pipelines::Pipelines,
    util::{birds_eye_transforms, rotate_transforms, seam_segment_color},
    Camera, GameViewScene, SurfaceType, Vertex,
};
use crate::{
    geo::{Point3f, Vector3f},
    seam::RangeStatus,
};
use bytemuck::cast_slice;
use nalgebra::distance;
use std::f32::consts::PI;
use wgpu::util::DeviceExt;

pub struct GameViewSceneBundle<'a> {
    scene: &'a GameViewScene,
    transform_bind_group: wgpu::BindGroup,
    surface_vertex_buffer: (usize, wgpu::Buffer),
    // hidden_surface_vertex_buffer: (usize, wgpu::Buffer),
    // wall_hitbox_vertex_buffer: (usize, wgpu::Buffer),
    // wall_hitbox_outline_vertex_buffer: (usize, wgpu::Buffer),
    seam_vertex_buffer: (usize, wgpu::Buffer),
}

impl<'a> GameViewSceneBundle<'a> {
    pub fn build(
        scene: &'a GameViewScene,
        device: &wgpu::Device,
        transform_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let (proj_matrix, view_matrix) = match &scene.camera {
            Camera::Rotate(camera) => rotate_transforms(camera, &scene.viewport),
            Camera::BirdsEye(camera) => birds_eye_transforms(camera, &scene.viewport),
        };

        let proj_matrix_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: cast_slice(proj_matrix.as_slice()),
            usage: wgpu::BufferUsage::UNIFORM,
        });
        let view_matrix_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: cast_slice(view_matrix.as_slice()),
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

        let (surface_vertices, hidden_surface_vertices) = get_surface_vertices(scene);
        let surface_vertex_buffer = (
            surface_vertices.len(),
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: cast_slice(&surface_vertices),
                usage: wgpu::BufferUsage::VERTEX,
            }),
        );
        // let hidden_surface_vertex_buffer = (
        //     hidden_surface_vertices.len(),
        //     device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        //         label: None,
        //         contents: cast_slice(&hidden_surface_vertices),
        //         usage: wgpu::BufferUsage::VERTEX,
        //     }),
        // );

        // let (wall_hitbox_vertices, wall_hitbox_outline_vertices) =
        //     get_wall_hitbox_vertices(scene);
        // let wall_hitbox_vertex_buffer = (
        //     wall_hitbox_vertices.len(),
        //     device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        //         label: None,
        //         contents: cast_slice(&wall_hitbox_vertices),
        //         usage: wgpu::BufferUsage::VERTEX,
        //     }),
        // );
        // let wall_hitbox_outline_vertex_buffer = (
        //     wall_hitbox_outline_vertices.len(),
        //     device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        //         label: None,
        //         contents: cast_slice(&wall_hitbox_outline_vertices),
        //         usage: wgpu::BufferUsage::VERTEX,
        //     }),
        // );

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
            surface_vertex_buffer,
            // hidden_surface_vertex_buffer,
            // wall_hitbox_vertex_buffer,
            // wall_hitbox_outline_vertex_buffer,
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

        render_pass.set_pipeline(&pipelines.surface);
        render_pass.set_vertex_buffer(0, self.surface_vertex_buffer.1.slice(..));
        render_pass.draw(0..self.surface_vertex_buffer.0 as u32, 0..1);

        render_pass.set_pipeline(&pipelines.seam);
        render_pass.set_vertex_buffer(0, self.seam_vertex_buffer.1.slice(..));
        render_pass.draw(0..self.seam_vertex_buffer.0 as u32, 0..1);

        // if scene.wall_hitbox_radius > 0.0 {
        //     // Render lines first since tris write to z buffer
        //     render_pass.set_pipeline(&self.wall_hitbox_outline_pipeline);
        //     render_pass
        //         .set_vertex_buffer(0, bundle.wall_hitbox_outline_vertex_buffer.1.slice(..));
        //     render_pass.draw(0..bundle.wall_hitbox_outline_vertex_buffer.0 as u32, 0..1);

        //     // When two wall hitboxes overlap, we should not increase the opacity within
        //     // their region of overlap (preference).
        //     // First pass writes only to depth buffer to ensure that only the closest
        //     // hitbox triangles are drawn, then second pass draws them.
        //     render_pass.set_vertex_buffer(0, bundle.wall_hitbox_vertex_buffer.1.slice(..));
        //     render_pass.set_pipeline(&self.wall_hitbox_depth_pass_pipeline);
        //     render_pass.draw(0..bundle.wall_hitbox_vertex_buffer.0 as u32, 0..1);
        //     render_pass.set_pipeline(&self.wall_hitbox_pipeline);
        //     render_pass.draw(0..bundle.wall_hitbox_vertex_buffer.0 as u32, 0..1);
        // }

        // render_pass.set_pipeline(&self.hidden_surface_pipeline);
        // render_pass.set_vertex_buffer(0, bundle.hidden_surface_vertex_buffer.1.slice(..));
        // render_pass.draw(0..bundle.hidden_surface_vertex_buffer.0 as u32, 0..1);
    }
}

fn get_surface_vertices(scene: &GameViewScene) -> (Vec<Vertex>, Vec<Vertex>) {
    let mut surface_vertices: Vec<Vertex> = Vec::new();
    let mut hidden_surface_vertices: Vec<Vertex> = Vec::new();

    for (i, surface) in scene.surfaces.iter().enumerate() {
        let hidden = scene.hidden_surfaces.contains(&i);
        let hovered = scene.hovered_surface == Some(i);

        let mut color = match surface.ty {
            SurfaceType::Floor => [0.5, 0.5, 1.0, 1.0],
            SurfaceType::Ceiling => [1.0, 0.5, 0.5, 1.0],
            SurfaceType::WallXProj => [0.3, 0.8, 0.3, 1.0],
            SurfaceType::WallZProj => [0.15, 0.4, 0.15, 1.0],
        };

        if hidden {
            let scale = 1.5;
            color[0] *= scale;
            color[1] *= scale;
            color[2] *= scale;
            color[3] = if hovered { 0.1 } else { 0.0 };
        }

        if hovered {
            let boost = if surface.ty == SurfaceType::Floor {
                0.08
            } else {
                0.2
            };
            color[0] += boost;
            color[1] += boost;
            color[2] += boost;
        }

        for pos in &surface.vertices {
            let vertex = Vertex { pos: *pos, color };
            if hidden {
                hidden_surface_vertices.push(vertex);
            } else {
                surface_vertices.push(vertex);
            }
        }
    }

    (surface_vertices, hidden_surface_vertices)
}

fn get_wall_hitbox_vertices(scene: &GameViewScene) -> (Vec<Vertex>, Vec<Vertex>) {
    let mut wall_hitbox_vertices: Vec<Vertex> = Vec::new();
    let mut wall_hitbox_outline_vertices: Vec<Vertex> = Vec::new();

    for (i, surface) in scene.surfaces.iter().enumerate() {
        if scene.hidden_surfaces.contains(&i) {
            continue;
        }

        let proj_dir: Vector3f;
        let color: [f32; 4];
        match surface.ty {
            SurfaceType::Floor => continue,
            SurfaceType::Ceiling => continue,
            SurfaceType::WallXProj => {
                proj_dir = Vector3f::x();
                color = [0.3, 0.8, 0.3, 0.4];
            }
            SurfaceType::WallZProj => {
                proj_dir = Vector3f::z();
                color = [0.15, 0.4, 0.15, 0.4];
            }
        };
        let outline_color = [0.0, 0.0, 0.0, 0.5];

        let proj_dist = scene.wall_hitbox_radius / surface.normal().dot(&proj_dir);

        let wall_vertices = surface.vertices();
        let ext_vertices = [
            wall_vertices[0] + proj_dist * proj_dir,
            wall_vertices[1] + proj_dist * proj_dir,
            wall_vertices[2] + proj_dist * proj_dir,
        ];
        let int_vertices = [
            wall_vertices[0] - proj_dist * proj_dir,
            wall_vertices[1] - proj_dist * proj_dir,
            wall_vertices[2] - proj_dist * proj_dir,
        ];

        wall_hitbox_vertices.extend_from_slice(&[
            Vertex::new(ext_vertices[0], color),
            Vertex::new(ext_vertices[1], color),
            Vertex::new(ext_vertices[2], color),
        ]);
        wall_hitbox_vertices.extend_from_slice(&[
            Vertex::new(int_vertices[0], color),
            Vertex::new(int_vertices[1], color),
            Vertex::new(int_vertices[2], color),
        ]);

        wall_hitbox_outline_vertices.extend_from_slice(&[
            Vertex::new(ext_vertices[0], outline_color),
            Vertex::new(ext_vertices[1], outline_color),
            Vertex::new(ext_vertices[2], outline_color),
        ]);
        wall_hitbox_outline_vertices.extend_from_slice(&[
            Vertex::new(int_vertices[0], outline_color),
            Vertex::new(int_vertices[1], outline_color),
            Vertex::new(int_vertices[2], outline_color),
        ]);

        let camera_dist = match &scene.camera {
            Camera::Rotate(camera) => distance(&int_vertices[0], &Point3f::from_slice(&camera.pos)),
            Camera::BirdsEye(camera) => 1000.0,
        };

        for i0 in 0..3 {
            let i1 = (i0 + 1) % 3;

            // Bump slightly inward. This prevents flickering with floors and adjacent
            // walls
            let mut bump = 0.1 * camera_dist / 1000.0;
            if surface.ty == SurfaceType::WallZProj {
                bump *= 2.0; // Avoid flickering between x and z projected wall hitboxes
            }

            let vertices = [int_vertices[i0], int_vertices[i1], ext_vertices[i0]];
            let normal = (vertices[1] - vertices[0])
                .cross(&(vertices[2] - vertices[0]))
                .normalize();
            for vertex in &vertices {
                wall_hitbox_vertices.push(Vertex::new(vertex - bump * normal, color));
            }

            let vertices = [ext_vertices[i0], int_vertices[i1], ext_vertices[i1]];
            let normal = (vertices[1] - vertices[0])
                .cross(&(vertices[2] - vertices[0]))
                .normalize();
            for vertex in &vertices {
                wall_hitbox_vertices.push(Vertex::new(vertex - bump * normal, color));
            }

            wall_hitbox_outline_vertices.extend_from_slice(&[
                Vertex::new(int_vertices[i0], outline_color),
                Vertex::new(ext_vertices[i0], outline_color),
            ]);
            wall_hitbox_outline_vertices.extend_from_slice(&[
                Vertex::new(int_vertices[i0], outline_color),
                Vertex::new(int_vertices[i1], outline_color),
            ]);
            wall_hitbox_outline_vertices.extend_from_slice(&[
                Vertex::new(ext_vertices[i0], outline_color),
                Vertex::new(ext_vertices[i1], outline_color),
            ]);
        }
    }

    (wall_hitbox_vertices, wall_hitbox_outline_vertices)
}

fn get_seam_vertices(scene: &GameViewScene) -> Vec<Vertex> {
    let mut vertices = Vec::new();

    for seam in &scene.seams {
        for segment in &seam.segments {
            let endpoint1 = segment.endpoint1();
            let endpoint2 = segment.endpoint2();

            let seam_dir = (endpoint2 - endpoint1).normalize();
            let perp_dir_1 = Vector3f::y().cross(&seam_dir);
            let perp_dir_2 = seam_dir.cross(&perp_dir_1);

            let color = seam_segment_color(segment.status);

            let radius = if scene.hovered_seam.as_ref() == Some(&seam.seam) {
                10.0
            } else {
                5.0
            };
            let num_sides = 10;

            let mut push_vertex = |endpoint: Point3f, angle: f32| {
                let pos = endpoint + radius * (angle.cos() * perp_dir_1 + angle.sin() * perp_dir_2);
                vertices.push(Vertex {
                    pos: [pos.x, pos.y, pos.z],
                    color,
                });
            };

            for i in 0..num_sides {
                let a0 = (i as f32 / num_sides as f32) * 2.0 * PI;
                let a1 = ((i + 1) as f32 / num_sides as f32) * 2.0 * PI;

                push_vertex(endpoint1, a0);
                push_vertex(endpoint2, a0);
                push_vertex(endpoint1, a1);

                push_vertex(endpoint2, a0);
                push_vertex(endpoint1, a1);
                push_vertex(endpoint2, a1);
            }
        }
    }

    vertices
}
