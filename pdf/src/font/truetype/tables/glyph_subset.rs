//! Unique subset dependency planning; no SFNT writing or outline expansion.
use super::{
    composite_glyf::{BudgetedCompositeGlyph, CompositeGlyph},
    loca::{GlyphHeader, GlyphLocations},
};
use crate::{
    budget::{MemoryLease, OperationBudget},
    error::{PdfError, PdfResult},
    parser::options::Limits,
};

pub(crate) struct GlyphClosure<'a> {
    bounds: [i16; 4],
    states: Vec<u8>,
    locations: GlyphLocations<'a>,
    _lease: MemoryLease,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closure_bounds_ignore_empty_outlines_and_include_cached_composites() {
        let with_bounds = |mut data: Vec<u8>, bounds: [i16; 4]| {
            for (slot, value) in data[2..10].chunks_exact_mut(2).zip(bounds) {
                slot.copy_from_slice(&value.to_be_bytes());
            }
            data
        };
        // Header-only simple fixtures exercise closure planning, not decoding.
        let mut simple = vec![0; 10];
        simple[..2].copy_from_slice(&1i16.to_be_bytes());
        let glyphs = [
            with_bounds(vec![0; 10], [-300, -300, 300, 300]),
            with_bounds(simple, [-4, -7, 12, 20]),
            with_bounds(composite(&[1]), [-9, -11, 25, 30]),
            with_bounds(composite(&[0]), [-500, -500, 500, 500]),
        ];
        let (offsets, data) = fixture(&glyphs);
        let locations = GlyphLocations::try_new(&offsets, &data, 4, 1).unwrap();
        let pool = OperationBudget::new(8192);
        for roots in [vec![1, 2, 3], vec![2, 1, 3]] {
            let closure = locations
                .subset_closure(roots, &Limits::default(), &pool)
                .unwrap();
            assert_eq!(closure.bounds(), [-9, -11, 25, 30]);
            drop(closure);
            assert_eq!(pool.live_bytes(), 0);
        }
        let closure = locations
            .subset_closure([3], &Limits::default(), &pool)
            .unwrap();
        assert_eq!(closure.bounds(), [0; 4]);
        drop(closure);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn rebuilt_tables_keep_components_and_empty_unselected_ranges() {
        let selected = composite(&[3]);
        let (offsets, data) = fixture(&[vec![], selected.clone(), vec![9; 40], vec![]]);
        let locations = GlyphLocations::try_new(&offsets, &data, 4, 1).unwrap();
        let pool = OperationBudget::new(8192);
        let closure = locations
            .subset_closure([1], &Limits::default(), &pool)
            .unwrap();
        let baseline = pool.live_bytes();
        let (glyf, loca) = closure
            .rebuild_glyf_loca(&Limits::default(), &pool)
            .unwrap();
        assert!(glyf.as_slice().len() < data.len());
        let rebuilt = GlyphLocations::try_new(loca.as_slice(), glyf.as_slice(), 4, 1).unwrap();
        assert!(rebuilt.glyph_data(2).unwrap().is_empty());
        assert_eq!(
            rebuilt
                .composite_glyph(1, &Limits::default())
                .unwrap()
                .unwrap()
                .components[0]
                .glyph_id,
            3
        );
        assert_eq!(&rebuilt.glyph_data(1).unwrap()[..selected.len()], &selected);
        drop(glyf);
        drop(loca);
        assert_eq!(pool.live_bytes(), baseline);
        drop(closure);
        assert_eq!(pool.live_bytes(), 0);
    }

    fn composite(children: &[u16]) -> Vec<u8> {
        let mut data = vec![255, 255, 0, 0, 0, 0, 0, 10, 0, 10];
        for (index, &child) in children.iter().enumerate() {
            let flags: u16 = if index + 1 < children.len() { 34 } else { 2 };
            data.extend(flags.to_be_bytes());
            data.extend(child.to_be_bytes());
            data.extend([0, 0]);
        }
        data
    }
    fn fixture(glyphs: &[Vec<u8>]) -> (Vec<u8>, Vec<u8>) {
        let mut offsets = 0u32.to_be_bytes().to_vec();
        let mut data = Vec::new();
        for glyph in glyphs {
            data.extend(glyph);
            offsets.extend((data.len() as u32).to_be_bytes());
        }
        (offsets, data)
    }
    #[test]
    fn closure_deduplicates_roots_and_components_and_holds_result_budget() {
        let (offsets, data) = fixture(&[vec![], composite(&[2, 2]), vec![], vec![]]);
        let locations = GlyphLocations::try_new(&offsets, &data, 4, 1).unwrap();
        let pool = OperationBudget::new(8192);
        let closure = locations
            .subset_closure([1, 1], &Limits::default(), &pool)
            .unwrap();
        assert_eq!(closure.glyph_ids().collect::<Vec<_>>(), [0, 1, 2]);
        assert_eq!(pool.live_bytes(), closure.states.capacity());
        drop(closure);
        assert_eq!(pool.live_bytes(), 0);
        let neighbor = pool.reserve(8191).unwrap();
        assert!(
            locations
                .subset_closure([1], &Limits::default(), &pool)
                .is_err()
        );
        assert_eq!(pool.live_bytes(), 8191);
        drop(neighbor);
        assert!(
            locations
                .subset_closure([1], &Limits::default(), &pool)
                .is_ok()
        );
        assert_eq!(pool.live_bytes(), 0);
    }
    #[test]
    fn closure_parse_peak_and_unwind_boundaries_release_all_scratch() {
        // Padding keeps the parse boundary above the independent component
        // storage limit, so the asserted refusal exercises cumulative work.
        let mut padded = composite(&[2]);
        padded.resize(128, 0);
        let (offsets, data) = fixture(&[vec![], padded, vec![]]);
        let locations = GlyphLocations::try_new(&offsets, &data, 3, 1).unwrap();
        let limits = Limits {
            max_decoded_bytes: 256,
            ..Limits::default()
        };
        let probe = OperationBudget::new(8192);
        let closure = locations.subset_closure([1], &limits, &probe).unwrap();
        let peak = probe.peak_bytes();
        drop(closure);
        assert_eq!(probe.live_bytes(), 0);
        let exact = OperationBudget::new(peak + 7);
        let neighbor = exact.reserve(7).unwrap();
        let closure = locations.subset_closure([1], &limits, &exact).unwrap();
        assert_eq!(exact.peak_bytes(), peak + 7);
        drop(closure);
        assert_eq!(exact.live_bytes(), 7);
        drop(neighbor);
        let short = OperationBudget::new(peak + 6);
        let neighbor = short.reserve(7).unwrap();
        assert!(matches!(locations.subset_closure([1], &limits, &short),
            Err(PdfError::ResourceLimit(message)) if message == "operation live bytes"));
        assert_eq!(short.live_bytes(), 7);
        drop(neighbor);
        drop(locations.subset_closure([1], &limits, &short).unwrap());
        assert_eq!(short.live_bytes(), 0);
        let too_small = Limits {
            max_decoded_bytes: 255,
            ..limits
        };
        assert!(matches!(locations.subset_closure([1], &too_small, &probe),
            Err(PdfError::ResourceLimit(message)) if message == "subset cumulative parse bytes"));
        assert_eq!(probe.live_bytes(), 0);
        let mut first = true;
        let roots = std::iter::from_fn(|| {
            if first {
                first = false;
                Some(1)
            } else {
                panic!("root iterator panic")
            }
        });
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = locations.subset_closure(roots, &limits, &probe);
        }));
        assert!(panic.is_err());
        assert_eq!(probe.live_bytes(), 0);
        drop(locations.subset_closure([1], &limits, &probe).unwrap());
        assert_eq!(probe.live_bytes(), 0);
    }

    #[test]
    fn closure_rejects_cycles_range_work_and_cached_long_path() {
        let (offsets, data) = fixture(&[
            vec![],
            composite(&[2]),
            composite(&[3]),
            vec![],
            composite(&[1]),
        ]);
        let locations = GlyphLocations::try_new(&offsets, &data, 5, 1).unwrap();
        let pool = OperationBudget::new(8192);
        let limits = Limits {
            max_depth: 3,
            ..Limits::default()
        };
        assert!(
            matches!(locations.subset_closure([1, 4], &limits, &pool), Err(PdfError::ResourceLimit(message)) if message == "subset dependency depth")
        );
        assert_eq!(pool.live_bytes(), 0);
        assert!(
            locations
                .subset_closure([5], &Limits::default(), &pool)
                .is_err()
        );
        let limits = Limits {
            max_objects: 2,
            ..Limits::default()
        };
        assert!(locations.subset_closure([1], &limits, &pool).is_err());
        let (offsets, data) = fixture(&[vec![], composite(&[2]), composite(&[1])]);
        let locations = GlyphLocations::try_new(&offsets, &data, 3, 1).unwrap();
        assert!(
            matches!(locations.subset_closure([1], &Limits::default(), &pool), Err(PdfError::FontError(message)) if message == "Cyclic subset glyph dependency")
        );
        assert_eq!(pool.live_bytes(), 0);
    }
}

