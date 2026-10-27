//! Coalesce identical adjacent alpha row runs without changing a single sample.
//! Pixels are A8 or native N32, whose alpha occupies its last byte.
//! Emitted rectangles never overlap, so their submission order is immaterial.
#[derive(Clone, Copy, Default)]
pub(super) struct CoverageRect {
    pub left: u8,
    pub right: u8,
    pub top: u8,
    pub bottom: u8,
    pub coverage: u8,
}

pub(super) fn rectangles<const PIXEL_BYTES: usize>(
    data: &[u8],
    stride: usize,
    bounds: (usize, usize, usize, usize),
    mut emit: impl FnMut(CoverageRect),
) {
    let (left, top, right, bottom) = bounds;
    let mut active = [CoverageRect::default(); 64];
    let mut count = 0;
    for row in top..bottom {
        let values = &data[row * stride + left * PIXEL_BYTES..row * stride + right * PIXEL_BYTES];
        let mut next = [CoverageRect::default(); 64];
        let (mut old, mut used, mut column) = (0, 0, 0);
        while column < right - left {
            let start = column;
            let coverage = values[column * PIXEL_BYTES + PIXEL_BYTES - 1];
            column += 1;
            while column < right - left
                && values[column * PIXEL_BYTES + PIXEL_BYTES - 1] == coverage
            {
                column += 1;
            }
            if coverage == 0 {
                continue;
            }
            let mut rect = CoverageRect {
                left: (left + start) as u8,
                right: (left + column) as u8,
                top: row as u8,
                bottom: (row + 1) as u8,
                coverage,
            };
            while old < count && active[old].left < rect.left {
                emit(active[old]);
                old += 1;
            }
            if old < count && active[old].left == rect.left {
                let previous = active[old];
                old += 1;
                if previous.right == rect.right
                    && previous.coverage == coverage
                    && previous.bottom == rect.top
                {
                    rect.top = previous.top;
                } else {
                    emit(previous);
                }
            }
            next[used] = rect;
            used += 1;
        }
        for rect in &active[old..count] {
            emit(*rect);
        }
        active = next;
        count = used;
    }
    for rect in &active[..count] {
        emit(*rect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn padded_n32_rows_emit_the_same_rectangles_as_a8() {
        let mut a8 = vec![0; 68 * 68];
        let stride = 68 * 4 + 12;
        let mut n32 = vec![91; stride * 68];
        for row in 0..68 {
            for column in 0..68 {
                let alpha = ((column * 19 + row / 3 * 31) % 256) as u8;
                a8[row * 68 + column] = alpha;
                n32[row * stride + column * 4 + 3] = alpha;
            }
        }
        let mut expected = Vec::new();
        let mut actual = Vec::new();
        rectangles::<1>(&a8, 68, (3, 5, 65, 64), |r| {
            expected.push((r.left, r.top, r.right, r.bottom, r.coverage))
        });
        rectangles::<4>(&n32, stride, (3, 5, 65, 64), |r| {
            actual.push((r.left, r.top, r.right, r.bottom, r.coverage))
        });
        assert_eq!(actual, expected);
    }
    #[test]
    fn runs_reconstruct_exactly_once_and_full_block_is_one_rectangle() {
        for pattern in 0..4 {
            let mut input = vec![0u8; 68 * 68];
            for y in 2..66 {
                for x in 2..66 {
                    input[y * 68 + x] = match pattern {
                        0 => 128,
                        1 => {
                            if x % 3 == 0 {
                                17
                            } else {
                                0
                            }
                        }
                        2 => {
                            if x + y < 68 {
                                (x * 3) as u8
                            } else {
                                0
                            }
                        }
                        _ => ((x / 4 + y / 4) % 3 * 97) as u8,
                    };
                }
            }
            let mut output = vec![0u8; 68 * 68];
            let mut count = 0;
            rectangles::<1>(&input, 68, (2, 2, 66, 66), |rect| {
                count += 1;
                for y in rect.top..rect.bottom {
                    for x in rect.left..rect.right {
                        let slot = &mut output[y as usize * 68 + x as usize];
                        assert_eq!(*slot, 0, "rectangles must not overlap");
                        *slot = rect.coverage;
                    }
                }
            });
            assert_eq!(input, output);
            if pattern == 0 {
                assert_eq!(count, 1);
            }
        }
    }
}
