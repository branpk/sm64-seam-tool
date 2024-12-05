use crate::{
    edge::{Edge, ProjectedPoint, ProjectionAxis},
    float_range::RangeF32,
    float_range::next_f32,
    float_range::prev_f32,
    game_state::{GameState, Globals},
    geo::{Point3f, Vector3f, direction_to_pitch_yaw, pitch_yaw_to_direction},
    graphics::{self, Camera, GameViewScene, RotateCamera, SurfaceType, Viewport},
    model::ExportProgress,
    process::Process,
    seam::PointFilter,
    seam::PointStatusFilter,
    seam::Seam,
    seam_processor::{SeamOutput, SeamProcessor, SeamProgress},
};
use graphics::{FocusedSeamData, FocusedSeamInfo, SeamInfo, SeamSegment, SeamViewCamera};
use std::{
    collections::HashSet,
    f32::consts::PI,
    io,
    io::Write,
    time::{Duration, Instant},
};

pub fn get_norm_mouse_pos(
    mouse_pos: [f32; 2],
    window_pos: [f32; 2],
    window_size: [f32; 2],
) -> (f32, f32) {
    let rel_mouse_pos = (
        mouse_pos[0] - window_pos[0],
        window_size[1] - mouse_pos[1] + window_pos[1],
    );
    (
        2.0 * rel_mouse_pos.0 / window_size[0] - 1.0,
        2.0 * rel_mouse_pos.1 / window_size[1] - 1.0,
    )
}

pub fn get_mouse_ray(
    mouse_pos: [f32; 2],
    window_pos: [f32; 2],
    window_size: [f32; 2],
    camera: &RotateCamera,
) -> Option<(Point3f, Vector3f)> {
    let norm_mouse_pos = get_norm_mouse_pos(mouse_pos, window_pos, window_size);
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

pub fn ray_surface_intersection(
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

pub fn find_hovered_seam(
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
        .find(|seam| seam.edge1 == edge || seam.edge2 == edge)
        .cloned()
}

pub fn build_game_view_scene(
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

                graphics::Surface {
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
                get_segment_info(seam, &progress)
            })
            .collect(),
        hovered_seam,
    }
}

pub fn get_segment_info(seam: &Seam, progress: &SeamProgress) -> SeamInfo {
    let segments = progress
        .segments()
        .map(|(range, status)| {
            let endpoint1 = seam.approx_point_at_w(range.start);
            let endpoint2 = seam.approx_point_at_w(range.end);
            SeamSegment {
                endpoint1,
                endpoint2,
                proj_endpoint1: ProjectedPoint {
                    w: range.start,
                    y: endpoint1[1],
                },
                proj_endpoint2: ProjectedPoint {
                    w: range.end,
                    y: endpoint2[1],
                },
                status,
            }
        })
        .collect();

    SeamInfo {
        seam: seam.clone(),
        segments,
    }
}

pub fn get_focused_seam_info(seam: &Seam, output: &SeamOutput) -> FocusedSeamInfo {
    match output {
        SeamOutput::Points(points) => FocusedSeamInfo {
            seam: seam.clone(),
            data: FocusedSeamData::Points(
                points
                    .points
                    .iter()
                    .map(|(point, status)| {
                        let pos = match seam.edge1.projection_axis {
                            ProjectionAxis::X => Point3f::new(0.0, point.y, point.w),
                            ProjectionAxis::Z => Point3f::new(point.w, point.y, 0.0),
                        };
                        (pos, *status)
                    })
                    .collect(),
            ),
        },
        SeamOutput::Segments(segments) => {
            let segments = get_segment_info(seam, segments).segments;
            FocusedSeamInfo {
                seam: seam.clone(),
                data: FocusedSeamData::Segments(segments),
            }
        }
    }
}

pub fn get_visible_w_range(
    camera: &SeamViewCamera,
    viewport: &Viewport,
    projection_axis: ProjectionAxis,
) -> RangeF32 {
    let span_x = camera.span_y * viewport.width as f64 / viewport.height as f64;
    let camera_w = match projection_axis {
        ProjectionAxis::X => camera.pos.z,
        ProjectionAxis::Z => camera.pos.x,
    };

    let h_min_w = prev_f32((camera_w - span_x / 2.0) as f32);
    let h_max_w = next_f32((camera_w + span_x / 2.0) as f32);

    RangeF32::inclusive(h_min_w, h_max_w)
}

pub fn get_visible_y_range(camera: &SeamViewCamera) -> RangeF32 {
    RangeF32::inclusive(
        prev_f32((camera.pos.y - camera.span_y / 2.0) as f32),
        next_f32((camera.pos.y + camera.span_y / 2.0) as f32),
    )
}

pub fn get_visible_w_range_for_seam(
    camera: &SeamViewCamera,
    viewport: &Viewport,
    seam: &Seam,
) -> RangeF32 {
    let h_range = get_visible_w_range(camera, viewport, seam.edge1.projection_axis);

    let top_y = camera.pos.y + camera.span_y / 2.0;
    let top_w = seam.edge1.approx_w_f64(top_y);
    let bottom_y = camera.pos.y - camera.span_y / 2.0;
    let bottom_w = seam.edge1.approx_w_f64(bottom_y);

    let v_range = RangeF32::inclusive(
        prev_f32(top_w.min(bottom_w) as f32),
        next_f32(top_w.max(bottom_w) as f32),
    );

    seam.w_range().intersect(&h_range).intersect(&v_range)
}

pub fn canonicalize_process_name(name: &str) -> String {
    name.trim_end_matches(".exe")
        .replace("_", "-")
        .to_lowercase()
}

pub fn sync_to_game(process: &Process, globals: &Globals) {
    let initial_global_timer: u32 = process.read(globals.global_timer);
    let start_time = Instant::now();
    while start_time.elapsed() < Duration::from_millis(50) {
        let global_timer: u32 = process.read(globals.global_timer);
        if global_timer != initial_global_timer {
            break;
        }
    }
}

pub fn save_seam_to_csv(
    writer: &mut impl Write,
    mut set_progress: impl FnMut(Option<ExportProgress>),
    seam: &Seam,
    point_filter: PointFilter,
    status_filter: PointStatusFilter,
    include_small_w: bool,
    w_range: RangeF32,
) -> io::Result<()> {
    match seam.edge1.projection_axis {
        ProjectionAxis::X => writeln!(writer, "z,z hex,y,y hex,type")?,
        ProjectionAxis::Z => writeln!(writer, "x,x hex,y,y hex,type")?,
    }

    let w_ranges = if include_small_w {
        vec![w_range]
    } else {
        let (left, right) = w_range.cut_out(&RangeF32::inclusive_exclusive(-1.0, 1.0));
        vec![left, right]
    };

    let total = w_ranges.iter().map(|range| range.count()).sum();
    let mut complete = 0;

    for w in w_ranges
        .into_iter()
        .flat_map(|range| range.iter().collect::<Vec<_>>())
    {
        let (y, status) = seam.check_point(w, point_filter);
        complete += 1;

        if complete % 100_000 == 0 {
            set_progress(Some(ExportProgress { complete, total }));
        }

        if status_filter.matches(status) {
            writeln!(
                writer,
                "{},{:#08X},{},{:#08X},{}",
                w,
                w.to_bits(),
                y,
                y.to_bits(),
                status,
            )?;
        }
    }

    writer.flush()?;
    set_progress(None);
    Ok(())
}
