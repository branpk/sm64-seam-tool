use bytemuck::{cast, from_bytes};
use game_state::{GameState, Globals};
use imgui::{im_str, Condition, ConfigFlags, Context};
use imgui_renderer::ImguiRenderer;
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use process::Process;
use read_process_memory::{copy_address, TryIntoProcessHandle};
use renderer::Renderer;
use scene::{Camera, RotateCamera, Scene, SurfaceType, Viewport};
use std::{collections::HashSet, convert::TryInto, time::Instant};
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

mod edge;
mod game_state;
mod geo;
mod imgui_renderer;
mod process;
mod renderer;
mod scene;

fn game_state_to_scene(viewport: Viewport, game_state: &GameState) -> Scene {
    Scene {
        viewport,
        camera: Camera::Rotate(RotateCamera {
            pos: game_state.lakitu_pos,
            target: game_state.lakitu_focus,
            fov_y: 45.0,
        }),
        surfaces: game_state
            .surfaces
            .iter()
            .map(|surface| {
                let ty = if surface.normal[1] > 0.01 {
                    SurfaceType::Floor
                } else if surface.normal[1] < -0.01 {
                    SurfaceType::Ceiling
                } else if surface.normal[0] < -0.707 || surface.normal[0] > 0.707 {
                    SurfaceType::WallXProj
                } else {
                    SurfaceType::WallZProj
                };

                let to_f32_3 =
                    |vertex: [i16; 3]| [vertex[0] as f32, vertex[1] as f32, vertex[2] as f32];

                scene::Surface {
                    ty,
                    vertices: [
                        to_f32_3(surface.vertex1),
                        to_f32_3(surface.vertex2),
                        to_f32_3(surface.vertex3),
                    ],
                    normal: surface.normal,
                }
            })
            .collect(),
        wall_hitbox_radius: 0.0,
        hovered_surface: None,
        hidden_surfaces: HashSet::new(),
    }
}

fn main() {
    futures::executor::block_on(async {
        let process = Process::attach(89228, 0x008EBA80);

        let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);

        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title("")
            .with_inner_size(PhysicalSize::new(800, 600))
            .build(&event_loop)
            .unwrap();

        let surface = unsafe { instance.create_surface(&window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::Default,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("no compatible device");
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                    shader_validation: true,
                },
                None,
            )
            .await
            .unwrap();

        let mut swap_chain_desc = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            width: window.inner_size().width,
            height: window.inner_size().height,
            present_mode: wgpu::PresentMode::Mailbox,
        };
        let mut swap_chain = device.create_swap_chain(&surface, &swap_chain_desc);

        let mut imgui = Context::create();
        imgui.set_ini_filename(None);
        imgui.style_mut().window_rounding = 0.0;
        imgui.style_mut().colors[imgui::StyleColor::WindowBg as usize] = [0.0, 0.0, 0.0, 0.0];
        imgui.io_mut().config_flags |= ConfigFlags::NO_MOUSE_CURSOR_CHANGE;

        let mut platform = WinitPlatform::init(&mut imgui);
        platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Default);

        let imgui_renderer =
            ImguiRenderer::new(&mut imgui, &device, &queue, swap_chain_desc.format);

        let mut renderer = Renderer::new(&device, swap_chain_desc.format);

        let mut last_frame = Instant::now();
        event_loop.run(move |event, _, control_flow| {
            platform.handle_event(imgui.io_mut(), &window, &event);
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(size) => {
                        swap_chain_desc.width = size.width;
                        swap_chain_desc.height = size.height;
                        swap_chain = device.create_swap_chain(&surface, &swap_chain_desc);
                    }
                    _ => {}
                },
                Event::MainEventsCleared => window.request_redraw(),
                Event::RedrawRequested(_) => {
                    if swap_chain_desc.width > 0 && swap_chain_desc.height > 0 {
                        last_frame = imgui.io_mut().update_delta_time(last_frame);

                        let state = GameState::read(&Globals::US, &process);

                        let viewport = Viewport {
                            x: 0.0,
                            y: 0.0,
                            width: imgui.io().display_size[0],
                            height: imgui.io().display_size[1],
                        };
                        let scene = game_state_to_scene(viewport, &state);

                        platform
                            .prepare_frame(imgui.io_mut(), &window)
                            .expect("Failed to prepare frame");

                        let ui = imgui.frame();

                        imgui::Window::new(im_str!("Hello world"))
                            .position([0.0, 0.0], Condition::Always)
                            .size(ui.io().display_size, Condition::Always)
                            .save_settings(false)
                            .resizable(false)
                            .title_bar(false)
                            .bring_to_front_on_focus(false)
                            .build(&ui, || {});

                        platform.prepare_render(&ui, &window);
                        let draw_data = ui.render();

                        let output_view = &swap_chain.get_current_frame().unwrap().output.view;

                        renderer.render(
                            &device,
                            &queue,
                            output_view,
                            (swap_chain_desc.width, swap_chain_desc.height),
                            swap_chain_desc.format,
                            &[scene],
                        );

                        imgui_renderer.render(
                            &device,
                            &queue,
                            output_view,
                            (swap_chain_desc.width, swap_chain_desc.height),
                            draw_data,
                        );
                    }
                }
                _ => {}
            }
        });
    })
}
