use bytemuck::{cast, from_bytes};
use edge::Edge;
use float_range::{step_f32, step_f32_by};
use game_state::{GameState, Globals};
use geo::{direction_to_pitch_yaw, pitch_yaw_to_direction, Point3f, Vector3f};
use imgui::{im_str, Condition, ConfigFlags, Context};
use imgui_renderer::ImguiRenderer;
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use process::Process;
use read_process_memory::{copy_address, TryIntoProcessHandle};
use renderer::Renderer;
use scene::{Camera, RotateCamera, Scene, SeamInfo, SeamSegment, SurfaceType, Viewport};
use seam::Seam;
use seam_processor::SeamProcessor;
use std::{
    collections::HashSet,
    convert::TryInto,
    f32::consts::PI,
    iter,
    time::{Duration, Instant},
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
mod imgui_renderer;
mod process;
mod renderer;
mod scene;
mod seam;
mod seam_processor;
mod spatial_partition;

fn build_scene(
    viewport: Viewport,
    game_state: &GameState,
    seam_processor: &SeamProcessor,
    hovered_seam: Option<Seam>,
) -> Scene {
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
        seams: seam_processor
            .active_seams()
            .iter()
            .map(|seam| {
                let progress = seam_processor.seam_progress(seam);

                let segments = progress
                    .segments()
                    .map(|(range, status)| SeamSegment {
                        endpoint1: seam.approx_point_at_w(range.start),
                        endpoint2: seam.approx_point_at_w(range.end),
                        status,
                    })
                    .collect();

                SeamInfo {
                    seam: seam.clone(),
                    segments,
                }
            })
            .collect(),
        hovered_seam,
    }
}

fn get_mouse_ray(
    mouse_pos: [f32; 2],
    window_pos: [f32; 2],
    window_size: [f32; 2],
    camera: &RotateCamera,
) -> Option<(Point3f, Vector3f)> {
    let rel_mouse_pos = (
        mouse_pos[0] - window_pos[0],
        window_size[1] - mouse_pos[1] + window_pos[1],
    );
    let norm_mouse_pos = (
        2.0 * rel_mouse_pos.0 / window_size[0] - 1.0,
        2.0 * rel_mouse_pos.1 / window_size[1] - 1.0,
    );
    if norm_mouse_pos.0.abs() > 1.0 || norm_mouse_pos.1.abs() > 1.0 {
        return None;
    }

    let forward_dir = (camera.target() - camera.pos()).normalize();
    let (pitch, yaw) = direction_to_pitch_yaw(&forward_dir);
    let up_dir = pitch_yaw_to_direction(pitch + PI / 2.0, yaw);
    let right_dir = pitch_yaw_to_direction(0.0, yaw - PI / 2.0);

    let top = (camera.fov_y / 2.0).tan();
    let right = top * window_size[0] / window_size[1];

    let mouse_dir =
        (forward_dir + top * norm_mouse_pos.1 * up_dir + right * norm_mouse_pos.0 * right_dir)
            .normalize();

    Some((camera.pos(), mouse_dir))
}

fn ray_surface_intersection(
    state: &GameState,
    ray: (Point3f, Vector3f),
) -> Option<(usize, Point3f)> {
    let mut nearest: Option<(f32, (usize, Point3f))> = None;

    for (i, surface) in state.surfaces.iter().enumerate() {
        let normal = surface.normal();
        let vertices = surface.vertices();

        let t = -normal.dot(&(ray.0 - vertices[0])) / normal.dot(&ray.1);
        if t <= 0.0 {
            continue;
        }

        let p = ray.0 + t * ray.1;

        let mut interior = true;
        for k in 0..3 {
            let edge = vertices[(k + 1) % 3] - vertices[k];
            if normal.dot(&edge.cross(&(p - vertices[k]))) < 0.0 {
                interior = false;
                break;
            }
        }
        if !interior {
            continue;
        }

        if nearest.is_none() || t < nearest.unwrap().0 {
            nearest = Some((t, (i, p)));
        }
    }

    nearest.map(|(_, result)| result)
}

fn find_hovered_seam(
    state: &GameState,
    active_seams: &[Seam],
    mouse_ray: (Point3f, Vector3f),
) -> Option<Seam> {
    let (surface_index, point) = ray_surface_intersection(state, mouse_ray)?;
    let surface = &state.surfaces[surface_index];
    let vertices = [surface.vertex1, surface.vertex2, surface.vertex3];

    let mut nearest_edge: Option<(f32, Edge)> = None;
    for k in 0..3 {
        let fvertex1 = surface.vertices()[k];
        let fvertex2 = surface.vertices()[(k + 1) % 3];
        let edge_dir = (fvertex2 - fvertex1).normalize();
        let inward_dir = surface.normal().cross(&edge_dir);
        let distance = inward_dir.dot(&(point - fvertex1)).abs();

        if nearest_edge.is_none() || distance < nearest_edge.unwrap().0 {
            let edge = Edge::new((vertices[k], vertices[(k + 1) % 3]), surface.normal);
            nearest_edge = Some((distance, edge));
        }
    }

    let (_, edge) = nearest_edge?;
    active_seams
        .iter()
        .filter(|seam| seam.edge1 == edge || seam.edge2 == edge)
        .cloned()
        .next()
}

fn main() {
    futures::executor::block_on(async {
        let process = Process::attach(85120, 0x008EBA80);

        let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);

        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title("seams legit 2.0")
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
        let mut seam_processor = SeamProcessor::new();
        let mut hovered_seam: Option<Seam> = None;

        let mut last_fps_time = Instant::now();
        let mut frames_since_fps = 0;
        let mut fps_string = String::new();

        let mut last_frame = Instant::now();
        event_loop.run(move |event, _, control_flow| {
            let elapsed = last_fps_time.elapsed();
            if elapsed > Duration::from_secs(1) {
                let fps = frames_since_fps as f64 / elapsed.as_secs_f64();
                let mspf = elapsed.as_millis() as f64 / frames_since_fps as f64;

                fps_string = format!("{:.2} mspf = {:.1} fps", mspf, fps);

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

                        let state = GameState::read(&Globals::US, &process);
                        seam_processor.update(&state);

                        let viewport = Viewport {
                            x: 0.0,
                            y: 0.0,
                            width: imgui.io().display_size[0],
                            height: imgui.io().display_size[1],
                        };
                        let scene =
                            build_scene(viewport, &state, &seam_processor, hovered_seam.clone());

                        let mouse_pos = imgui.io().mouse_pos;
                        let mut mouse_ray: Option<(Point3f, Vector3f)> = None;

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
                            .build(&ui, || {
                                if let Camera::Rotate(camera) = &scene.camera {
                                    mouse_ray = get_mouse_ray(
                                        mouse_pos,
                                        ui.window_pos(),
                                        ui.window_size(),
                                        camera,
                                    );
                                }

                                ui.text(im_str!("{}", fps_string));
                                ui.text(im_str!("remaining: {}", seam_processor.remaining_seams()));
                            });

                        platform.prepare_render(&ui, &window);
                        let draw_data = ui.render();

                        hovered_seam = mouse_ray.and_then(|mouse_ray| {
                            find_hovered_seam(&state, seam_processor.active_seams(), mouse_ray)
                        });

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

                        frames_since_fps += 1;
                    }
                }
                _ => {}
            }
        });
    })
}
