use bytemuck::{cast, from_bytes};
use edge::Edge;
use float_range::{step_f32, step_f32_by};
use game_state::{GameState, Globals};
use geo::{direction_to_pitch_yaw, pitch_yaw_to_direction, Point3f, Vector3f};
use imgui::{im_str, Condition, ConfigFlags, Context, DrawData, MouseButton, Ui};
use imgui_renderer::ImguiRenderer;
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use process::Process;
use read_process_memory::{copy_address, TryIntoProcessHandle};
use renderer::Renderer;
use scene::{
    Camera, GameViewScene, RotateCamera, Scene, SeamInfo, SeamSegment, SurfaceType, Viewport,
};
use seam::Seam;
use seam_processor::SeamProcessor;
use std::{
    collections::HashSet,
    convert::TryInto,
    f32::consts::PI,
    iter,
    time::{Duration, Instant},
};
use util::{find_hovered_seam, get_mouse_ray};
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
mod util;

struct App {
    process: Process,
    globals: Globals,
    seam_processor: SeamProcessor,
    hovered_seam: Option<Seam>,
    selected_seam: Option<Seam>,
    fps_string: String,
}

impl App {
    fn new() -> Self {
        App {
            // FIXME: Set denorm setting (or handle manually)
            process: Process::attach(94400, 0x008EBA80),
            globals: Globals::US,
            seam_processor: SeamProcessor::new(),
            hovered_seam: None,
            selected_seam: None,
            fps_string: String::new(),
        }
    }

    fn sync_to_game(&self) {
        let initial_global_timer: u32 = self.process.read(self.globals.global_timer);
        let start_time = Instant::now();
        while start_time.elapsed() < Duration::from_millis(50) {
            let global_timer: u32 = self.process.read(self.globals.global_timer);
            if global_timer != initial_global_timer {
                break;
            }
        }
    }

    fn render(&mut self, ui: &Ui) -> Vec<Scene> {
        self.sync_to_game();

        let state = GameState::read(&self.globals, &self.process);
        self.seam_processor.update(&state);

        let game_view_height = if self.selected_seam.is_some() {
            ui.window_size()[1] / 2.0
        } else {
            ui.window_size()[1]
        };

        let mut scenes = Vec::new();
        imgui::ChildWindow::new("game-view")
            .size([ui.window_size()[0], game_view_height])
            .build(ui, || {
                scenes.push(Scene::GameView(self.render_game_view(ui, &state)));
            });

        if let Some(seam) = self.selected_seam.clone() {
            imgui::ChildWindow::new("seam-info")
                .size([ui.window_size()[0], ui.window_size()[1] / 2.0])
                .build(ui, || {
                    ui.text(im_str!("Seam: {:?}", seam));

                    if ui.button(im_str!("Close"), [0.0, 0.0]) {
                        self.selected_seam = None;
                    }
                });
        }

        scenes
    }

    fn render_game_view(&mut self, ui: &Ui, state: &GameState) -> GameViewScene {
        let viewport = Viewport {
            x: 0.0,
            y: 0.0,
            width: ui.window_size()[0],
            height: ui.window_size()[1],
        };
        let scene = build_game_view_scene(
            viewport,
            &state,
            &self.seam_processor,
            self.hovered_seam.clone(),
        );
        if let Camera::Rotate(camera) = &scene.camera {
            let mouse_ray =
                get_mouse_ray(ui.io().mouse_pos, ui.window_pos(), ui.window_size(), camera);
            self.hovered_seam = mouse_ray.and_then(|mouse_ray| {
                find_hovered_seam(&state, self.seam_processor.active_seams(), mouse_ray)
            });
        }

        if let Some(hovered_seam) = &self.hovered_seam {
            if ui.is_mouse_clicked(MouseButton::Left) {
                self.selected_seam = Some(hovered_seam.clone());
            }
        }

        ui.text(im_str!("{}", self.fps_string));
        ui.text(im_str!(
            "remaining: {}",
            self.seam_processor.remaining_seams()
        ));

        scene
    }
}

fn build_game_view_scene(
    viewport: Viewport,
    game_state: &GameState,
    seam_processor: &SeamProcessor,
    hovered_seam: Option<Seam>,
) -> GameViewScene {
    GameViewScene {
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

fn main() {
    futures::executor::block_on(async {
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
        let mut app = App::new();

        let mut last_fps_time = Instant::now();
        let mut frames_since_fps = 0;

        let mut last_frame = Instant::now();
        event_loop.run(move |event, _, control_flow| {
            let elapsed = last_fps_time.elapsed();
            if elapsed > Duration::from_secs(1) {
                let fps = frames_since_fps as f64 / elapsed.as_secs_f64();
                let mspf = elapsed.as_millis() as f64 / frames_since_fps as f64;

                app.fps_string = format!("{:.2} mspf = {:.1} fps", mspf, fps);

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

                        let mut scenes = Vec::new();

                        let ui = imgui.frame();

                        imgui::Window::new(im_str!("app"))
                            .position([0.0, 0.0], Condition::Always)
                            .size(ui.io().display_size, Condition::Always)
                            .save_settings(false)
                            .resizable(false)
                            .title_bar(false)
                            .scroll_bar(false)
                            .bring_to_front_on_focus(false)
                            .build(&ui, || {
                                scenes = app.render(&ui);
                            });

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
