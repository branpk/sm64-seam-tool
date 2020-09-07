use crate::{
    edge::{Edge, Orientation, ProjectedPoint, ProjectionAxis},
    float_range::RangeF32,
    game_state::GameState,
    geo::{point_f64_to_f32, Point3f},
    graphics::{
        seam_view_screen_to_world, Camera, GameViewScene, Scene, SeamViewCamera, SeamViewScene,
        Viewport,
    },
    model::{App, ConnectedView, ConnectionMenu, SeamExportForm, SeamViewState},
    seam::PointFilter,
    util::{
        build_game_view_scene, canonicalize_process_name, find_hovered_seam, get_focused_seam_info,
        get_mouse_ray, get_norm_mouse_pos, sync_to_game,
    },
};
use imgui::{im_str, Condition, MouseButton, Ui};
use itertools::Itertools;
use nalgebra::{Point3, Vector3};
use sysinfo::{ProcessExt, SystemExt};

pub fn render_app(ui: &Ui, app: &mut App) -> Vec<Scene> {
    let style_token = ui.push_style_color(imgui::StyleColor::WindowBg, [0.0, 0.0, 0.0, 0.0]);

    let mut scenes = Vec::new();
    imgui::Window::new(im_str!("##app"))
        .position([0.0, 0.0], Condition::Always)
        .size(ui.io().display_size, Condition::Always)
        .save_settings(false)
        .resizable(false)
        .title_bar(false)
        .scroll_bar(false)
        .scrollable(false)
        .bring_to_front_on_focus(false)
        .build(&ui, || {
            scenes = match app {
                App::ConnectionMenu(menu) => {
                    if let Some(model) = render_connection_menu(ui, menu) {
                        *app = App::Connected(model);
                    }
                    Vec::new()
                }
                App::Connected(view) => render_connected_view(ui, view),
            }
        });

    style_token.pop(ui);
    scenes
}

fn render_connection_menu(ui: &Ui, menu: &mut ConnectionMenu) -> Option<ConnectedView> {
    menu.system.refresh_processes();
    let processes: Vec<_> = menu
        .system
        .get_processes()
        .values()
        .sorted_by_key(|process| process.name().to_lowercase())
        .collect();

    let mut process_index = menu
        .selected_pid
        .and_then(|selected_pid| {
            processes
                .iter()
                .position(|process| process.pid() == selected_pid)
        })
        .unwrap_or_else(|| {
            let known_process = processes.iter().position(|process| {
                let name = canonicalize_process_name(process.name());
                menu.config.base_addresses.contains_key(name.as_str())
            });
            known_process.unwrap_or(0)
        });

    ui.text("Connect to emulator");

    ui.spacing();
    ui.set_next_item_width(300.0);
    imgui::ComboBox::new(&im_str!("##process")).build_simple(
        ui,
        &mut process_index,
        &processes,
        &|process| im_str!("{:8}: {}", process.pid(), process.name()).into(),
    );
    let selected_process = processes.get(process_index).cloned();
    let selected_pid = selected_process.map(|process| process.pid());
    let changed_pid = selected_pid != menu.selected_pid;
    menu.selected_pid = selected_pid;

    ui.spacing();
    ui.text(im_str!("Base address: "));
    ui.same_line(110.0);
    ui.set_next_item_width(190.0);
    if ui
        .input_text(im_str!("##base-addr"), &mut menu.base_addr_buffer)
        .build()
    {
        menu.selected_base_addr = parse_int::parse(menu.base_addr_buffer.to_str()).ok();
    }
    if changed_pid {
        if let Some(selected_process) = selected_process {
            if let Some(base_addr) = menu
                .config
                .base_addresses
                .get(canonicalize_process_name(selected_process.name()).as_str())
            {
                menu.selected_base_addr = Some(*base_addr);
                menu.base_addr_buffer = im_str!("{:#X}", *base_addr);
                menu.base_addr_buffer.reserve(32);
            }
        }
    }

    ui.spacing();
    ui.text(im_str!("Game version: "));
    ui.same_line(110.0);
    ui.set_next_item_width(100.0);
    imgui::ComboBox::new(im_str!("")).build_simple(
        ui,
        &mut menu.selected_version_index,
        &menu.config.game_versions,
        &|game_version| im_str!("{}", game_version.name).into(),
    );

    ui.spacing();
    if let Some(pid) = menu.selected_pid {
        if let Some(base_addr) = menu.selected_base_addr {
            if ui.button(im_str!("Connect"), [0.0, 0.0]) {
                return Some(ConnectedView::new(
                    pid as u32,
                    base_addr,
                    menu.config.game_versions[menu.selected_version_index]
                        .globals
                        .clone(),
                ));
            }
        }
    }

    None
}

