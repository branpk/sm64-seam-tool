use bytemuck::{cast, from_bytes};
use edge::{Edge, Orientation, ProjectedPoint, ProjectionAxis};
use float_range::{step_f32, step_f32_by};
use game_state::{GameState, Globals};
use geo::{
    direction_to_pitch_yaw, pitch_yaw_to_direction, point_f32_to_f64, point_f64_to_f32, Point3f,
    Vector3f, Vector4f,
};
use graphics::{
    seam_view_screen_to_world, BirdsEyeCamera, Camera, GameViewScene, ImguiRenderer, Renderer,
    RotateCamera, Scene, SeamInfo, SeamSegment, SeamViewCamera, SeamViewScene, SurfaceType,
    Viewport,
};
use imgui::{im_str, Condition, ConfigFlags, Context, DrawData, MouseButton, Ui};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use nalgebra::{Point3, Vector3};
use process::Process;
use read_process_memory::{copy_address, TryIntoProcessHandle};
use seam::Seam;
use seam_processor::{SeamProcessor, SeamProgress};
use std::{
    collections::HashSet,
    convert::TryInto,
    f32::consts::PI,
    iter,
    time::{Duration, Instant},
};
use util::{
    build_game_view_scene, find_hovered_seam, get_mouse_ray, get_norm_mouse_pos, get_segment_info,
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
mod process;
mod seam;
mod seam_processor;
mod spatial_partition;
mod util;

struct SeamViewState {
    seam: Seam,
    camera_pos: Point3<f64>,
    mouse_drag_start_pos: Option<Point3<f64>>,
}

impl SeamViewState {
    fn new(seam: Seam) -> Self {
        let camera_pos = seam.endpoint1() + (seam.endpoint2() - seam.endpoint1()) / 2.0;
        Self {
            seam,
            camera_pos: point_f32_to_f64(camera_pos),
            mouse_drag_start_pos: None,
        }
    }
}

struct App {
    process: Process,
    globals: Globals,
    sync_to_game: bool,
    seam_processor: SeamProcessor,
    hovered_seam: Option<Seam>,
    seam_view: Option<SeamViewState>,
    fps_string: String,
}

impl App {
    fn new() -> Self {
        App {
            // FIXME: Set denorm setting (or handle manually)
            process: Process::attach(54564, 0x008EBA80),
            globals: Globals::US,
            sync_to_game: false,
            seam_processor: SeamProcessor::new(),
            hovered_seam: None,
            seam_view: None,
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
        if self.sync_to_game {
            self.sync_to_game();
        }

        let state = GameState::read(&self.globals, &self.process);
        self.seam_processor.update(&state);

        let mut scenes = Vec::new();

        imgui::ChildWindow::new("game-view")
            .size([
                0.0,
                if self.seam_view.is_some() {
                    ui.window_size()[1] / 2.0
                } else {
                    0.0
                },
            ])
            .build(ui, || {
                scenes.push(Scene::GameView(self.render_game_view(ui, &state)));
            });

        if self.seam_view.is_some() {
            imgui::ChildWindow::new("seam-info").build(ui, || {
                scenes.push(Scene::SeamView(self.render_seam_view(ui)));
            });
        }

        scenes
    }

    fn render_game_view(&mut self, ui: &Ui, state: &GameState) -> GameViewScene {
        let viewport = Viewport {
            x: ui.window_pos()[0],
            y: ui.window_pos()[1],
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
            if ui.is_mouse_clicked(MouseButton::Left) && !ui.is_any_item_hovered() {
                self.seam_view = Some(SeamViewState::new(hovered_seam.clone()));
            }
        }

        ui.text(im_str!("{}", self.fps_string));
        ui.text(im_str!(
            "remaining: {}",
            self.seam_processor.remaining_seams()
        ));

        ui.checkbox(im_str!("sync"), &mut self.sync_to_game);

        scene
    }

    fn render_seam_view(&mut self, ui: &Ui) -> SeamViewScene {
        let seam_view = self.seam_view.as_mut().unwrap();
        let seam = seam_view.seam.clone();

        let viewport = Viewport {
            x: ui.window_pos()[0],
            y: ui.window_pos()[1],
            width: ui.window_size()[0],
            height: ui.window_size()[1],
        };

        let w_axis = match seam.edge1.projection_axis {
            ProjectionAxis::X => Vector3::z(),
            ProjectionAxis::Z => Vector3::x(),
        };
        let screen_right = match seam.edge1.orientation {
            Orientation::Positive => -w_axis,
            Orientation::Negative => w_axis,
        };

        let w_range = seam.edge1.w_range();
        let y_range = seam.edge1.y_range();
        let span_y = (y_range.end - y_range.start + 50.0)
            .max((w_range.end - w_range.start + 50.0) * viewport.height / viewport.width);

        let mut camera = SeamViewCamera {
            pos: seam_view.camera_pos,
            span_y: span_y as f64,
            right_dir: screen_right,
        };

        let screen_mouse_pos =
            get_norm_mouse_pos(ui.io().mouse_pos, ui.window_pos(), ui.window_size());
        let screen_mouse_pos = Point3f::new(screen_mouse_pos.0, screen_mouse_pos.1, 0.0);
        let mut world_mouse_pos = seam_view_screen_to_world(&camera, &viewport, screen_mouse_pos);

        if ui.is_mouse_clicked(MouseButton::Left)
            && !ui.is_any_item_hovered()
            && screen_mouse_pos.x.abs() <= 1.0
            && screen_mouse_pos.y.abs() <= 1.0
        {
            seam_view.mouse_drag_start_pos = Some(world_mouse_pos);
        }
        if ui.is_mouse_down(MouseButton::Left) {
            if let Some(mouse_drag_start_pos) = seam_view.mouse_drag_start_pos {
                seam_view.camera_pos += mouse_drag_start_pos - world_mouse_pos;
                camera.pos = seam_view.camera_pos;
                world_mouse_pos = seam_view_screen_to_world(&camera, &viewport, screen_mouse_pos);
            }
        } else {
            seam_view.mouse_drag_start_pos = None;
        }

        let progress = self.seam_processor.seam_progress(&seam);
        let scene = SeamViewScene {
            viewport,
            camera,
            seam: get_segment_info(&seam, &progress),
        };

        let close_seam_view = ui.button(im_str!("Close"), [0.0, 0.0]);

        let rounded_mouse = point_f64_to_f32(world_mouse_pos);
        match seam.edge1.projection_axis {
            ProjectionAxis::X => {
                ui.text(im_str!("(_, {}, {})", rounded_mouse.y, rounded_mouse.z));
            }
            ProjectionAxis::Z => {
                ui.text(im_str!("({}, {}, _)", rounded_mouse.x, rounded_mouse.y));
            }
        }

        if close_seam_view {
            self.seam_view = None;
        }
        scene
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
                            .scrollable(false)
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
