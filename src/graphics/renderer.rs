use super::{
    game_view::GameViewSceneBundle, pipelines::Pipelines, seam_view::SeamViewSceneBundle, Scene,
    DEPTH_TEXTURE_FORMAT, NUM_OUTPUT_SAMPLES,
};
use std::iter;

pub struct Renderer {
    multisample_texture: Option<((u32, u32), wgpu::Texture)>,
    depth_texture: Option<((u32, u32), wgpu::Texture)>,
    transform_bind_group_layout: wgpu::BindGroupLayout,
    pipelines: Pipelines,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> Self {
        let transform_bind_group_layout =
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
                    // u_View
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
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

        let pipelines = Pipelines::create(device, &transform_bind_group_layout, output_format);

        Self {
            multisample_texture: None,
            depth_texture: None,
            transform_bind_group_layout,
            pipelines,
        }
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output_view: &wgpu::TextureView,
        output_size: (u32, u32),
        output_format: wgpu::TextureFormat,
        scenes: &[Scene],
    ) {
        if self
            .multisample_texture
            .as_ref()
            .filter(|(size, _)| size == &output_size)
            .is_none()
        {
            self.multisample_texture = Some((
                output_size,
                create_multisample_texture(device, output_format, output_size),
            ));
        }
        let multisample_texture_view = self
            .multisample_texture
            .as_ref()
            .unwrap()
            .1
            .create_view(&wgpu::TextureViewDescriptor::default());

        if self
            .depth_texture
            .as_ref()
            .filter(|(size, _)| size == &output_size)
            .is_none()
        {
            self.depth_texture = Some((output_size, create_depth_texture(device, output_size)));
        }
        let depth_texture_view = self
            .depth_texture
            .as_ref()
            .unwrap()
            .1
            .create_view(&wgpu::TextureViewDescriptor::default());

        let game_view_scene_bundles: Vec<GameViewSceneBundle<'_>> = scenes
            .iter()
            .filter_map(|scene| {
                if let Scene::GameView(scene) = scene {
                    Some(GameViewSceneBundle::build(
                        scene,
                        device,
                        &self.transform_bind_group_layout,
                    ))
                } else {
                    None
                }
            })
            .collect();

        let seam_view_scene_bundles: Vec<SeamViewSceneBundle<'_>> = scenes
            .iter()
            .filter_map(|scene| {
                if let Scene::SeamView(scene) = scene {
                    Some(SeamViewSceneBundle::build(
                        scene,
                        device,
                        &self.transform_bind_group_layout,
                    ))
                } else {
                    None
                }
            })
            .collect();

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &multisample_texture_view,
                    resolve_target: Some(&output_view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.06,
                            g: 0.06,
                            b: 0.06,
                            a: 1.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            for bundle in &game_view_scene_bundles {
                bundle.draw(&mut render_pass, &self.pipelines, output_size);
            }
            for bundle in &seam_view_scene_bundles {
                bundle.draw(&mut render_pass, &self.pipelines, output_size);
            }
        }

        let command_buffer = encoder.finish();
        queue.submit(iter::once(command_buffer));
    }
}

fn create_multisample_texture(
    device: &wgpu::Device,
    output_format: wgpu::TextureFormat,
    output_size: (u32, u32),
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: output_size.0,
            height: output_size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: NUM_OUTPUT_SAMPLES,
        dimension: wgpu::TextureDimension::D2,
        format: output_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}

fn create_depth_texture(device: &wgpu::Device, output_size: (u32, u32)) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: output_size.0,
            height: output_size.1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: NUM_OUTPUT_SAMPLES,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_TEXTURE_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}
