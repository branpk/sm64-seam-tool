use bytemuck::{cast_slice, Pod, Zeroable};
use imgui::{Context, DrawCmd, DrawData, DrawVert};
use std::{convert::TryInto, iter, mem::size_of};
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
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            min_binding_size: None,
                            has_dynamic_offset: false,
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
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // u_Texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
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
            vertex: wgpu::VertexState {
                module: &device
                    .create_shader_module(wgpu::include_spirv!("../../bin/shaders/imgui.vert.spv")),
                entry_point: "main",
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: size_of::<DrawVert>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        // a_Pos
                        wgpu::VertexAttribute {
                            offset: 0,
                            format: wgpu::VertexFormat::Float32x2,
                            shader_location: 0,
                        },
                        // a_TexCoord
                        wgpu::VertexAttribute {
                            offset: 8,
                            format: wgpu::VertexFormat::Float32x2,
                            shader_location: 1,
                        },
                        // a_Color
                        wgpu::VertexAttribute {
                            offset: 16,
                            format: wgpu::VertexFormat::Unorm8x4,
                            shader_location: 2,
                        },
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &device
                    .create_shader_module(wgpu::include_spirv!("../../bin/shaders/imgui.frag.spv")),
                entry_point: "main",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
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
            anisotropy_clamp: 1,
            border_color: None,
        });

        let fonts = imgui.fonts();
        let font_texture = fonts.build_rgba32_texture();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: font_texture.width,
                height: font_texture.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            font_texture.data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some((4 * font_texture.width).try_into().unwrap()),
                rows_per_image: Some(font_texture.height.try_into().unwrap()),
            },
            wgpu::Extent3d {
                width: font_texture.width,
                height: font_texture.height,
                depth_or_array_layers: 1,
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
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let proj_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.proj_bind_group_layout,
            entries: &[
                // u_Proj
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &proj_matrix_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });

        let buffers: Vec<(wgpu::Buffer, wgpu::Buffer)> = draw_data
            .draw_lists()
            .map(|command_list| {
                let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: cast_slice(command_list.idx_buffer()),
                    usage: wgpu::BufferUsages::INDEX,
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
                    usage: wgpu::BufferUsages::VERTEX,
                });
                (index_buffer, vertex_buffer)
            })
            .collect();

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &proj_bind_group, &[]);
            render_pass.set_bind_group(1, &self.font_texture_bind_group, &[]);

            for (command_list, (index_buffer, vertex_buffer)) in
                draw_data.draw_lists().zip(buffers.iter())
            {
                render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));

                for command in command_list.commands() {
                    if let DrawCmd::Elements { count, cmd_params } = command {
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
                }
            }
        }

        let command_buffer = encoder.finish();
        queue.submit(iter::once(command_buffer));
    }
}
