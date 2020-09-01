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

pub fn step_f32_by(x: f32, mut amount: i32) -> f32 {
    let mut bits = x.to_bits();

    // negative to positive
    if (bits & (1 << 31)) != 0 {
        let to_zero = (bits & !(1 << 31)) as i32;
        if amount > to_zero {
            bits = 0;
            amount -= to_zero + 1;
        }
    }

    // positive to negative
    if (bits & (1 << 31)) == 0 {
        let to_zero = -(bits as i32);
        if amount < to_zero {
            bits = 1 << 31;
            amount += -to_zero + 1;
        }
    }

    if (bits & (1 << 31)) == 0 {
        bits = (bits as i32 + amount) as u32;
    } else {
        bits = (bits as i32 - amount) as u32;
    }

    f32::from_bits(bits)
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

    pub fn intersect(&self, other: &Self) -> Self {
        RangeF32::inclusive_exclusive(self.start.max(other.start), self.end.min(other.end))
    }

    pub fn split(&self, segment_length: f32) -> impl Iterator<Item = Self> {
        let start = self.start;
        let end = self.end;

        (0..)
            .map(move |i| {
                RangeF32::inclusive_exclusive(
                    start + i as f32 * segment_length,
                    (start + (i + 1) as f32 * segment_length).min(end),
                )
            })
            .take_while(|segment| !segment.is_empty())
    }
}
