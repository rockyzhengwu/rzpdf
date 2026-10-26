//! Coalesce identical adjacent A8 row runs without changing a single sample.
//! Emitted rectangles never overlap, so their submission order is immaterial.
#[derive(Clone, Copy, Default)]
pub(super) struct CoverageRect {
    pub left: u8,
    pub right: u8,
    pub top: u8,
    pub bottom: u8,
    pub coverage: u8,
}

pub(super) fn rectangles(
    data: &[u8],
    stride: usize,
    bounds: (usize, usize, usize, usize),
    mut emit: impl FnMut(CoverageRect),
) {
    let (left, top, right, bottom) = bounds;
    let mut active = [CoverageRect::default(); 64];
    let mut count = 0;
    for row in top..bottom {
        let values = &data[row * stride + left..row * stride + right];
        let mut next = [CoverageRect::default(); 64];
        let (mut old, mut used, mut column) = (0, 0, 0);
        while column < values.len() {
            let start = column;
            let coverage = values[column];
            column += 1;
            while column < values.len() && values[column] == coverage {
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
            rectangles(&input, 68, (2, 2, 66, 66), |rect| {
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
