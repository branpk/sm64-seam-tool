use crate::{
    edge::{Edge, ProjectionAxis},
    float_range::RangeF32,
};

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
            Some(Seam {
                edge1,
                edge2,
                endpoints: vertices1,
            })
        } else {
            None
        }
    }
}
