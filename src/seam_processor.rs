use crate::{
    float_range::RangeF32,
    game_state::{GameState, Surface},
    seam::{RangeInteraction, Seam},
    spatial_partition::SpatialPartition,
};
use itertools::Itertools;
use std::collections::HashMap;

// TODO: Break up by number of points, not length of segment
const SEGMENT_LENGTH: f32 = 10.0;

#[derive(Debug, Clone)]
pub struct SeamProgress {
    pub complete: Vec<(RangeF32, RangeInteraction)>,
    pub remaining: RangeF32,
}

impl SeamProgress {
    fn new(range: RangeF32) -> Self {
        Self {
            complete: Vec::new(),
            remaining: range,
        }
    }

    fn take_next_segment(&mut self) -> Option<RangeF32> {
        if self.remaining.is_empty() {
            None
        } else {
            let split = (self.remaining.start + SEGMENT_LENGTH).min(self.remaining.end);
            let result = RangeF32::inclusive_exclusive(self.remaining.start, split);
            self.remaining.start = split;
            Some(result)
        }
    }

    fn complete_segment(&mut self, range: RangeF32, interaction: RangeInteraction) {
        if let Some(prev) = self.complete.last_mut() {
            if prev.0.end == range.start && prev.1 == interaction {
                prev.0.end = range.end;
                return;
            }
        }
        self.complete.push((range, interaction));
    }
}

pub struct SeamProcessor {
    seams: Vec<Seam>,
    progress: HashMap<Seam, SeamProgress>,
}

impl SeamProcessor {
    pub fn new() -> Self {
        Self {
            seams: Vec::new(),
            progress: HashMap::new(),
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

        self.seams.clear();

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
                        self.seams.push(seam);
                    }
                }
            }
        }
    }

    pub fn update(&mut self, state: &GameState) {
        self.find_seams(state);

        for seam in &self.seams {
            let progress = self
                .progress
                .entry(seam.clone())
                .or_insert_with(|| SeamProgress::new(seam.w_range()));

            if let Some(range) = progress.take_next_segment() {
                progress.complete_segment(range, seam.check_range(range));
                break;
            }
        }
    }

    pub fn seams(&self) -> &[Seam] {
        &self.seams
    }

    pub fn seam_progress(&self, seam: &Seam) -> SeamProgress {
        self.progress
            .get(seam)
            .cloned()
            .unwrap_or(SeamProgress::new(seam.w_range()))
    }
}
