use crate::{
    float_range::{next_f32, prev_f32, RangeF32},
    game_state::{Config, Globals},
    geo::point_f32_to_f64,
    process::Process,
    seam::{PointFilter, Seam},
    seam_processor::SeamProcessor,
};
use imgui::{im_str, ImString};
use nalgebra::Point3;
use std::fs;
use sysinfo::{System, SystemExt};

#[derive(Debug)]
pub enum App {
    ConnectionMenu(ConnectionMenu),
    Connected(ConnectedView),
}

impl App {
    pub fn new() -> Self {
        Self::ConnectionMenu(ConnectionMenu::new())
    }
}

#[derive(Debug)]
pub struct ConnectionMenu {
    pub config: Config,
    pub system: System,
    pub selected_pid: Option<usize>,
    pub base_addr_buffer: ImString,
    pub selected_base_addr: Option<usize>,
    pub selected_version_index: usize,
}

impl ConnectionMenu {
    pub fn new() -> Self {
        let config_text = fs::read_to_string("config.json").unwrap();
        let config = json5::from_str(&config_text).unwrap();
        Self {
            config,
            system: System::new(),
            selected_pid: None,
            base_addr_buffer: ImString::with_capacity(32),
            selected_base_addr: None,
            selected_version_index: 0,
        }
    }
}

#[derive(Debug)]
pub struct ConnectedView {
    pub process: Process,
    pub globals: Globals,
    pub sync_to_game: bool,
    pub seam_processor: SeamProcessor,
    pub hovered_seam: Option<Seam>,
    pub seam_view: Option<SeamViewState>,
    pub fps_string: String,
    pub export_form: Option<SeamExportForm>,
}

impl ConnectedView {
    pub fn new(pid: u32, base_address: usize, globals: Globals) -> Self {
        Self {
            process: Process::attach(pid, base_address),
            globals,
            sync_to_game: false,
            seam_processor: SeamProcessor::new(),
            hovered_seam: None,
            seam_view: None,
            fps_string: String::new(),
            export_form: None,
        }
    }
}

#[derive(Debug)]
pub struct SeamViewState {
    pub seam: Seam,
    pub camera_pos: Point3<f64>,
    pub mouse_drag_start_pos: Option<Point3<f64>>,
    pub zoom: f64,
    pub initial_span_y: Option<f64>,
}

impl SeamViewState {
    pub fn new(seam: Seam) -> Self {
        let camera_pos = seam.endpoint1() + (seam.endpoint2() - seam.endpoint1()) / 2.0;
        Self {
            seam,
            camera_pos: point_f32_to_f64(camera_pos),
            mouse_drag_start_pos: None,
            zoom: 0.0,
            initial_span_y: None,
        }
    }
}

#[derive(Debug)]
pub struct SeamExportForm {
    pub seam: Seam,
    pub filter: PointFilter,
    pub include_small_w: bool,
    pub min_w: Option<f32>,
    pub max_w: Option<f32>,
    pub min_w_buffer: ImString,
    pub max_w_buffer: ImString,
}

impl SeamExportForm {
    pub fn new(seam: Seam, filter: PointFilter) -> Self {
        let w_range = seam.w_range();
        let mut min_w_buffer = im_str!("{}", w_range.start);
        min_w_buffer.reserve(32);
        let mut max_w_buffer = im_str!("{}", prev_f32(w_range.end));
        max_w_buffer.reserve(32);

        Self {
            seam,
            filter,
            include_small_w: false,
            min_w: Some(w_range.start),
            max_w: Some(prev_f32(w_range.end)),
            min_w_buffer,
            max_w_buffer,
        }
    }
}
