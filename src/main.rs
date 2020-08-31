use imgui::{im_str, Condition, ConfigFlags, Context};
use imgui_renderer::ImguiRenderer;
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use std::time::Instant;
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

mod imgui_renderer;

fn main() {
    futures::executor::block_on(async {
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
        imgui.io_mut().config_flags |= ConfigFlags::NO_MOUSE_CURSOR_CHANGE;

        let mut platform = WinitPlatform::init(&mut imgui);
        platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Default);

        let imgui_renderer =
            ImguiRenderer::new(&mut imgui, &device, &queue, swap_chain_desc.format);

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
                                ui.text(im_str!("Hello world!"));
                                ui.text(im_str!("こんにちは世界！"));
                                ui.text(im_str!("This...is...imgui-rs!"));
                                ui.separator();
                                let mouse_pos = ui.io().mouse_pos;
                                ui.text(format!(
                                    "Mouse Position: ({:.1},{:.1})",
                                    mouse_pos[0], mouse_pos[1]
                                ));
                            });

                        platform.prepare_render(&ui, &window);
                        let draw_data = ui.render();

                        let output_view = &swap_chain.get_current_frame().unwrap().output.view;
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
