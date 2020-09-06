use bytemuck::{cast, from_bytes};
use edge::{Edge, Orientation, ProjectedPoint, ProjectionAxis};
use float_range::{flush_f32_to_zero, next_f32, prev_f32, RangeF32};
use game_state::{Config, GameState, Globals};
use geo::{
    direction_to_pitch_yaw, pitch_yaw_to_direction, point_f32_to_f64, point_f64_to_f32, Point3f,
    Vector3f, Vector4f,
};
use graphics::{
    seam_view_screen_to_world, BirdsEyeCamera, Camera, GameViewScene, ImguiRenderer, Renderer,
    RotateCamera, Scene, SeamInfo, SeamSegment, SeamViewCamera, SeamViewScene, SurfaceType,
    Viewport,
};
use imgui::{im_str, Condition, ConfigFlags, Context, DrawData, ImString, MouseButton, Ui};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use itertools::Itertools;
use lazy_static::lazy_static;
use model::App;
use nalgebra::{Point3, Vector3};
use process::Process;
use read_process_memory::{copy_address, TryIntoProcessHandle};
use seam::{PointFilter, Seam};
use seam_processor::{SeamProcessor, SeamProgress};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    convert::TryInto,
    f32::consts::PI,
    fs::{self, File},
    io::Read,
    iter,
    time::{Duration, Instant},
};
use sysinfo::{ProcessExt, System, SystemExt};
use ui::render_app;
use util::{
    build_game_view_scene, find_hovered_seam, get_focused_seam_info, get_mouse_ray,
    get_norm_mouse_pos, get_segment_info,
};
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

mod edge;
mod float_range;
mod game_state;
mod geo;
mod graphics;
mod model;
mod process;
mod seam;
mod seam_processor;
mod spatial_partition;
mod ui;
mod util;

fn main() {
    futures::executor::block_on(async {
        let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);

        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title("seams legit 2.0")
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
        let mut app = App::new();

        let mut last_fps_time = Instant::now();
        let mut frames_since_fps = 0;

        let mut last_frame = Instant::now();
        event_loop.run(move |event, _, control_flow| {
            let elapsed = last_fps_time.elapsed();
            if elapsed > Duration::from_secs(1) {
                let fps = frames_since_fps as f64 / elapsed.as_secs_f64();
                let mspf = elapsed.as_millis() as f64 / frames_since_fps as f64;

                if let App::Connected(model) = &mut app {
                    model.fps_string = format!("{:.2} mspf = {:.1} fps", mspf, fps);
                }

                last_fps_time = Instant::now();
                frames_since_fps = 0;
            }

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

                        platform
                            .prepare_frame(imgui.io_mut(), &window)
                            .expect("Failed to prepare frame");

                        let ui = imgui.frame();
                        let scenes = render_app(&ui, &mut app);
                        platform.prepare_render(&ui, &window);
                        let draw_data = ui.render();

                        let output_view = &swap_chain.get_current_frame().unwrap().output.view;

                        renderer.render(
                            &device,
                            &queue,
                            output_view,
                            (swap_chain_desc.width, swap_chain_desc.height),
                            swap_chain_desc.format,
                            &scenes,
                        );

                        imgui_renderer.render(
                            &device,
                            &queue,
                            output_view,
                            (swap_chain_desc.width, swap_chain_desc.height),
                            draw_data,
                        );

                        frames_since_fps += 1;
                    }
                }
                _ => {}
            }
        });
    })
}
