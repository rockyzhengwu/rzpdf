//! Composite glyph records only; no recursion, point alignment or hinting.
use super::loca::GlyphHeader;
use crate::{
    error::{PdfError, PdfResult},
    parser::options::Limits,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentArguments {
    Offset([i16; 2]),
    Points([u16; 2]),
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphComponent {
    pub flags: u16,
    pub glyph_id: u16,
    pub arguments: ComponentArguments,
    /// x'=a*x+c*y, y'=b*x+d*y; F2DOT14 decoded exactly to f64.
    pub transform: [f64; 4],
}
#[derive(Debug)]
pub struct CompositeGlyph<'a> {
    pub header: GlyphHeader,
    pub components: Vec<GlyphComponent>,
    pub instructions: &'a [u8],
}

/// Collection capacity stays charged until the owner is dropped.
#[derive(Debug)]
pub(crate) struct BudgetedCompositeGlyph<'a> {
    glyph: CompositeGlyph<'a>,
    _lease: crate::budget::MemoryLease,
}

impl<'a> BudgetedCompositeGlyph<'a> {
    pub(crate) fn glyph(&self) -> &CompositeGlyph<'a> {
        &self.glyph
    }
}

impl<'a> CompositeGlyph<'a> {
    pub(crate) fn try_new_with_budget(
        data: &'a [u8],
        glyph_count: u16,
        limits: &Limits,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<BudgetedCompositeGlyph<'a>> {
        let mut count = 0usize;
        // Validate all records/instructions before allocating. The second
        // traversal sees the same immutable bytes; no growth is necessary.
        Self::visit_components(data, glyph_count, limits, |_| {
            count += 1;
            Ok(())
        })?;
        let bytes = count
            .checked_mul(std::mem::size_of::<GlyphComponent>())
            .ok_or_else(|| PdfError::ResourceLimit("Composite capacity overflow".into()))?;
        let mut lease = budget.reserve(bytes)?;
        let mut components = Vec::new();
        components
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("Composite allocation".into()))?;
        let capacity_bytes = components
            .capacity()
            .checked_mul(std::mem::size_of::<GlyphComponent>())
            .ok_or_else(|| PdfError::ResourceLimit("Composite capacity overflow".into()))?;
        lease.resize(capacity_bytes)?;
        let (header, instructions) =
            Self::visit_components(data, glyph_count, limits, |component| {
                components.push(component);
                Ok(())
            })?;
        Ok(BudgetedCompositeGlyph {
            glyph: Self {
                header,
                components,
                instructions,
            },
            _lease: lease,
        })
    }

    pub fn try_new(data: &'a [u8], glyph_count: u16, limits: &Limits) -> PdfResult<Self> {
        let mut components = Vec::new();
        let (header, instructions) =
            Self::visit_components(data, glyph_count, limits, |component| {
                components
                    .try_reserve(1)
                    .map_err(|_| PdfError::ResourceLimit("Composite allocation".into()))?;
                components.push(component);
                Ok(())
            })?;
        Ok(Self {
            header,
            components,
            instructions,
        })
    }

    /// Parse records without allocating a component collection. Callbacks may
    /// fail to stop traversal; no partial parse result is returned. Callers
    /// retaining callback output must discard it if later validation fails.
    pub(crate) fn visit_components(
        data: &'a [u8],
        glyph_count: u16,
        limits: &Limits,
        mut visit: impl FnMut(GlyphComponent) -> PdfResult<()>,
    ) -> PdfResult<(GlyphHeader, &'a [u8])> {
        let header = GlyphHeader::from_prefix(data)?;
        if !header.is_composite() {
            return Err(PdfError::FontError("Expected composite glyph".into()));
        }
        if data.len() > limits.max_decoded_bytes {
            return Err(PdfError::ResourceLimit("Composite glyph input".into()));
        }
        let mut offset = 10usize;
        let take = |offset: &mut usize, size: usize| -> PdfResult<&'a [u8]> {
            let end = offset
                .checked_add(size)
                .ok_or_else(|| PdfError::FontError("Composite offset overflow".into()))?;
            let bytes = data
                .get(*offset..end)
                .ok_or_else(|| PdfError::FontError("Truncated composite glyph".into()))?;
            *offset = end;
            Ok(bytes)
        };
        let word = |offset: &mut usize| -> PdfResult<u16> {
            let b = take(offset, 2)?;
            Ok(u16::from_be_bytes([b[0], b[1]]))
        };
        let mut component_count = 0usize;
        let mut has_instructions = false;
        loop {
            let flags = word(&mut offset)?;
            let glyph_id = word(&mut offset)?;
            if glyph_id >= glyph_count
                || flags & 0xe010 != 0
                || (flags & 0x00c8).count_ones() > 1
                || flags & 0x1800 == 0x1800
            {
                return Err(PdfError::FontError(
                    "Invalid composite flags or glyph ID".into(),
                ));
            }
            if component_count == 0 && flags & 2 == 0 {
                return Err(PdfError::FontError(
                    "First composite component needs XY offsets".into(),
                ));
            }
            let raw = if flags & 1 != 0 {
                [word(&mut offset)?, word(&mut offset)?]
            } else {
                let b = take(&mut offset, 2)?;
                [u16::from(b[0]), u16::from(b[1])]
            };
            let arguments = if flags & 2 != 0 {
                ComponentArguments::Offset(if flags & 1 != 0 {
                    [raw[0] as i16, raw[1] as i16]
                } else {
                    [raw[0] as u8 as i8 as i16, raw[1] as u8 as i8 as i16]
                })
            } else {
                ComponentArguments::Points(raw)
            };
            let fixed = |offset: &mut usize| -> PdfResult<f64> {
                Ok(f64::from(word(offset)? as i16) / 16384.)
            };
            let transform = if flags & 8 != 0 {
                let scale = fixed(&mut offset)?;
                [scale, 0., 0., scale]
            } else if flags & 64 != 0 {
                [fixed(&mut offset)?, 0., 0., fixed(&mut offset)?]
            } else if flags & 128 != 0 {
                [
                    fixed(&mut offset)?,
                    fixed(&mut offset)?,
                    fixed(&mut offset)?,
                    fixed(&mut offset)?,
                ]
            } else {
                [1., 0., 0., 1.]
            };
            let count = component_count
                .checked_add(1)
                .ok_or_else(|| PdfError::ResourceLimit("Composite count overflow".into()))?;
            if count > limits.max_objects
                || count
                    .checked_mul(std::mem::size_of::<GlyphComponent>())
                    .is_none_or(|bytes| bytes > limits.max_decoded_bytes)
            {
                return Err(PdfError::ResourceLimit(
                    "Composite components/storage".into(),
                ));
            }
            visit(GlyphComponent {
                flags,
                glyph_id,
                arguments,
                transform,
            })?;
            component_count = count;
            has_instructions |= flags & 256 != 0;
            if flags & 32 == 0 {
                break;
            }
        }
        let instructions = if has_instructions {
            let length = usize::from(word(&mut offset)?);
            take(&mut offset, length)?
        } else {
            &data[offset..offset]
        };
        Ok((header, instructions))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn budgeted_components_hold_capacity_and_release_for_retry() {
        let data = [255, 255, 0, 0, 0, 0, 0, 10, 0, 10, 0, 2, 0, 1, 0, 0];
        let limits = Limits::default();
        let probe = crate::budget::OperationBudget::new(4096);
        let glyph = CompositeGlyph::try_new_with_budget(&data, 2, &limits, &probe).unwrap();
        assert_eq!(glyph.glyph().components[0].glyph_id, 1);
        let bytes = probe.live_bytes();
        assert_eq!(
            bytes,
            glyph.glyph().components.capacity() * std::mem::size_of::<GlyphComponent>()
        );
        drop(glyph);
        assert_eq!(probe.live_bytes(), 0);
        let pool = crate::budget::OperationBudget::new(bytes);
        let neighbor = pool.reserve(1).unwrap();
        assert!(
            matches!(CompositeGlyph::try_new_with_budget(&data, 2, &limits, &pool),
            Err(PdfError::ResourceLimit(message)) if message == "operation live bytes")
        );
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        let glyph = CompositeGlyph::try_new_with_budget(&data, 2, &limits, &pool).unwrap();
        drop(glyph);
        assert_eq!(pool.live_bytes(), 0);
        assert!(CompositeGlyph::try_new_with_budget(&data[..15], 2, &limits, &pool).is_err());
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn borrowed_visit_matches_collection_and_propagates_callback_failure() {
        let mut data = vec![255, 255, 0, 0, 0, 0, 0, 10, 0, 10];
        // Instruction flag on the first, not last, component.
        for value in [0x123_u16, 1, 0, 0, 3, 2, 0, 0, 1] {
            data.extend_from_slice(&value.to_be_bytes());
        }
        data.push(0xaa);
        let limits = Limits::default();
        let collected = CompositeGlyph::try_new(&data, 3, &limits).unwrap();
        let mut visited = Vec::new();
        let (header, instructions) =
            CompositeGlyph::visit_components(&data, 3, &limits, |component| {
                visited.push(component);
                Ok(())
            })
            .unwrap();
        assert_eq!(header, collected.header);
        assert_eq!(visited, collected.components);
        assert_eq!(instructions, &[0xaa]);
        let mut calls = 0;
        let error = CompositeGlyph::visit_components(&data, 3, &limits, |_| {
            calls += 1;
            Err(PdfError::ResourceLimit("visitor quota".into()))
        })
        .unwrap_err();
        assert!(matches!(error, PdfError::ResourceLimit(message) if message == "visitor quota"));
        assert_eq!(calls, 1);
        assert!(
            CompositeGlyph::visit_components(&data[..data.len() - 1], 3, &limits, |_| Ok(()))
                .is_err()
        );
    }

    #[test]
    fn component_word_offsets_and_full_signed_matrix_are_exact() {
        let mut data = vec![255, 255, 0, 0, 0, 0, 0, 10, 0, 10];
        for value in [
            0x83_u16,
            0,
            (-300_i16) as u16,
            200,
            (-16384_i16) as u16,
            8192,
            4096,
            16384,
        ] {
            data.extend_from_slice(&value.to_be_bytes());
        }
        let component = CompositeGlyph::try_new(&data, 1, &Limits::default())
            .unwrap()
            .components[0];
        assert_eq!(component.arguments, ComponentArguments::Offset([-300, 200]));
        assert_eq!(component.transform, [-1., 0.5, 0.25, 1.]);
        for flags in [0x0093_u16, 0x00c3, 0x1883, 0x0081] {
            let mut bad = data.clone();
            bad[10..12].copy_from_slice(&flags.to_be_bytes());
            assert!(CompositeGlyph::try_new(&bad, 1, &Limits::default()).is_err());
        }
        let mut padded = data.clone();
        padded.extend_from_slice(&[0, 0]);
        let loca = [0, 0, 0, 14];
        let locations = super::super::loca::GlyphLocations::try_new(&loca, &padded, 1, 0).unwrap();
        assert_eq!(
            locations
                .composite_glyph(0, &Limits::default())
                .unwrap()
                .unwrap()
                .components[0],
            component
        );
    }
    #[test]
    fn composite_negative_contours_and_flag_boundaries_are_explicit() {
        let mut data = [255, 255, 0, 0, 0, 0, 0, 10, 0, 10, 0, 2, 0, 1, 0, 0];
        for contours in [-1i16, -2, i16::MIN] {
            data[..2].copy_from_slice(&contours.to_be_bytes());
            let glyph = CompositeGlyph::try_new(&data, 2, &Limits::default()).unwrap();
            assert_eq!(glyph.components.len(), 1);
            assert_eq!(glyph.components[0].glyph_id, 1);
            assert_eq!(
                glyph.components[0].arguments,
                ComponentArguments::Offset([0, 0])
            );
            assert!(glyph.instructions.is_empty());
        }
        for flags in [
            0x0012u16, 0x2002, 0x4002, 0x8002, 0x004a, 0x008a, 0x00c2, 0x1802,
        ] {
            data[10..12].copy_from_slice(&flags.to_be_bytes());
            assert!(
                matches!(CompositeGlyph::try_new(&data, 2, &Limits::default()),
                Err(PdfError::FontError(message)) if message == "Invalid composite flags or glyph ID"),
                "flags {flags:#06x}"
            );
        }
        data[10..12].copy_from_slice(&0u16.to_be_bytes());
        assert!(
            matches!(CompositeGlyph::try_new(&data, 2, &Limits::default()),
            Err(PdfError::FontError(message)) if message == "First composite component needs XY offsets")
        );
        data[10..12].copy_from_slice(&2u16.to_be_bytes());
        for contours in [0i16, 1, i16::MAX] {
            data[..2].copy_from_slice(&contours.to_be_bytes());
            assert!(
                matches!(CompositeGlyph::try_new(&data, 2, &Limits::default()),
                Err(PdfError::FontError(message)) if message == "Expected composite glyph")
            );
        }
    }

    #[test]
    fn composite_records_preserve_arguments_transforms_and_early_instruction_flag() {
        let data = [
            255, 255, 0, 0, 0, 0, 0, 10, 0, 10, 1, 42, 0, 1, 255, 2, 32, 0, 0, 1, 0, 2, 1, 44, 0,
            7, 0, 2, 12, 34,
        ];
        let glyph = CompositeGlyph::try_new(&data, 3, &Limits::default()).unwrap();
        assert_eq!(glyph.components.len(), 2);
        assert_eq!(
            glyph.components[0].arguments,
            ComponentArguments::Offset([-1, 2])
        );
        assert_eq!(glyph.components[0].transform, [0.5, 0., 0., 0.5]);
        assert_eq!(
            glyph.components[1].arguments,
            ComponentArguments::Points([300, 7])
        );
        assert_eq!(glyph.instructions, &[12, 34]);
        for length in 0..data.len() {
            assert!(CompositeGlyph::try_new(&data[..length], 3, &Limits::default()).is_err());
        }
        assert!(CompositeGlyph::try_new(&data, 1, &Limits::default()).is_err());
        assert!(
            CompositeGlyph::try_new(
                &data,
                3,
                &Limits {
                    max_objects: 1,
                    ..Limits::default()
                }
            )
            .is_err()
        );
        let mut bad = data;
        bad[11] |= 64;
        assert!(CompositeGlyph::try_new(&bad, 3, &Limits::default()).is_err());
    }
}
