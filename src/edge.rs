/// The axis along which a wall projects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProjectionAxis {
    X,
    Z,
}

impl ProjectionAxis {
    /// Determine the projection axis for a wall given its normal vector.
    pub fn of_wall(normal: &[f32; 3]) -> Self {
        if normal[0] < -0.707 || normal[0] > 0.707 {
            Self::X
        } else {
            Self::Z
        }
    }
}

/// The orientation of a wall.
///
/// An x projective surface is positive iff `normal.x > 0`.
/// A z projective surfaces is positive iff `normal.z <= 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Orientation {
    /// Accept r if r >= 0.
    Positive,
    /// Accept r if r <= 0.
    Negative,
}

impl Orientation {
    /// Get the orientation for a wall given its normal vector.
    pub fn of_wall(normal: &[f32; 3]) -> Self {
        match ProjectionAxis::of_wall(normal) {
            ProjectionAxis::X => {
                if normal[0] > 0.0 {
                    Self::Positive
                } else {
                    Self::Negative
                }
            }
            ProjectionAxis::Z => {
                if normal[2] <= 0.0 {
                    Self::Positive
                } else {
                    Self::Negative
                }
            }
        }
    }
}

/// A projected point used for edge calculations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProjectedPoint<T> {
    /// The relevant non-y coordinate.
    ///
    /// Equal to x for z projective surfaces, and z for x projective surfaces.
    pub w: T,
    /// The y coordinate.
    pub y: T,
}

impl<T: Clone> ProjectedPoint<T> {
    /// Project the point along the given axis.
    pub fn project(point: [T; 3], axis: ProjectionAxis) -> Self {
        match axis {
            ProjectionAxis::X => Self {
                w: point[2].clone(),
                y: point[1].clone(),
            },
            ProjectionAxis::Z => Self {
                w: point[0].clone(),
                y: point[1].clone(),
            },
        }
    }
}

/// An edge of a wall.
///
/// `vertex1`, `vertex2` should be listed in CCW order (i.e. match the game's order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Edge {
    pub projection_axis: ProjectionAxis,
    pub vertex1: ProjectedPoint<i16>,
    pub vertex2: ProjectedPoint<i16>,
    pub orientation: Orientation,
}

impl Edge {
    /// Return true if the given point lies on the inside of the edge.
    ///
    /// A point is inside a wall iff all three of the wall's edges accept the point.
    pub fn accepts(&self, point: [f32; 3]) -> bool {
        self.accepts_projected(ProjectedPoint::project(point, self.projection_axis))
    }

    /// Return true if the projected point lies on the inside of the edge.
    pub fn accepts_projected(&self, point: ProjectedPoint<f32>) -> bool {
        let w = point.w;
        let y = point.y;

        let w1 = self.vertex1.w as f32;
        let y1 = self.vertex1.y as f32;

        let w2 = self.vertex2.w as f32;
        let y2 = self.vertex2.y as f32;

        let r = (y1 - y) * (w2 - w1) - (w1 - w) * (y2 - y1);

        match self.orientation {
            Orientation::Positive => r >= 0.0,
            Orientation::Negative => r <= 0.0,
        }
    }
}
