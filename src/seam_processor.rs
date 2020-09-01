use crate::{
    game_state::{GameState, Surface},
    seam::Seam,
    spatial_partition::SpatialPartition,
};
use itertools::Itertools;

pub struct SeamProcessor {
    seams: Vec<Seam>,
}

impl SeamProcessor {
    pub fn new() -> Self {
        Self { seams: Vec::new() }
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
    }

    pub fn seams(&self) -> &[Seam] {
        &self.seams
    }
}