impl GlyphClosure<'_> {
    /// Union of retained glyph headers with actual contour-bearing leaves.
    /// All-empty subsets use the zero rectangle; no outline decoding is implied.
    pub(crate) fn bounds(&self) -> [i16; 4] {
        self.bounds
    }
    /// Original GIDs remain stable; caller sets head.indexToLocFormat to 1.
    /// The original source is borrowed by this closure, preventing substitution.
    pub(crate) fn rebuild_glyf_loca(
        &self,
        limits: &Limits,
        budget: &OperationBudget,
    ) -> PdfResult<(crate::budget::BudgetedBytes, crate::budget::BudgetedBytes)> {
        let locations = &self.locations;
        let mut total = 0usize;
        for gid in self.glyph_ids() {
            total = locations
                .glyph_data(gid)?
                .len()
                .checked_add(3)
                .map(|length| length & !3)
                .and_then(|length| total.checked_add(length))
                .filter(|&length| {
                    length <= u32::MAX as usize
                        && length <= limits.max_decoded_bytes
                        && length <= limits.max_raw_stream_bytes
                })
                .ok_or_else(|| PdfError::ResourceLimit("Subset glyf bytes".into()))?;
        }
        let loca_bytes = (self.states.len() + 1)
            .checked_mul(4)
            .filter(|&length| {
                length <= limits.max_decoded_bytes && length <= limits.max_raw_stream_bytes
            })
            .ok_or_else(|| PdfError::ResourceLimit("Subset loca bytes".into()))?;
        let (mut glyf, glyf_lease) = table::<u8>(total, budget)?;
        let (mut loca, loca_lease) = table::<u8>(loca_bytes, budget)?;
        for (id, state) in self.states.iter().enumerate() {
            loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
            if *state == 2 {
                glyf.extend_from_slice(locations.glyph_data(id as u16)?);
                glyf.resize((glyf.len() + 3) & !3, 0);
            }
        }
        loca.extend_from_slice(&(glyf.len() as u32).to_be_bytes());
        Ok((
            crate::budget::BudgetedBytes::from_reserved_vec(glyf, glyf_lease)?,
            crate::budget::BudgetedBytes::from_reserved_vec(loca, loca_lease)?,
        ))
    }

    pub(crate) fn glyph_ids(&self) -> impl Iterator<Item = u16> + '_ {
        self.states
            .iter()
            .enumerate()
            .filter_map(|(id, state)| (*state == 2).then_some(id as u16))
    }
}

