use crate::{
    float_range::RangeF32,
    game_state::{GameState, Surface},
    seam::{RangeStatus, Seam},
    spatial_partition::SpatialPartition,
};
use rayon::prelude::*;
use std::{
    collections::{HashMap, VecDeque},
    iter,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const DEFAULT_SEGMENT_LENGTH: f32 = 5.0;

#[derive(Debug, Clone, PartialEq)]
struct SeamRequest {
    seam: Seam,
    w_range: RangeF32,
    segment_length: f32,
    is_focused: bool,
}

impl SeamRequest {
    fn unfocused(seam: Seam) -> Self {
        let w_range = seam.w_range();
        Self {
            seam,
            w_range,
            segment_length: DEFAULT_SEGMENT_LENGTH,
            is_focused: false,
        }
    }

    fn focused(seam: Seam, w_range: RangeF32, segment_length: f32) -> Self {
        Self {
            seam,
            w_range,
            segment_length,
            is_focused: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SeamProgress {
    segment_length: f32,
    complete: Vec<(RangeF32, RangeStatus)>,
    remaining: RangeF32,
}

impl SeamProgress {
    fn new(range: RangeF32, segment_length: f32) -> Self {
        Self {
            segment_length,
            complete: Vec::new(),
            remaining: range,
        }
    }

    pub fn segments(&self) -> impl Iterator<Item = (RangeF32, RangeStatus)> + '_ {
        self.complete
            .iter()
            .cloned()
            .chain(iter::once((self.remaining, RangeStatus::Unchecked)))
    }

    fn is_complete(&self) -> bool {
        self.remaining.is_empty()
    }

    fn take_next_segment(&mut self) -> Option<RangeF32> {
        if self.remaining.is_empty() {
            return None;
        }

        if self.remaining.start >= -1.0 && self.remaining.start < 1.0 {
            let split = 1.0f32.min(self.remaining.end);
            let skipped_range = RangeF32::inclusive_exclusive(self.remaining.start, split);
            self.remaining.start = split;
            self.complete_segment(skipped_range, RangeStatus::Skipped);
        }

        if self.remaining.is_empty() {
            return None;
        }

        let mut split = (self.remaining.start + self.segment_length).min(self.remaining.end);
        if self.remaining.start < -1.0 && split > -1.0 {
            split = -1.0;
        }

        let result = RangeF32::inclusive_exclusive(self.remaining.start, split);
        self.remaining.start = split;

        Some(result)
    }

    fn complete_segment(&mut self, range: RangeF32, status: RangeStatus) {
        assert_ne!(status, RangeStatus::Unchecked);
        if !range.is_empty() {
            if let Some(prev) = self.complete.last_mut() {
                if prev.0.end == range.start && prev.1 == status {
                    prev.0.end = range.end;
                    return;
                }
            }
            self.complete.push((range, status));
        }
    }
}

pub struct SeamProcessor {
    active_seams: Vec<Seam>,
    progress: HashMap<Seam, SeamProgress>,
    queue: Arc<Mutex<VecDeque<SeamRequest>>>,
    output_receiver: Receiver<(SeamRequest, SeamProgress)>,
    focused_seam: Option<(SeamRequest, SeamProgress)>,
}

impl SeamProcessor {
    pub fn new() -> Self {
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let queue2 = queue.clone();

        let (sender, receiver) = channel();
        thread::spawn(move || processor_thread(Arc::clone(&queue2), sender));

        Self {
            active_seams: Vec::new(),
            progress: HashMap::new(),
            queue,
            output_receiver: receiver,
            focused_seam: None,
        }
    }

    fn find_seams(&mut self, state: &GameState) {
        let get_edges = |surface: &Surface| {
            [
                (surface.vertex1, surface.vertex2),
                (surface.vertex2, surface.vertex3),
                (surface.vertex3, surface.vertex1),
            ]
        };

        self.active_seams.clear();

        let walls = state
            .surfaces
            .iter()
            .filter(|surface| surface.normal[1].abs() <= 0.01);

        let start_time = Instant::now();
        let cutoff = Duration::from_secs_f32(1.0);

        let mut spatial_partition = SpatialPartition::new();
        for wall in walls {
            if start_time.elapsed() > cutoff {
                // Probably an invalid surface pool
                self.active_seams.clear();
                return;
            }

            spatial_partition.insert(wall.clone());
        }

        for (wall1, wall2) in spatial_partition.pairs() {
            if start_time.elapsed() > cutoff {
                self.active_seams.clear();
                return;
            }

            let edges1 = get_edges(wall1);
            let edges2 = get_edges(wall2);

            for edge1 in &edges1 {
                for edge2 in &edges2 {
                    if let Some(seam) = Seam::between(*edge1, wall1.normal, *edge2, wall2.normal) {
                        self.active_seams.push(seam);
                    }
                }
            }
        }
    }

    pub fn update(&mut self, state: &GameState) {
        self.find_seams(state);

        {
            let mut queue = self.queue.lock().unwrap();

            queue.retain(|request| self.active_seams.contains(&request.seam));

            if queue.is_empty() {
                for seam in &self.active_seams {
                    if !self.progress.contains_key(seam) {
                        queue.push_back(SeamRequest::unfocused(seam.clone()));
                    }
                }
            }
        }

        while let Ok((request, progress)) = self.output_receiver.try_recv() {
            if request.is_focused {
                if let Some((focused_request, _)) = &self.focused_seam {
                    if focused_request == &request {
                        self.focused_seam = Some((request, progress));
                    }
                }
            } else {
                self.progress.insert(request.seam, progress);
            }
        }
    }

    pub fn focused_seam_progress(
        &mut self,
        seam: &Seam,
        w_range: RangeF32,
        segment_length: f32,
    ) -> SeamProgress {
        let request = SeamRequest::focused(seam.clone(), w_range, segment_length);
        let mut progress = SeamProgress::new(w_range, segment_length);

        if let Some((focused_request, focused_progress)) = &self.focused_seam {
            if &focused_request.seam == seam {
                progress = focused_progress.clone();
            }
            if focused_request == &request {
                return progress;
            }
        }

        self.focused_seam = Some((request.clone(), progress.clone()));

        {
            let mut queue = self.queue.lock().unwrap();
            queue.clear();
            queue.push_back(request);
        }

        progress
    }

    pub fn active_seams(&self) -> &[Seam] {
        &self.active_seams
    }

    pub fn remaining_seams(&self) -> usize {
        self.active_seams
            .iter()
            .filter(|seam| !self.seam_progress(seam).is_complete())
            .count()
    }

    pub fn seam_progress(&self, seam: &Seam) -> SeamProgress {
        self.progress
            .get(seam)
            .cloned()
            .unwrap_or(SeamProgress::new(seam.w_range(), DEFAULT_SEGMENT_LENGTH))
    }
}

fn processor_thread(
    queue: Arc<Mutex<VecDeque<SeamRequest>>>,
    output: Sender<(SeamRequest, SeamProgress)>,
) {
    loop {
        let head = queue.lock().unwrap().pop_front();
        if let Some(request) = head {
            let mut progress = SeamProgress::new(request.w_range, request.segment_length);

            let mut segments = Vec::new();
            while let Some(segment) = progress.take_next_segment() {
                segments.push(segment);
            }

            let segment_statuses: Vec<(RangeF32, RangeStatus)> = segments
                .into_par_iter()
                .map(|segment| (segment, request.seam.check_range(segment)))
                .collect();

            for (segment, status) in segment_statuses {
                progress.complete_segment(segment, status);
                let _ = output.send((request.clone(), progress.clone()));
            }
        }
    }
}