fn render_connected_view(ui: &Ui, view: &mut ConnectedView) -> Vec<Scene> {
    if view.sync_to_game {
        sync_to_game(&view.process, &view.globals);
    }

    let state = GameState::read(&view.globals, &view.process);
    view.seam_processor.update(&state);

    let mut scenes = Vec::new();

    imgui::ChildWindow::new("game-view")
        .size([
            0.0,
            if view.seam_view.is_some() {
                ui.window_size()[1] / 2.0
            } else {
                0.0
            },
        ])
        .build(ui, || {
            scenes.push(Scene::GameView(render_game_view(ui, view, &state)));
        });

    if view.seam_view.is_some() {
        imgui::ChildWindow::new("seam-info").build(ui, || {
            scenes.push(Scene::SeamView(render_seam_view(ui, view)));
        });
    }

    if let Some(form) = &mut view.export_form {
        if !render_export_form(ui, form) {
            view.export_form = None;
        }
    }

    scenes
}

fn render_game_view(ui: &Ui, view: &mut ConnectedView, state: &GameState) -> GameViewScene {
    let viewport = Viewport {
        x: ui.window_pos()[0],
        y: ui.window_pos()[1],
        width: ui.window_size()[0],
        height: ui.window_size()[1],
    };
    let scene = build_game_view_scene(
        viewport,
        &state,
        &view.seam_processor,
        view.hovered_seam.clone(),
    );
    if let Camera::Rotate(camera) = &scene.camera {
        let mouse_ray = get_mouse_ray(ui.io().mouse_pos, ui.window_pos(), ui.window_size(), camera);
        view.hovered_seam = mouse_ray.and_then(|mouse_ray| {
            find_hovered_seam(&state, view.seam_processor.active_seams(), mouse_ray)
        });
    }

    if let Some(hovered_seam) = &view.hovered_seam {
        if ui.is_mouse_clicked(MouseButton::Left)
            && !ui.is_any_item_hovered()
            && view.export_form.is_none()
        {
            view.seam_view = Some(SeamViewState::new(hovered_seam.clone()));
        }
    }

    ui.text(im_str!("{}", view.fps_string));
    ui.text(im_str!(
        "remaining: {}",
        view.seam_processor.remaining_seams()
    ));

    ui.checkbox(im_str!("sync"), &mut view.sync_to_game);

    let all_filters = PointFilter::all();
    let mut filter_index = all_filters
        .iter()
        .position(|filter| view.seam_processor.filter() == *filter)
        .unwrap();
    ui.set_next_item_width(100.0);
    if imgui::ComboBox::new(im_str!("##filter")).build_simple(
        ui,
        &mut filter_index,
        &all_filters,
        &|filter| im_str!("{}", filter).into(),
    ) {
        view.seam_processor.set_filter(all_filters[filter_index]);
    }

    scene
}

