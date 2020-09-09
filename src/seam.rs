use crate::{
    edge::{Edge, ProjectedPoint},
    float_range::{next_f32, prev_f32, RangeF32},
    geo::Point3f,
};
use std::fmt::{self, Display};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointFilter {
    None,
    IntY,
    QuarterIntY,
}

impl Default for PointFilter {
    fn default() -> Self {
        Self::None
    }
}

impl Display for PointFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PointFilter::None => write!(f, "all y"),
            PointFilter::IntY => write!(f, "int y"),
            PointFilter::QuarterIntY => write!(f, "qint y"),
        }
    }
}

impl PointFilter {
    pub fn all() -> Vec<Self> {
        vec![Self::None, Self::IntY, Self::QuarterIntY]
    }

    pub fn matches(&self, point: ProjectedPoint<f32>) -> bool {
        match self {
            PointFilter::None => true,
            PointFilter::IntY => point.y.fract() == 0.0,
            PointFilter::QuarterIntY => [0.0, 0.25, 0.5, 0.75].contains(&point.y.fract()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointStatus {
    Gap,
    Overlap,
    None,
}

impl Display for PointStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PointStatus::Gap => write!(f, "gap"),
            PointStatus::Overlap => write!(f, "overlap"),
            PointStatus::None => write!(f, "none"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointStatusFilter {
    GapsOnly,
    OverlapsOnly,
    GapsAndOverlaps,
    AllPoints,
}

impl PointStatusFilter {
    pub fn all() -> Vec<Self> {
        vec![
            Self::GapsOnly,
            Self::OverlapsOnly,
            Self::GapsAndOverlaps,
            Self::AllPoints,
        ]
    }

    pub fn matches(&self, status: PointStatus) -> bool {
        match self {
            PointStatusFilter::GapsOnly => status == PointStatus::Gap,
            PointStatusFilter::OverlapsOnly => status == PointStatus::Overlap,
            PointStatusFilter::GapsAndOverlaps => {
                status == PointStatus::Gap || status == PointStatus::Overlap
            }
            PointStatusFilter::AllPoints => true,
        }
    }
}

impl Display for PointStatusFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PointStatusFilter::GapsOnly => write!(f, "gaps only"),
            PointStatusFilter::OverlapsOnly => write!(f, "overlaps only"),
            PointStatusFilter::GapsAndOverlaps => write!(f, "gaps and overlaps"),
            PointStatusFilter::AllPoints => write!(f, "all points"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RangeStatus {
    Checked { has_gap: bool, has_overlap: bool },
    Unchecked,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Seam {
    pub edge1: Edge,
    pub edge2: Edge,
    /// For visualization
    pub endpoints: ([i16; 3], [i16; 3]),
}

impl Seam {
    pub fn between(
        vertices1: ([i16; 3], [i16; 3]),
        normal1: [f32; 3],
        vertices2: ([i16; 3], [i16; 3]),
        normal2: [f32; 3],
    ) -> Option<Seam> {
        let edge1 = Edge::new(vertices1, normal1);
        let edge2 = Edge::new(vertices2, normal2);

        // Simplifying assumption
        if edge1.projection_axis != edge2.projection_axis {
            return None;
        }

        // TODO: Edges that don't share vertices
        if vertices1.0 == vertices2.1 && vertices1.1 == vertices2.0 {
            let seam = Seam {
                edge1,
                edge2,
                endpoints: vertices1,
            };

            // Simplifying assumption
            if seam.edge1.is_vertical() || seam.edge2.is_vertical() {
                return None;
            }

            Some(seam)
        } else {
            None
        }
    }

    pub fn w_range(&self) -> RangeF32 {
        self.edge1.w_range().intersect(&self.edge2.w_range())
    }

    pub fn check_point(&self, w: f32, filter: PointFilter) -> (f32, PointStatus) {
        let y_approx = self.edge1.approx_y(w);

        let mut seen_in1 = false;
        let mut seen_in2 = false;

        let mut y_lo = y_approx;
        let mut y_hi = next_f32(y_approx);

        for i in 0..20 {
            if seen_in1 && seen_in2 {
                break;
            }

            let y;
            if i % 2 == 0 {
                y = y_lo;
                y_lo = prev_f32(y_lo);
            } else {
                y = y_hi;
                y_hi = next_f32(y_hi);
            }

            let point = ProjectedPoint { w, y };

            let in1 = self.edge1.accepts_projected(point);
            let in2 = self.edge2.accepts_projected(point);

            if in1 && !in2 {
                seen_in1 = true;
            }
            if in2 && !in1 {
                seen_in2 = true;
            }

            if filter.matches(point) {
                if in1 && in2 {
                    return (y, PointStatus::Overlap);
                }
                if !in1 && !in2 {
                    return (y, PointStatus::Gap);
                }
            }
        }

        (y_approx, PointStatus::None)
    }

    pub fn check_range(&self, w_range: RangeF32, filter: PointFilter) -> (usize, RangeStatus) {
        let mut has_gap = false;
        let mut has_overlap = false;
        let mut num_interesting_points = w_range.count();

        for w in w_range.iter() {
            match self.check_point(w, filter).1 {
                PointStatus::Gap => {
                    has_gap = true;
                    num_interesting_points += 1;
                }
                PointStatus::Overlap => {
                    has_overlap = true;
                    num_interesting_points += 1;
                }
                PointStatus::None => {}
            }
        }

        (
            num_interesting_points,
            RangeStatus::Checked {
                has_gap,
                has_overlap,
            },
        )
    }

    pub fn approx_point_at_w(&self, w: f32) -> [f32; 3] {
        let x1 = self.endpoints.0[0] as f32;
        let y1 = self.endpoints.0[1] as f32;
        let z1 = self.endpoints.0[2] as f32;

        let x2 = self.endpoints.1[0] as f32;
        let y2 = self.endpoints.1[1] as f32;
        let z2 = self.endpoints.1[2] as f32;

        let t = self.edge1.approx_t(w);
        [x1 + t * (x2 - x1), y1 + t * (y2 - y1), z1 + t * (z2 - z1)]
    }

    pub fn endpoint1(&self) -> Point3f {
        Point3f::new(
            self.endpoints.0[0] as f32,
            self.endpoints.0[1] as f32,
            self.endpoints.0[2] as f32,
        )
    }

    pub fn endpoint2(&self) -> Point3f {
        Point3f::new(
            self.endpoints.1[0] as f32,
            self.endpoints.1[1] as f32,
            self.endpoints.1[2] as f32,
        )
    }
}
