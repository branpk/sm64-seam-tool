use crate::{
    edge::{Edge, ProjectedPoint},
    float_range::{step_f32_by, RangeF32},
    geo::Point3f,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointStatus {
    Gap,
    Overlap,
    None,
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

    pub fn check_point(&self, w: f32) -> PointStatus {
        let y0 = self.edge1.approx_y(w);

        // TODO: Verify that we go far enough to be within each wall separately

        for i in -1..=1 {
            let y = step_f32_by(y0, i);
            let point = ProjectedPoint { w, y };

            let in1 = self.edge1.accepts_projected(point);
            let in2 = self.edge2.accepts_projected(point);

            if in1 && in2 {
                return PointStatus::Overlap;
            }
            if !in1 && !in2 {
                return PointStatus::Gap;
            }
        }

        PointStatus::None
    }

    pub fn check_range(&self, w_range: RangeF32) -> RangeStatus {
        let mut has_gap = false;
        let mut has_overlap = false;

        for w in w_range.iter() {
            match self.check_point(w) {
                PointStatus::Gap => has_gap = true,
                PointStatus::Overlap => has_overlap = true,
                PointStatus::None => {}
            }
        }

        RangeStatus::Checked {
            has_gap,
            has_overlap,
        }
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
