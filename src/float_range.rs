use std::iter;

/// Compute the next float larger than x.
pub fn step_f32(x: f32) -> f32 {
    let bits = x.to_bits();
    if (bits & (1 << 31)) == 0 {
        f32::from_bits(bits + 1)
    } else if bits == (1 << 31) {
        0.0
    } else {
        f32::from_bits(bits - 1)
    }
}

/// A closed range of float values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RangeF32 {
    pub start: f32,
    pub end: f32,
}

impl RangeF32 {
    pub fn inclusive_exclusive(start: f32, end: f32) -> Self {
        Self { start, end }
    }

    pub fn inclusive(min: f32, max: f32) -> Self {
        Self::inclusive_exclusive(min, step_f32(max))
    }

    pub fn empty() -> Self {
        Self::inclusive_exclusive(0.0, 0.0)
    }

    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    pub fn iter(&self) -> impl Iterator<Item = f32> {
        // Could be done more efficiently by chaining two integer ranges (negative then positive)
        let end = self.end;
        iter::successors(Some(self.start), |x| Some(step_f32(*x))).take_while(move |x| *x < end)
    }
}
