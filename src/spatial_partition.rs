use crate::game_state::Surface;
use itertools::iproduct;
use std::{
    collections::{HashMap, HashSet},
    ops::RangeInclusive,
};

const BUCKET_SIZE: i16 = 200;

type BucketKey = (i16, i16);

pub struct SpatialPartition {
    surfaces: Vec<Surface>,
    buckets: HashMap<BucketKey, Vec<usize>>,
}

impl SpatialPartition {
    pub fn new() -> Self {
        Self {
            surfaces: Vec::new(),
            buckets: HashMap::new(),
        }
    }

    fn coord_range(&self, surface: &Surface, index: usize) -> RangeInclusive<i16> {
        let min = surface.vertex1[index]
            .min(surface.vertex2[index])
            .min(surface.vertex3[index]);
        let max = surface.vertex1[index]
            .max(surface.vertex2[index])
            .max(surface.vertex3[index]);

        let min_bucket = min.div_euclid(BUCKET_SIZE);
        let max_bucket = max.div_euclid(BUCKET_SIZE) + 1;

        min_bucket..=max_bucket
    }

    fn surface_buckets(&self, surface: &Surface) -> impl Iterator<Item = BucketKey> + use<> {
        let x_range = self.coord_range(surface, 0);
        let z_range = self.coord_range(surface, 2);

        iproduct!(x_range, z_range)
    }

    pub fn insert(&mut self, surface: Surface) {
        let index = self.surfaces.len();
        self.surfaces.push(surface);

        for bucket in self.surface_buckets(&surface) {
            self.buckets.entry(bucket).or_default().push(index);
        }
    }

    fn nearby_surface_indices(&self, surface: &Surface) -> HashSet<usize> {
        let mut indices = HashSet::new();
        for bucket in self.surface_buckets(surface) {
            for index in self.buckets.get(&bucket).into_iter().flatten() {
                indices.insert(*index);
            }
        }
        indices
    }

    pub fn pairs(&self) -> impl Iterator<Item = (&Surface, &Surface)> {
        self.surfaces
            .iter()
            .enumerate()
            .flat_map(move |(index1, surface1)| {
                self.nearby_surface_indices(surface1)
                    .into_iter()
                    .filter(move |index2| *index2 > index1)
                    .map(move |index2| (surface1, &self.surfaces[index2]))
            })
    }
}
