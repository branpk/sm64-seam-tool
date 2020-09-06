use std::iter;

pub fn flush_f32_to_zero(x: f32) -> f32 {
    let bits = x.to_bits();
    let exp = (bits & !(1 << 31)) >> 23;
    if exp == 0 {
        f32::from_bits(bits & (1 << 31))
    } else {
        x
    }
}

/// Compute the next float larger than x.
pub fn next_f32(x: f32) -> f32 {
    let x = flush_f32_to_zero(x);
    let bits = x.to_bits();
    let result_bits = if bits == 0 {
        1 << 23
    } else if (bits & (1 << 31)) == 0 {
        bits + 1
    } else if bits == (1 << 31) {
        0
    } else if bits == ((1 << 31) | (1 << 23)) {
        1 << 31
    } else {
        bits - 1
    };
    f32::from_bits(result_bits)
}

/// Compute the previous float smaller than x.
pub fn prev_f32(x: f32) -> f32 {
    let x = flush_f32_to_zero(x);
    let bits = x.to_bits();
    let result_bits = if bits == 0 {
        1 << 31
    } else if bits == (1 << 23) {
        0
    } else if (bits & (1 << 31)) == 0 {
        bits - 1
    } else if bits == (1 << 31) {
        (1 << 31) | (1 << 23)
    } else {
        bits + 1
    };
    f32::from_bits(result_bits)
}

pub fn f32s_between(start: f32, end: f32) -> u32 {
    let start = flush_f32_to_zero(start);
    let end = flush_f32_to_zero(end);
    if start >= end {
        return 0;
    }

    fn positive_lt(x: u32) -> u32 {
        if (x & (1 << 31)) == 0 {
            x - (1 << 23) + 1
        } else {
            0
        }
    }
    fn negative_ge(x: u32) -> u32 {
        positive_lt(x ^ (1 << 31)) + 1
    }

    let start_bits = start.to_bits();
    let end_bits = end.to_bits();
    let positive = positive_lt(end_bits) - positive_lt(start_bits);
    let negative = negative_ge(start_bits) - negative_ge(end_bits);
    positive + negative
}

/// A closed range of float values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RangeF32 {
    pub start: f32,
    pub end: f32,
}

impl RangeF32 {
    pub fn inclusive_exclusive(start: f32, end: f32) -> Self {
        Self {
            start: flush_f32_to_zero(start),
            end: flush_f32_to_zero(end),
        }
    }

    pub fn inclusive(min: f32, max: f32) -> Self {
        Self::inclusive_exclusive(min, next_f32(max))
    }

    pub fn empty() -> Self {
        Self::inclusive_exclusive(0.0, 0.0)
    }

    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    pub fn count(&self) -> usize {
        f32s_between(self.start, self.end) as usize
    }

    pub fn iter(&self) -> impl Iterator<Item = f32> {
        // Could be done more efficiently by chaining two integer ranges (negative then positive)
        let end = self.end;
        iter::successors(Some(self.start), |x| Some(next_f32(*x))).take_while(move |x| *x < end)
    }

    pub fn intersect(&self, other: &Self) -> Self {
        RangeF32::inclusive_exclusive(self.start.max(other.start), self.end.min(other.end))
    }

    pub fn cut_out(&self, other: &Self) -> (Self, Self) {
        (
            RangeF32::inclusive_exclusive(self.start, other.start.min(self.end)),
            RangeF32::inclusive_exclusive(other.end.max(self.start), self.end),
        )
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
