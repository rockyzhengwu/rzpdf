//! Bounded TrueType glyph locations. This validates ranges, not glyph programs.
use crate::error::{PdfError, PdfResult};

/// Declared glyf header values in font design units, not a verified outline.
/// Control-point bounds may exceed tight curve bounds and do not cover arbitrary
/// variable-font instances. Composite component bodies remain unparsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphHeader {
    pub number_of_contours: i16,
    pub bounds: [i16; 4],
}

impl GlyphHeader {
    pub fn is_composite(&self) -> bool {
        self.number_of_contours < 0
    }

    /// Parse only the ten-byte header, allowing trailing glyph body/padding.
    pub fn from_prefix(data: &[u8]) -> PdfResult<Self> {
        let header = data
            .get(..10)
            .ok_or_else(|| PdfError::FontError("Truncated glyf header".into()))?;
        let read = |offset| i16::from_be_bytes([header[offset], header[offset + 1]]);
        let bounds = [read(2), read(4), read(6), read(8)];
        if bounds[0] > bounds[2] || bounds[1] > bounds[3] {
            return Err(PdfError::FontError("Inverted glyf header bounds".into()));
        }
        Ok(Self {
            number_of_contours: read(0),
            bounds,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GlyphLocations<'a> {
    data: &'a [u8],
    glyf: &'a [u8],
    count: u16,
    stride: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphDependencySummary {
    /// Counts repeated occurrences, not only distinct GIDs.
    pub visited_glyphs: usize,
    /// Root is depth 1, including an empty or simple root.
    pub max_depth: usize,
}

impl<'a> GlyphLocations<'a> {
    pub(crate) fn glyph_count(&self) -> u16 {
        self.count
    }

    /// Validate component dependencies iteratively, without native recursion.
    /// Does not decode leaf points or perform transforms/point alignment/hinting.
    /// Repeated DAG occurrences are charged separately to bound graph expansion.
    pub fn validate_composite_dependencies(
        &self,
        gid: u16,
        limits: &crate::parser::options::Limits,
    ) -> PdfResult<GlyphDependencySummary> {
        use super::composite_glyf::{CompositeGlyph, GlyphComponent};
        struct Frame {
            gid: u16,
            components: Vec<GlyphComponent>,
            next: usize,
        }
        let mut stack: Vec<Frame> = Vec::new();
        let mut summary = GlyphDependencySummary {
            visited_glyphs: 0,
            max_depth: 0,
        };
        let mut charged_bytes = 0usize;
        let mut pending = Some(gid);
        loop {
            if let Some(gid) = pending.take() {
                if stack.iter().any(|frame| frame.gid == gid) {
                    return Err(PdfError::FontError(
                        "Cyclic composite glyph dependency".into(),
                    ));
                }
                let depth = stack.len().checked_add(1).ok_or_else(|| {
                    PdfError::ResourceLimit("Glyph dependency depth overflow".into())
                })?;
                if depth > limits.max_depth || summary.visited_glyphs >= limits.max_objects {
                    return Err(PdfError::ResourceLimit(
                        "Glyph dependency depth/occurrences".into(),
                    ));
                }
                let data = self.glyph_data(gid)?;
                let base_cost = data
                    .len()
                    .checked_add(std::mem::size_of::<Frame>())
                    .and_then(|bytes| charged_bytes.checked_add(bytes))
                    .ok_or_else(|| {
                        PdfError::ResourceLimit("Glyph dependency budget overflow".into())
                    })?;
                if base_cost > limits.max_decoded_bytes {
                    return Err(PdfError::ResourceLimit(
                        "Glyph dependency cumulative parse/storage".into(),
                    ));
                }
                let components =
                    if !data.is_empty() && GlyphHeader::from_prefix(data)?.is_composite() {
                        CompositeGlyph::try_new(data, self.count, limits)?.components
                    } else {
                        Vec::new()
                    };
                let cost = components
                    .len()
                    .checked_mul(std::mem::size_of::<GlyphComponent>())
                    .and_then(|bytes| bytes.checked_add(data.len()))
                    .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Frame>()))
                    .ok_or_else(|| {
                        PdfError::ResourceLimit("Glyph dependency storage overflow".into())
                    })?;
                charged_bytes = charged_bytes.checked_add(cost).ok_or_else(|| {
                    PdfError::ResourceLimit("Glyph dependency budget overflow".into())
                })?;
                if charged_bytes > limits.max_decoded_bytes {
                    return Err(PdfError::ResourceLimit(
                        "Glyph dependency cumulative parse/storage".into(),
                    ));
                }
                summary.visited_glyphs += 1;
                summary.max_depth = summary.max_depth.max(depth);
                stack.try_reserve(1).map_err(|_| {
                    PdfError::ResourceLimit("Glyph dependency stack allocation".into())
                })?;
                stack.push(Frame {
                    gid,
                    components,
                    next: 0,
                });
            }
            let Some(frame) = stack.last_mut() else {
                break;
            };
            if let Some(component) = frame.components.get(frame.next) {
                pending = Some(component.glyph_id);
                frame.next += 1;
            } else {
                stack.pop();
            }
        }
        Ok(summary)
    }

    /// Parse composite records in the bounded glyph range, without expanding them.
    pub fn composite_glyph(
        &self,
        gid: u16,
        limits: &crate::parser::options::Limits,
    ) -> PdfResult<Option<super::composite_glyf::CompositeGlyph<'a>>> {
        let data = self.glyph_data(gid)?;
        if data.is_empty() {
            Ok(None)
        } else {
            super::composite_glyf::CompositeGlyph::try_new(data, self.count, limits).map(Some)
        }
    }

    pub fn try_new(data: &'a [u8], glyf: &'a [u8], count: u16, format: i16) -> PdfResult<Self> {
        let stride = match format {
            0 => 2,
            1 => 4,
            _ => return Err(PdfError::FontError("Invalid indexToLocFormat".into())),
        };
        if count == 0 || data.len() < (usize::from(count) + 1) * stride {
            return Err(PdfError::FontError("Truncated or empty loca table".into()));
        }
        let result = Self {
            data,
            glyf,
            count,
            stride,
        };
        let mut previous = 0;
        for index in 0..=usize::from(count) {
            let offset = result.offset(index);
            if offset < previous || u64::from(offset) > glyf.len() as u64 {
                return Err(PdfError::FontError("Invalid loca glyph range".into()));
            }
            previous = offset;
        }
        Ok(result)
    }

    fn offset(&self, index: usize) -> u32 {
        let start = index * self.stride;
        if self.stride == 2 {
            u32::from(u16::from_be_bytes([self.data[start], self.data[start + 1]])) * 2
        } else {
            u32::from_be_bytes(self.data[start..start + 4].try_into().unwrap())
        }
    }

    /// Empty glyph descriptions are valid. Returned bytes remain unparsed.
    pub fn glyph_data(&self, gid: u16) -> PdfResult<&'a [u8]> {
        if gid >= self.count {
            return Err(PdfError::FontError("Glyph ID exceeds loca count".into()));
        }
        let index = usize::from(gid);
        Ok(&self.glyf[self.offset(index) as usize..self.offset(index + 1) as usize])
    }

