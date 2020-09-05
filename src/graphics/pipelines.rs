use super::{Vertex, DEPTH_TEXTURE_FORMAT, NUM_OUTPUT_SAMPLES};
use bytemuck::offset_of;
use std::mem::size_of;

pub struct Pipelines {
    pub surface: wgpu::RenderPipeline,
    pub hidden_surface: wgpu::RenderPipeline,
    pub wall_hitbox: wgpu::RenderPipeline,
    pub wall_hitbox_depth_pass: wgpu::RenderPipeline,
    pub wall_hitbox_outline: wgpu::RenderPipeline,
    pub seam: wgpu::RenderPipeline,
    pub grid_line: wgpu::RenderPipeline,
}

impl Pipelines {
    pub fn create(
        device: &wgpu::Device,
        transform_bind_group_layout: &wgpu::BindGroupLayout,
        output_format: wgpu::TextureFormat,
    ) -> Self {
        let surface =
            create_surface_pipeline(device, &transform_bind_group_layout, output_format, true);

        let hidden_surface =
            create_surface_pipeline(device, &transform_bind_group_layout, output_format, false);

        let wall_hitbox = create_wall_hitbox_pipeline(
            device,
            &transform_bind_group_layout,
            output_format,
            true,
            wgpu::PrimitiveTopology::TriangleList,
        );

        let wall_hitbox_depth_pass = create_wall_hitbox_pipeline(
            device,
            &transform_bind_group_layout,
            output_format,
            false,
            wgpu::PrimitiveTopology::TriangleList,
        );

        let wall_hitbox_outline = create_wall_hitbox_pipeline(
            device,
            &transform_bind_group_layout,
            output_format,
            true,
            wgpu::PrimitiveTopology::LineList,
        );

        let seam = create_color_pipeline(
            device,
            &transform_bind_group_layout,
            output_format,
            wgpu::PrimitiveTopology::TriangleList,
        );
        let grid_line = create_color_pipeline(
            device,
            &transform_bind_group_layout,
            output_format,
            wgpu::PrimitiveTopology::LineList,
        );

        Self {
            surface,
            hidden_surface,
            wall_hitbox,
            wall_hitbox_depth_pass,
            wall_hitbox_outline,
            seam,
            grid_line,
        }
    }
}

fn create_surface_pipeline(
    device: &wgpu::Device,
    transform_bind_group_layout: &wgpu::BindGroupLayout,
    output_format: wgpu::TextureFormat,
    depth_write_enabled: bool,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(
            &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&transform_bind_group_layout],
                push_constant_ranges: &[],
            }),
        ),
        vertex_stage: wgpu::ProgrammableStageDescriptor {
            module: &device
                .create_shader_module(wgpu::include_spirv!("../../bin/shaders/surface.vert.spv")),
            entry_point: "main",
        },
        fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
            module: &device
                .create_shader_module(wgpu::include_spirv!("../../bin/shaders/surface.frag.spv")),
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
        depth_stencil_state: Some(wgpu::DepthStencilStateDescriptor {
            format: DEPTH_TEXTURE_FORMAT,
            depth_write_enabled,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilStateDescriptor::default(),
        }),
        vertex_state: wgpu::VertexStateDescriptor {
            index_format: wgpu::IndexFormat::Uint16,
            vertex_buffers: &[wgpu::VertexBufferDescriptor {
                stride: size_of::<Vertex>() as wgpu::BufferAddress,
                step_mode: wgpu::InputStepMode::Vertex,
                attributes: &[
                    // a_Pos
                    wgpu::VertexAttributeDescriptor {
                        offset: offset_of!(Vertex, pos) as wgpu::BufferAddress,
                        format: wgpu::VertexFormat::Float3,
                        shader_location: 0,
                    },
                    // a_Color
                    wgpu::VertexAttributeDescriptor {
                        offset: offset_of!(Vertex, color) as wgpu::BufferAddress,
                        format: wgpu::VertexFormat::Float4,
                        shader_location: 1,
                    },
                ],
            }],
        },
        sample_count: NUM_OUTPUT_SAMPLES,
        sample_mask: !0,
        alpha_to_coverage_enabled: false,
    })
}

