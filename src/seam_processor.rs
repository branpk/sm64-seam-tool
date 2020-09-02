use crate::{
    float_range::{step_f32_by, RangeF32},
    game_state::{GameState, Surface},
    seam::{RangeStatus, Seam},
    spatial_partition::SpatialPartition,
};
use itertools::Itertools;
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

const MAX_SEGMENT_SIZE: i32 = 100_000;
const MAX_SEGMENT_LENGTH: f32 = 5.0;

#[derive(Debug, Clone)]
pub struct SeamProgress {
    complete: Vec<(RangeF32, RangeStatus)>,
    remaining: RangeF32,
}

impl SeamProgress {
    fn new(range: RangeF32) -> Self {
        Self {
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

        let mut split = (self.remaining.start + MAX_SEGMENT_LENGTH).min(self.remaining.end);
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
    queue: Arc<Mutex<VecDeque<Seam>>>,
    output_receiver: Receiver<(Seam, SeamProgress)>,
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

            queue.retain(|seam| self.active_seams.contains(seam));

            if queue.is_empty() {
                for seam in &self.active_seams {
                    if !self.progress.contains_key(seam) {
                        queue.push_back(seam.clone());
                    }
                }
            }
        }

        while let Ok((seam, progress)) = self.output_receiver.try_recv() {
            self.progress.insert(seam, progress);
        }
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
            .unwrap_or(SeamProgress::new(seam.w_range()))
    }
}

fn processor_thread(queue: Arc<Mutex<VecDeque<Seam>>>, output: Sender<(Seam, SeamProgress)>) {
    loop {
        let head = queue.lock().unwrap().pop_front();
        if let Some(seam) = head {
            let mut progress = SeamProgress::new(seam.w_range());

            let mut segments = Vec::new();
            while let Some(segment) = progress.take_next_segment() {
                segments.push(segment);
            }

            let segment_statuses: Vec<(RangeF32, RangeStatus)> = segments
                .into_par_iter()
                .map(|segment| (segment, seam.check_range(segment)))
                .collect();

            for (segment, status) in segment_statuses {
                progress.complete_segment(segment, status);
                let _ = output.send((seam.clone(), progress.clone()));
            }
        }
    }
}
