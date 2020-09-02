use crate::{
    float_range::{step_f32_by, RangeF32},
    game_state::{GameState, Surface},
    seam::{RangeStatus, Seam},
    spatial_partition::SpatialPartition,
};
use itertools::Itertools;
use std::{
    collections::{HashMap, VecDeque},
    iter,
    time::{Duration, Instant},
};

const MAX_SEGMENT_SIZE: i32 = 100_000;
const MAX_SEGMENT_LENGTH: f32 = 10.0;

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

        let mut split = step_f32_by(self.remaining.start, MAX_SEGMENT_SIZE)
            .min(self.remaining.start + MAX_SEGMENT_LENGTH)
            .min(self.remaining.end);
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
    queue: VecDeque<Seam>,
}

impl SeamProcessor {
    pub fn new() -> Self {
        Self {
            active_seams: Vec::new(),
            progress: HashMap::new(),
            queue: VecDeque::new(),
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

        let mut spatial_partition = SpatialPartition::new();
        for wall in walls {
            spatial_partition.insert(wall.clone());
        }

        for (wall1, wall2) in spatial_partition.pairs() {
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

        let active_seams = &self.active_seams;
        self.queue.retain(|seam| active_seams.contains(seam));

        if self.queue.is_empty() {
            for seam in &self.active_seams {
                if !self.seam_progress(seam).is_complete() {
                    self.queue.push_back(seam.clone());
                }
            }
        }

        if let Some(seam) = self.queue.pop_front() {
            let progress = self
                .progress
                .entry(seam.clone())
                .or_insert_with(|| SeamProgress::new(seam.w_range()));

            let start_time = Instant::now();
            while start_time.elapsed() < Duration::from_millis(16) {
                if let Some(range) = progress.take_next_segment() {
                    progress.complete_segment(range, seam.check_range(range));
                }
            }

            if !progress.is_complete() {
                self.queue.push_front(seam);
            }
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