fn create_wall_hitbox_pipeline(
    device: &wgpu::Device,
    transform_bind_group_layout: &wgpu::BindGroupLayout,
    output_format: wgpu::TextureFormat,
    color_write_enabled: bool,
    primitive_topology: wgpu::PrimitiveTopology,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(
            &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&transform_bind_group_layout],
                push_constant_ranges: &[],
            }),
        ),
        vertex_stage: wgpu::ProgrammableStageDescriptor {
            module: &device
                .create_shader_module(wgpu::include_spirv!("../../bin/shaders/color.vert.spv")),
            entry_point: "main",
        },
        fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
            module: &device
                .create_shader_module(wgpu::include_spirv!("../../bin/shaders/color.frag.spv")),
            entry_point: "main",
        }),
        rasterization_state: None,
        primitive_topology,
        color_states: &[wgpu::ColorStateDescriptor {
            format: output_format,
            alpha_blend: wgpu::BlendDescriptor::REPLACE,
            color_blend: wgpu::BlendDescriptor {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            write_mask: if color_write_enabled {
                wgpu::ColorWrite::ALL
            } else {
                wgpu::ColorWrite::empty()
            },
        }],
        depth_stencil_state: Some(wgpu::DepthStencilStateDescriptor {
            format: DEPTH_TEXTURE_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilStateDescriptor::default(),
        }),
        vertex_state: wgpu::VertexStateDescriptor {
            index_format: wgpu::IndexFormat::Uint16,
            vertex_buffers: &[wgpu::VertexBufferDescriptor {
                stride: size_of::<Vertex>() as wgpu::BufferAddress,
                step_mode: wgpu::InputStepMode::Vertex,
                attributes: &[
                    // a_Pos
                    wgpu::VertexAttributeDescriptor {
                        offset: offset_of!(Vertex, pos) as wgpu::BufferAddress,
                        format: wgpu::VertexFormat::Float3,
                        shader_location: 0,
                    },
                    // a_Color
                    wgpu::VertexAttributeDescriptor {
                        offset: offset_of!(Vertex, color) as wgpu::BufferAddress,
                        format: wgpu::VertexFormat::Float4,
                        shader_location: 1,
                    },
                ],
            }],
        },
        sample_count: NUM_OUTPUT_SAMPLES,
        sample_mask: !0,
        alpha_to_coverage_enabled: false,
    })
}

fn create_color_pipeline(
    device: &wgpu::Device,
    transform_bind_group_layout: &wgpu::BindGroupLayout,
    output_format: wgpu::TextureFormat,
    primitive_topology: wgpu::PrimitiveTopology,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(
            &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&transform_bind_group_layout],
                push_constant_ranges: &[],
            }),
        ),
        vertex_stage: wgpu::ProgrammableStageDescriptor {
            module: &device
                .create_shader_module(wgpu::include_spirv!("../../bin/shaders/color.vert.spv")),
            entry_point: "main",
        },
        fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
            module: &device
                .create_shader_module(wgpu::include_spirv!("../../bin/shaders/color.frag.spv")),
            entry_point: "main",
        }),
        rasterization_state: None,
        primitive_topology,
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
        depth_stencil_state: Some(wgpu::DepthStencilStateDescriptor {
            format: DEPTH_TEXTURE_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilStateDescriptor::default(),
        }),
        vertex_state: wgpu::VertexStateDescriptor {
            index_format: wgpu::IndexFormat::Uint16,
            vertex_buffers: &[wgpu::VertexBufferDescriptor {
                stride: size_of::<Vertex>() as wgpu::BufferAddress,
                step_mode: wgpu::InputStepMode::Vertex,
                attributes: &[
                    // a_Pos
                    wgpu::VertexAttributeDescriptor {
                        offset: offset_of!(Vertex, pos) as wgpu::BufferAddress,
                        format: wgpu::VertexFormat::Float3,
                        shader_location: 0,
                    },
                    // a_Color
                    wgpu::VertexAttributeDescriptor {
                        offset: offset_of!(Vertex, color) as wgpu::BufferAddress,
                        format: wgpu::VertexFormat::Float4,
                        shader_location: 1,
                    },
                ],
            }],
        },
        sample_count: NUM_OUTPUT_SAMPLES,
        sample_mask: !0,
        alpha_to_coverage_enabled: false,
    })
}