fn table<T>(count: usize, budget: &OperationBudget) -> PdfResult<(Vec<T>, MemoryLease)> {
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| PdfError::ResourceLimit("subset table overflow".into()))?;
    let mut lease = budget.reserve(bytes)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| PdfError::ResourceLimit("subset table allocation".into()))?;
    lease.resize(
        values
            .capacity()
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(|| PdfError::ResourceLimit("subset table overflow".into()))?,
    )?;
    Ok((values, lease))
}

struct Frame<'a> {
    gid: u16,
    glyph: Option<BudgetedCompositeGlyph<'a>>,
    next: usize,
    height: usize,
    header: Option<GlyphHeader>,
    has_contours: bool,
}

impl<'a> GlyphLocations<'a> {
    /// Includes GID zero. Input font bytes remain borrowed and caller-accounted.
    /// Work charges roots, unique glyphs and every component edge; double-pass
    /// composite parsing charges both input traversals. No native recursion.
    pub(crate) fn subset_closure(
        &self,
        roots: impl IntoIterator<Item = u16>,
        limits: &Limits,
        budget: &OperationBudget,
    ) -> PdfResult<GlyphClosure<'a>> {
        let count = usize::from(self.glyph_count());
        if count == 0 || limits.max_depth == 0 {
            return Err(PdfError::ResourceLimit("subset glyph count/depth".into()));
        }
        let (mut states, lease) = table(count, budget)?;
        states.resize(count, 0u8);
        let (mut heights, _heights_lease) = table(count, budget)?;
        heights.resize(count, 0usize);
        let (mut contours, _contours_lease) = table(count, budget)?;
        contours.resize(count, false);
        let mut bounds: Option<[i16; 4]> = None;
        let stack_limit = count.min(limits.max_depth).min(limits.max_objects);
        let (mut stack, _stack_lease) = table::<Frame<'a>>(stack_limit, budget)?;
        let mut work = 0usize;
        let mut parsed = 0usize;
        let mut charge = || -> PdfResult<()> {
            work = work
                .checked_add(1)
                .filter(|&value| value <= limits.max_objects)
                .ok_or_else(|| PdfError::ResourceLimit("subset dependency work".into()))?;
            Ok(())
        };
        for root in std::iter::once(0).chain(roots) {
            charge()?;
            let mut pending = Some(root);
            loop {
                if let Some(gid) = pending.take() {
                    let index = usize::from(gid);
                    let state = *states.get(index).ok_or_else(|| {
                        PdfError::FontError("Subset glyph ID out of range".into())
                    })?;
                    if state == 1 {
                        return Err(PdfError::FontError("Cyclic subset glyph dependency".into()));
                    }
                    if state == 2 {
                        if stack
                            .len()
                            .checked_add(heights[index])
                            .is_none_or(|depth| depth > limits.max_depth)
                        {
                            return Err(PdfError::ResourceLimit("subset dependency depth".into()));
                        }
                        if let Some(parent) = stack.last_mut() {
                            parent.height = parent.height.max(heights[index] + 1);
                            parent.has_contours |= contours[index];
                        }
                    } else {
                        charge()?;
                        if stack.len() >= stack_limit {
                            return Err(PdfError::ResourceLimit("subset dependency depth".into()));
                        }
                        let data = self.glyph_data(gid)?;
                        let header = if data.is_empty() {
                            None
                        } else {
                            Some(GlyphHeader::from_prefix(data)?)
                        };
                        let composite = header.is_some_and(|header| header.is_composite());
                        let bytes = data.len().checked_mul(if composite { 2 } else { 1 });
                        parsed = bytes
                            .and_then(|bytes| parsed.checked_add(bytes))
                            .filter(|&value| value <= limits.max_decoded_bytes)
                            .ok_or_else(|| {
                                PdfError::ResourceLimit("subset cumulative parse bytes".into())
                            })?;
                        let glyph = if composite {
                            Some(CompositeGlyph::try_new_with_budget(
                                data,
                                self.glyph_count(),
                                limits,
                                budget,
                            )?)
                        } else {
                            None
                        };
                        states[index] = 1;
                        stack.push(Frame {
                            gid,
                            glyph,
                            next: 0,
                            height: 1,
                            header,
                            has_contours: header
                                .is_some_and(|header| header.number_of_contours > 0),
                        });
                    }
                }
                let Some(frame) = stack.last_mut() else { break };
                let child = frame
                    .glyph
                    .as_ref()
                    .and_then(|glyph| glyph.glyph().components.get(frame.next))
                    .map(|component| component.glyph_id);
                if let Some(child) = child {
                    charge()?;
                    frame.next += 1;
                    pending = Some(child);
                } else {
                    let frame = stack.pop().unwrap();
                    let index = usize::from(frame.gid);
                    heights[index] = frame.height;
                    contours[index] = frame.has_contours;
                    if frame.has_contours {
                        let current = frame.header.unwrap().bounds;
                        bounds = Some(match bounds {
                            None => current,
                            Some(previous) => [
                                previous[0].min(current[0]),
                                previous[1].min(current[1]),
                                previous[2].max(current[2]),
                                previous[3].max(current[3]),
                            ],
                        });
                    }
                    states[index] = 2;
                    if let Some(parent) = stack.last_mut() {
                        parent.height = parent.height.max(frame.height + 1);
                        parent.has_contours |= frame.has_contours;
                    }
                }
            }
        }
        Ok(GlyphClosure {
            bounds: bounds.unwrap_or([0; 4]),
            states,
            locations: *self,
            _lease: lease,
        })
    }
}
