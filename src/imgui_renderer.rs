use bytemuck::{cast_slice, Pod, Zeroable};
use imgui::{Context, DrawCmd, DrawData, DrawVert};
use std::{iter, mem::size_of};
use wgpu::util::DeviceExt;

#[derive(Debug, Clone, Copy)]
struct DrawVertPod(DrawVert);

unsafe impl Zeroable for DrawVertPod {}
unsafe impl Pod for DrawVertPod {}

#[derive(Debug)]
pub struct ImguiRenderer {
    pipeline: wgpu::RenderPipeline,
    proj_bind_group_layout: wgpu::BindGroupLayout,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    font_texture_bind_group: wgpu::BindGroup,
}

impl ImguiRenderer {
    pub fn new(
        imgui: &mut Context,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output_format: wgpu::TextureFormat,
    ) -> Self {
        let proj_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    // u_Proj
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::VERTEX,
                        ty: wgpu::BindingType::UniformBuffer {
                            dynamic: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    // u_Sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Sampler { comparison: false },
                        count: None,
                    },
                    // u_Texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::SampledTexture {
                            dimension: wgpu::TextureViewDimension::D2,
                            component_type: wgpu::TextureComponentType::Float,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[&proj_bind_group_layout, &texture_bind_group_layout],
                    push_constant_ranges: &[],
                }),
            ),
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                module: &device
                    .create_shader_module(wgpu::include_spirv!("../bin/shaders/imgui.vert.spv")),
                entry_point: "main",
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                module: &device
                    .create_shader_module(wgpu::include_spirv!("../bin/shaders/imgui.frag.spv")),
                entry_point: "main",
            }),
            rasterization_state: None,
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[wgpu::ColorStateDescriptor {
                format: output_format,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                color_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: None,
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint16,
                vertex_buffers: &[wgpu::VertexBufferDescriptor {
                    stride: size_of::<DrawVert>() as wgpu::BufferAddress,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &[
                        // a_Pos
                        wgpu::VertexAttributeDescriptor {
                            offset: 0,
                            format: wgpu::VertexFormat::Float2,
                            shader_location: 0,
                        },
                        // a_TexCoord
                        wgpu::VertexAttributeDescriptor {
                            offset: 8,
                            format: wgpu::VertexFormat::Float2,
                            shader_location: 1,
                        },
                        // a_Color
                        wgpu::VertexAttributeDescriptor {
                            offset: 16,
                            format: wgpu::VertexFormat::Uchar4Norm,
                            shader_location: 2,
                        },
                    ],
                }],
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: f32::MAX,
            compare: None,
            anisotropy_clamp: None,
            border_color: None,
        });

        let mut fonts = imgui.fonts();
        let font_texture = fonts.build_rgba32_texture();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: font_texture.width,
                height: font_texture.height,
                depth: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsage::COPY_DST | wgpu::TextureUsage::SAMPLED,
        });
        queue.write_texture(
            wgpu::TextureCopyView {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            font_texture.data,
            wgpu::TextureDataLayout {
                offset: 0,
                bytes_per_row: 4 * font_texture.width,
                rows_per_image: font_texture.height,
            },
            wgpu::Extent3d {
                width: font_texture.width,
                height: font_texture.height,
                depth: 1,
            },
        );

        let font_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &texture_bind_group_layout,
            entries: &[
                // u_Sampler
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                // u_Texture
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
            ],
        });

        Self {
            pipeline,
            proj_bind_group_layout,
            texture_bind_group_layout,
            font_texture_bind_group,
        }
    }

    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output_view: &wgpu::TextureView,
        output_size: (u32, u32),
        draw_data: &DrawData,
    ) {
        let proj_matrix: [[f32; 4]; 4] = [
            [2.0 / output_size.0 as f32, 0.0, 0.0, 0.0],
            [0.0, -2.0 / output_size.1 as f32, 0.0, 0.0],
            [0.0, 0.0, -1.0, 0.0],
            [-1.0, 1.0, 0.0, 1.0],
        ];
        let proj_matrix_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: cast_slice(&proj_matrix),
            usage: wgpu::BufferUsage::UNIFORM,
        });
        let proj_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.proj_bind_group_layout,
            entries: &[
                // u_Proj
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer {
                        buffer: &proj_matrix_buffer,
                        offset: 0,
                        size: None,
                    },
                },
            ],
        });

        let buffers: Vec<(wgpu::Buffer, wgpu::Buffer)> = draw_data
            .draw_lists()
            .map(|command_list| {
                let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: cast_slice(command_list.idx_buffer()),
                    usage: wgpu::BufferUsage::INDEX,
                });
                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: cast_slice(
                        &command_list
                            .vtx_buffer()
                            .iter()
                            .map(|vertex| DrawVertPod(*vertex))
                            .collect::<Vec<DrawVertPod>>(),
                    ),
                    usage: wgpu::BufferUsage::VERTEX,
                });
                (index_buffer, vertex_buffer)
            })
            .collect();

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                    attachment: output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &proj_bind_group, &[]);
            render_pass.set_bind_group(1, &self.font_texture_bind_group, &[]);

            for (command_list, (index_buffer, vertex_buffer)) in
                draw_data.draw_lists().zip(buffers.iter())
            {
                render_pass.set_index_buffer(index_buffer.slice(..));
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));

                for command in command_list.commands() {
                    match command {
                        DrawCmd::Elements { count, cmd_params } => {
                            let clip_rect = cmd_params.clip_rect;
                            render_pass.set_scissor_rect(
                                clip_rect[0] as u32,
                                clip_rect[1] as u32,
                                (clip_rect[2] - clip_rect[0]) as u32,
                                (clip_rect[3] - clip_rect[1]) as u32,
                            );

                            render_pass.draw_indexed(
                                cmd_params.idx_offset as u32
                                    ..(cmd_params.idx_offset + count) as u32,
                                0,
                                0..1,
                            );
                        }
                        _ => {}
                    }
                }
            }
        }

        let command_buffer = encoder.finish();
        queue.submit(iter::once(command_buffer));
    }
}