fn render_seam_view(ui: &Ui, view: &mut ConnectedView) -> SeamViewScene {
    let seam_view = view.seam_view.as_mut().unwrap();
    let seam = seam_view.seam.clone();

    let viewport = Viewport {
        x: ui.window_pos()[0],
        y: ui.window_pos()[1],
        width: ui.window_size()[0],
        height: ui.window_size()[1],
    };

    let screen_mouse_pos = get_norm_mouse_pos(ui.io().mouse_pos, ui.window_pos(), ui.window_size());
    let screen_mouse_pos = Point3f::new(screen_mouse_pos.0, screen_mouse_pos.1, 0.0);

    let mut camera = get_seam_view_camera(seam_view, &viewport);
    let mut world_mouse_pos = seam_view_screen_to_world(&camera, &viewport, screen_mouse_pos);

    if ui.is_mouse_clicked(MouseButton::Left)
        && !ui.is_any_item_hovered()
        && view.export_form.is_none()
        && screen_mouse_pos.x.abs() <= 1.0
        && screen_mouse_pos.y.abs() <= 1.0
    {
        seam_view.mouse_drag_start_pos = Some(world_mouse_pos);
    }
    if ui.is_mouse_down(MouseButton::Left) {
        if let Some(mouse_drag_start_pos) = seam_view.mouse_drag_start_pos {
            seam_view.camera_pos += mouse_drag_start_pos - world_mouse_pos;
            camera = get_seam_view_camera(seam_view, &viewport);
            world_mouse_pos = seam_view_screen_to_world(&camera, &viewport, screen_mouse_pos);
        }
    } else {
        seam_view.mouse_drag_start_pos = None;
    }

    if !ui.is_any_item_hovered()
        && screen_mouse_pos.x.abs() <= 1.0
        && screen_mouse_pos.y.abs() <= 1.0
    {
        seam_view.zoom += ui.io().mouse_wheel as f64 / 5.0;

        // Move camera to keep world mouse pos the same
        camera = get_seam_view_camera(seam_view, &viewport);
        let new_world_mouse_pos = seam_view_screen_to_world(&camera, &viewport, screen_mouse_pos);
        seam_view.camera_pos += world_mouse_pos - new_world_mouse_pos;

        camera = get_seam_view_camera(seam_view, &viewport);
        world_mouse_pos = seam_view_screen_to_world(&camera, &viewport, screen_mouse_pos);
    }

    let segment_length = camera.span_y as f32 / 100.0;

    let margin = 1.5;

    let span_w = camera.span_y * viewport.width as f64 / viewport.height as f64;
    let w = match seam.edge1.projection_axis {
        ProjectionAxis::X => camera.pos.z,
        ProjectionAxis::Z => camera.pos.x,
    };
    let left_w = (w - margin * span_w / 2.0) as f32;
    let right_w = (w + margin * span_w / 2.0) as f32;

    let top_y = (camera.pos.y + margin * camera.span_y / 2.0) as f32;
    let bottom_y = (camera.pos.y - margin * camera.span_y / 2.0) as f32;
    let top_w = seam.edge1.approx_w(top_y) - 1.0;
    let bottom_w = seam.edge1.approx_w(bottom_y) + 1.0;

    // TODO: Compute this better to avoid things disappearing when zooming in
    let min_w = (left_w.max(top_w.min(bottom_w)).max(seam.w_range().start) / segment_length)
        .floor()
        * segment_length;
    let max_w = right_w.min(top_w.max(bottom_w)).min(seam.w_range().end);
    let visible_w_range = RangeF32::inclusive(min_w, max_w);

    let progress =
        view.seam_processor
            .focused_seam_progress(&seam, visible_w_range, segment_length);

    let mut vertical_grid_lines = Vec::new();
    let mut horizontal_grid_lines = Vec::new();

    let (left_w_range, right_w_range) =
        RangeF32::inclusive(left_w, right_w).cut_out(&RangeF32::inclusive_exclusive(-1.0, 1.0));
    if left_w_range.count() + right_w_range.count() < 100 {
        for w in left_w_range.iter().chain(right_w_range.iter()) {
            vertical_grid_lines.push(Point3::new(w as f64, 0.0, w as f64));
        }
    }

    let (left_y_range, right_y_range) =
        RangeF32::inclusive(bottom_y, top_y).cut_out(&RangeF32::inclusive_exclusive(-1.0, 1.0));
    if left_y_range.count() + right_y_range.count() < 100 {
        for y in left_y_range.iter().chain(right_y_range.iter()) {
            horizontal_grid_lines.push(Point3::new(0.0, y as f64, 0.0));
        }
    }

    let scene = SeamViewScene {
        viewport,
        camera,
        seam: get_focused_seam_info(&seam, &progress),
        vertical_grid_lines,
        horizontal_grid_lines,
    };

    let close_seam_view = ui.button(im_str!("Close"), [0.0, 0.0]);

    ui.same_line(50.0);
    if ui.button(im_str!("Export"), [0.0, 0.0]) {
        view.export_form = Some(SeamExportForm::new(
            seam.clone(),
            view.seam_processor.filter(),
        ));
    }

    ui.spacing();

    let rounded_mouse = point_f64_to_f32(world_mouse_pos);
    match seam.edge1.projection_axis {
        ProjectionAxis::X => {
            ui.text(im_str!("(_, {}, {})", rounded_mouse.y, rounded_mouse.z));
            ui.text(im_str!(
                "(_, {:#08X}, {:#08X})",
                rounded_mouse.y.to_bits(),
                rounded_mouse.z.to_bits(),
            ));
        }
        ProjectionAxis::Z => {
            ui.text(im_str!("({}, {}, _)", rounded_mouse.x, rounded_mouse.y));
            ui.text(im_str!(
                "({:#08X}, {:#08X}, _)",
                rounded_mouse.x.to_bits(),
                rounded_mouse.y.to_bits(),
            ));
        }
    }

    if close_seam_view {
        view.seam_view = None;
    }
    scene
}