    /// None denotes a zero-length loca range; a zero-contour header remains Some.
    /// Does not validate contour points, composite references or instructions.
    pub fn glyph_header(&self, gid: u16) -> PdfResult<Option<GlyphHeader>> {
        let data = self.glyph_data(gid)?;
        if data.is_empty() {
            Ok(None)
        } else {
            GlyphHeader::from_prefix(data).map(Some)
        }
    }

    /// Decode a simple glyph in this bounded range; composites reject explicitly.
    pub fn simple_glyph(
        &self,
        gid: u16,
        limits: &crate::parser::options::Limits,
    ) -> PdfResult<Option<super::glyf::SimpleGlyph<'a>>> {
        let data = self.glyph_data(gid)?;
        if data.is_empty() {
            Ok(None)
        } else {
            super::glyf::SimpleGlyph::try_new(data, limits).map(Some)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GlyphLocations;

    #[test]
    fn composite_dependencies_bound_depth_occurrences_and_reject_cycles() {
        use crate::{error::PdfError, parser::options::Limits};
        fn composite(children: &[u16]) -> Vec<u8> {
            let mut bytes: Vec<u8> = [-1_i16, 0, 0, 10, 10]
                .into_iter()
                .flat_map(i16::to_be_bytes)
                .collect();
            for (index, child) in children.iter().enumerate() {
                let flags: u16 = if index + 1 < children.len() { 34 } else { 2 };
                bytes.extend(flags.to_be_bytes());
                bytes.extend(child.to_be_bytes());
                bytes.extend([0, 0]);
            }
            bytes
        }
        fn check(
            glyphs: &[Vec<u8>],
            limits: &Limits,
        ) -> crate::error::PdfResult<super::GlyphDependencySummary> {
            let mut data = Vec::new();
            let mut offsets = 0_u32.to_be_bytes().to_vec();
            for glyph in glyphs {
                data.extend(glyph);
                offsets.extend((data.len() as u32).to_be_bytes());
            }
            GlyphLocations::try_new(&offsets, &data, glyphs.len() as u16, 1)?
                .validate_composite_dependencies(0, limits)
        }
        let limits = Limits::default();
        let dag = [composite(&[1, 1]), composite(&[2]), vec![0; 10]];
        let result = check(&dag, &limits).unwrap();
        assert_eq!(result.visited_glyphs, 5);
        assert_eq!(result.max_depth, 3);
        for glyphs in [
            vec![composite(&[0])],
            vec![composite(&[1]), composite(&[0])],
        ] {
            assert!(matches!(
                check(&glyphs, &limits),
                Err(PdfError::FontError(_))
            ));
        }
        for bounded in [
            Limits {
                max_depth: 2,
                ..limits.clone()
            },
            Limits {
                max_objects: 4,
                ..limits.clone()
            },
            Limits {
                max_decoded_bytes: 256,
                ..limits.clone()
            },
        ] {
            assert!(matches!(
                check(&dag, &bounded),
                Err(PdfError::ResourceLimit(_))
            ));
        }
        assert!(check(&[composite(&[1])], &limits).is_err());
        assert!(check(&[composite(&[1]), vec![0; 9]], &limits).is_err());
        let empty = check(&[vec![]], &limits).unwrap();
        assert_eq!((empty.visited_glyphs, empty.max_depth), (1, 1));
    }

    #[test]
    fn glyph_headers_preserve_signed_bounds_and_empty_range_distinction() {
        use super::GlyphHeader;
        for contours in [0_i16, 2, -1, -2] {
            let bytes: Vec<u8> = [contours, -200, -300, 700, 800]
                .into_iter()
                .flat_map(i16::to_be_bytes)
                .collect();
            let header = GlyphHeader::from_prefix(&bytes).unwrap();
            assert_eq!(header.bounds, [-200, -300, 700, 800]);
            assert_eq!(header.number_of_contours, contours);
            assert_eq!(header.is_composite(), contours < 0);
            for length in 0..10 {
                assert!(GlyphHeader::from_prefix(&bytes[..length]).is_err());
            }
            let loca = [0, 0, 0, 5, 0, 5];
            let locations = GlyphLocations::try_new(&loca, &bytes, 2, 0).unwrap();
            assert_eq!(locations.glyph_header(0).unwrap(), Some(header));
            assert_eq!(locations.glyph_header(1).unwrap(), None);
            assert!(locations.glyph_header(2).is_err());
        }
        for bounds in [[1_i16, 0, -1, 0], [0, 1, 0, -1]] {
            let bytes: Vec<u8> = std::iter::once(1_i16)
                .chain(bounds)
                .flat_map(i16::to_be_bytes)
                .collect();
            assert!(GlyphHeader::from_prefix(&bytes).is_err());
        }
        assert!(GlyphHeader::from_prefix(&[0; 10]).is_ok());
        assert!(
            GlyphLocations::try_new(&[0, 0, 0, 1], &[0; 2], 1, 0)
                .unwrap()
                .glyph_header(0)
                .is_err()
        );
    }

    #[test]
    fn short_and_long_locations_preserve_empty_glyphs() {
        let glyf = [1, 2, 3, 4, 5, 6];
        let short = [0, 0, 0, 1, 0, 1, 0, 3];
        let long: Vec<u8> = [0_u32, 2, 2, 6]
            .into_iter()
            .flat_map(u32::to_be_bytes)
            .collect();
        for (data, format) in [(&short[..], 0), (&long[..], 1)] {
            let locations = GlyphLocations::try_new(data, &glyf, 3, format).unwrap();
            assert_eq!(locations.glyph_data(0).unwrap(), &[1, 2]);
            assert_eq!(locations.glyph_data(1).unwrap(), &[]);
            assert_eq!(locations.glyph_data(2).unwrap(), &[3, 4, 5, 6]);
            assert!(locations.glyph_data(3).is_err());
        }
    }

    #[test]
    fn invalid_locations_are_rejected_before_lookup() {
        let data = [0, 0, 0, 1, 0, 3];
        for length in 0..data.len() {
            assert!(GlyphLocations::try_new(&data[..length], &[0; 6], 2, 0).is_err());
        }
        assert!(GlyphLocations::try_new(&data, &[0; 5], 2, 0).is_err());
        assert!(GlyphLocations::try_new(&[0, 0, 0, 3, 0, 2], &[0; 6], 2, 0).is_err());
        assert!(GlyphLocations::try_new(&data, &[0; 6], 0, 0).is_err());
        assert!(GlyphLocations::try_new(&data, &[0; 6], 2, 2).is_err());
        let overflow: Vec<u8> = [0_u32, u32::MAX]
            .into_iter()
            .flat_map(u32::to_be_bytes)
            .collect();
        assert!(GlyphLocations::try_new(&overflow, &[0; 6], 1, 1).is_err());
    }
}