fn get_seam_view_camera(seam_view: &mut SeamViewState, viewport: &Viewport) -> SeamViewCamera {
    let seam = &seam_view.seam;

    let w_axis = match seam.edge1.projection_axis {
        ProjectionAxis::X => Vector3::z(),
        ProjectionAxis::Z => Vector3::x(),
    };
    let screen_right = match seam.edge1.orientation {
        Orientation::Positive => -w_axis,
        Orientation::Negative => w_axis,
    };

    let initial_span_y = *seam_view.initial_span_y.get_or_insert_with(|| {
        let w_range = seam.edge1.w_range();
        let y_range = seam.edge1.y_range();
        (y_range.end - y_range.start + 50.0)
            .max((w_range.end - w_range.start + 50.0) * viewport.height / viewport.width)
            as f64
    });
    let span_y = initial_span_y / 2.0f64.powf(seam_view.zoom);

    SeamViewCamera {
        pos: seam_view.camera_pos,
        span_y,
        right_dir: screen_right,
    }
}

fn render_export_form(ui: &Ui, form: &mut SeamExportForm) -> bool {
    let style_token = ui.push_style_color(imgui::StyleColor::WindowBg, [0.06, 0.06, 0.06, 0.94]);

    let mut opened = true;
    imgui::Window::new(im_str!("Export seam data"))
        .size([500.0, 300.0], Condition::Appearing)
        .opened(&mut opened)
        .build(ui, || {
            let show_point =
                |projection_axis: ProjectionAxis, point: ProjectedPoint<i16>| match projection_axis
                {
                    ProjectionAxis::X => format!("(_, {}, {})", point.y, point.w),
                    ProjectionAxis::Z => format!("({}, {}, _)", point.w, point.y),
                };
            let show_edge = |edge: Edge| {
                let normal_info = match (edge.projection_axis, edge.orientation) {
                    (ProjectionAxis::X, Orientation::Positive) => "x+",
                    (ProjectionAxis::X, Orientation::Negative) => "x-",
                    (ProjectionAxis::Z, Orientation::Positive) => "z-",
                    (ProjectionAxis::Z, Orientation::Negative) => "z+",
                };
                format!(
                    "v1 = {}, v2 = {}, n = {}",
                    show_point(edge.projection_axis, edge.vertex1),
                    show_point(edge.projection_axis, edge.vertex2),
                    normal_info,
                )
            };

            ui.text(im_str!("edge 1: {}", show_edge(form.seam.edge1)));
            ui.text(im_str!("edge 2: {}", show_edge(form.seam.edge2)));

            ui.spacing();
            let all_filters = PointFilter::all();
            let mut filter_index = all_filters
                .iter()
                .position(|filter| form.filter == *filter)
                .unwrap();
            ui.set_next_item_width(100.0);
            if imgui::ComboBox::new(im_str!("##filter")).build_simple(
                ui,
                &mut filter_index,
                &all_filters,
                &|filter| im_str!("{}", filter).into(),
            ) {
                form.filter = all_filters[filter_index];
            }

            ui.spacing();
            ui.checkbox(im_str!("Include [-1, 1]"), &mut form.include_small_w);

            let coord_axis_str = match form.seam.edge1.projection_axis {
                ProjectionAxis::X => "z",
                ProjectionAxis::Z => "x",
            };

            ui.spacing();

            ui.text(im_str!("min {}: ", coord_axis_str));
            ui.same_line(80.0);
            ui.set_next_item_width(100.0);
            if ui
                .input_text(im_str!("##min-w"), &mut form.min_w_buffer)
                .build()
            {
                form.min_w = form.min_w_buffer.to_str().parse::<f32>().ok();
            }

            ui.text(im_str!("max {}: ", coord_axis_str));
            ui.same_line(80.0);
            ui.set_next_item_width(100.0);
            if ui
                .input_text(im_str!("##max-w"), &mut form.max_w_buffer)
                .build()
            {
                form.max_w = form.max_w_buffer.to_str().parse::<f32>().ok();
            }

            if let Some(min_w) = form.min_w {
                if let Some(max_w) = form.max_w {
                    ui.spacing();
                    if ui.button(im_str!("Export"), [0.0, 0.0]) {
                        let w_range = RangeF32::inclusive(min_w, max_w);
                        dbg!(w_range);
                    }
                }
            }
        });

    style_token.pop(ui);
    opened
}
