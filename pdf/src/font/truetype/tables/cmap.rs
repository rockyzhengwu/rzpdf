use crate::error::{PdfError, PdfResult};
use crate::font::truetype::reader::TrueTypeReader;

fn reserved_cmap_vec<T>(count: usize) -> PdfResult<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| PdfError::ResourceLimit("TrueType cmap array allocation".into()))?;
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format12(groups: &[(u32, u32, u32)]) -> Vec<u8> {
        let mut data = vec![0, 12, 0, 0];
        data.extend_from_slice(&(16u32 + groups.len() as u32 * 12).to_be_bytes());
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&(groups.len() as u32).to_be_bytes());
        for (start, end, gid) in groups {
            for value in [start, end, gid] {
                data.extend_from_slice(&value.to_be_bytes());
            }
        }
        data
    }

    #[test]
    fn format12_maps_bmp_and_supplementary_codes_without_truncation() {
        let data = format12(&[(65, 66, 2), (0x20000, 0x20002, 100)]);
        let cmap = TCmap::try_new(3, 10, 0, &mut TrueTypeReader::new(&data)).unwrap();
        for (code, gid) in [
            (64, 0),
            (65, 2),
            (66, 3),
            (67, 0),
            (0x1ffff, 0),
            (0x20000, 100),
            (0x20002, 102),
            (0x20003, 0),
        ] {
            assert_eq!(cmap.get_gid_u32(code), gid);
        }
        assert_eq!(cmap.get_gid(65), 2);
        for end in 0..data.len() {
            assert!(TCmap::try_new(3, 10, 0, &mut TrueTypeReader::new(&data[..end])).is_err());
        }
    }

    #[test]
    fn format12_rejects_bad_lengths_ranges_and_glyph_overflow() {
        for groups in [
            vec![(10, 9, 1)],
            vec![(1, 5, 1), (5, 6, 20)],
            vec![(1, 2, u32::MAX)],
        ] {
            let data = format12(&groups);
            assert!(TCmap::try_new(3, 10, 0, &mut TrueTypeReader::new(&data)).is_err());
        }
        let mut data = format12(&[(1, 2, 3)]);
        data[12..16].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(TCmap::try_new(3, 10, 0, &mut TrueTypeReader::new(&data)).is_err());
        let empty = format12(&[]);
        assert_eq!(
            TCmap::try_new(0, 4, 0, &mut TrueTypeReader::new(&empty))
                .unwrap()
                .get_gid_u32(0x20000),
            0
        );
    }
    #[test]
    fn format4_binary_lookup_matches_all_bmp_codes() {
        let words: [u16; 20] = [
            4, 40, 0, 6, 4, 1, 2, 65, 70, 65535, 0, 60, 67, 65535, 1, 65534, 1, 0, 0, 0,
        ];
        let data: Vec<u8> = words.iter().flat_map(|word| word.to_be_bytes()).collect();
        let cmap = TCmap::try_new(3, 1, 0, &mut TrueTypeReader::new(&data)).unwrap();
        for code in 0..=u16::MAX {
            let expected = match code {
                60..=65 => code + 1,
                67..=70 => code - 2,
                _ => 0,
            };
            assert_eq!(cmap.get_gid(code), expected, "code {code}");
        }
    }

    #[test]
    fn format4_rejects_overlapping_ranges_and_incomplete_sentinel() {
        let mut words: [u16; 20] = [
            4, 40, 0, 6, 4, 1, 2, 65, 70, 65535, 0, 65, 65, 65535, 1, 1, 1, 0, 0, 0,
        ];
        let parse = |words: &[u16]| {
            let data: Vec<u8> = words.iter().flat_map(|word| word.to_be_bytes()).collect();
            TCmap::try_new(3, 1, 0, &mut TrueTypeReader::new(&data))
        };
        assert!(parse(&words).is_err());
        words[12] = 66;
        let cmap = parse(&words).unwrap();
        assert_eq!(cmap.get_gid(65), 66);
        assert_eq!(cmap.get_gid(66), 67);
        assert_eq!(cmap.get_gid(70), 71);
        assert_eq!(cmap.get_gid(71), 0);
        assert_eq!(cmap.get_gid(65535), 0);
        words[13] = 65534;
        assert!(parse(&words).is_err());
    }

    #[test]
    fn direct_format4_checks_bounds_before_consuming_input() {
        let words: [u16; 14] = [4, 4, 1, 0, 65, 65535, 0, 65, 65535, 1, 1, 4, 0, 5];
        let data: Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
        assert!(Format4Cmap::try_new(&mut TrueTypeReader::new(&data), 34).is_ok());
        for end in 0..data.len() {
            let mut reader = TrueTypeReader::new(&data[..end]);
            assert!(Format4Cmap::try_new(&mut reader, 34).is_err());
            assert_eq!(reader.offset(), 0);
        }
        assert!(matches!(
            reserved_cmap_vec::<u16>(usize::MAX),
            Err(PdfError::ResourceLimit(_))
        ));
    }
    #[test]
    fn format4_lengths_offsets_and_delta_are_checked() {
        let words: [u16; 17] = [4, 34, 0, 4, 4, 1, 0, 65, 65535, 0, 65, 65535, 1, 1, 4, 0, 5];
        let data: Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
        let cmap = TCmap::try_new(3, 1, 0, &mut TrueTypeReader::new(&data)).unwrap();
        assert_eq!(cmap.get_gid(65), 6);
        assert_eq!(cmap.get_gid(66), 0);
        for end in 0..data.len() {
            assert!(TCmap::try_new(3, 1, 0, &mut TrueTypeReader::new(&data[..end])).is_err());
        }
        let mut bad = data.clone();
        bad[28..30].copy_from_slice(&2u16.to_be_bytes());
        let cmap = TCmap::try_new(3, 1, 0, &mut TrueTypeReader::new(&bad)).unwrap();
        assert_eq!(cmap.get_gid(65), 0);
        let mut reader = TrueTypeReader::new(&data);
        reader.reset_offset(usize::MAX);
        assert!(reader.read_be_u16().is_err());
        assert!(reader.read_be_u32().is_err());
        assert!(reader.read_tag().is_err());
        assert_eq!(
            TrueTypeReader::new(&[0x12, 0x34]).read_be_u16().unwrap(),
            0x1234
        );
    }
}

#[derive(Debug, Clone)]
pub struct Format0Cmap {
    gids: [u8; 256],
}

impl Format0Cmap {
    pub fn try_new(reader: &mut TrueTypeReader) -> PdfResult<Self> {
        let mut gids = [0; 256];
        for gid in &mut gids {
            *gid = reader.read_u8()?;
        }
        Ok(Format0Cmap { gids })
    }
    pub fn get_gid(&self, c: u8) -> u8 {
        self.gids[c as usize]
    }
}

#[cfg(test)]
mod format0_tests {
    use super::*;

    #[test]
    fn fixed_glyph_array_handles_all_codes_and_truncations() {
        let data: Vec<u8> = (0..=255).collect();
        let cmap = Format0Cmap::try_new(&mut TrueTypeReader::new(&data)).unwrap();
        for code in 0..=255 {
            assert_eq!(cmap.get_gid(code), code);
        }
        for end in 0..256 {
            assert!(Format0Cmap::try_new(&mut TrueTypeReader::new(&data[..end])).is_err());
        }
    }
}

#[derive(Debug, Clone)]
pub struct Format4Cmap {
    end_code: Vec<u16>,
    start_code: Vec<u16>,
    id_delta: Vec<i16>,
    id_range_offset: Vec<u16>,
    glyph_id_array: Vec<u16>,
}
impl Format4Cmap {
    pub fn try_new(reader: &mut TrueTypeReader, length: u16) -> PdfResult<Self> {
        if length < 16 || usize::from(length) - 6 > reader.remaining() {
            return Err(PdfError::FontError(
                "Truncated TrueType cmap format 4".into(),
            ));
        }
        let start = reader.offset();
        let seg_count_x2 = reader.read_be_u16()?;
        if seg_count_x2 == 0
            || seg_count_x2 % 2 != 0
            || usize::from(length) < 16 + usize::from(seg_count_x2) * 4
        {
            return Err(PdfError::FontError(
                "Invalid TrueType cmap format 4 segment count/length".into(),
            ));
        }
        let _search_range = reader.read_be_u16()?;
        let _entry_selector = reader.read_be_u16()?;
        let _range_shift = reader.read_be_u16()?;
        let count = usize::from(seg_count_x2 / 2);
        let mut end_code = reserved_cmap_vec(count)?;
        for _i in 0..seg_count_x2 / 2 {
            let v = reader.read_be_u16()?;
            end_code.push(v)
        }
        let reserved_pad = reader.read_be_u16()?;
        let mut start_code = reserved_cmap_vec(count)?;
        for _i in 0..seg_count_x2 / 2 {
            let v = reader.read_be_u16()?;
            start_code.push(v);
        }
        let mut id_delta = reserved_cmap_vec(count)?;
        for _i in 0..seg_count_x2 / 2 {
            let v = reader.read_be_i16()?;
            id_delta.push(v);
        }
        let mut id_range_offset = reserved_cmap_vec(count)?;
        for _i in 0..seg_count_x2 / 2 {
            let v = reader.read_be_u16()?;
            id_range_offset.push(v);
        }
        let end = reader.offset();
        let remain_len = usize::from(length)
            .checked_sub(end - start + 6)
            .ok_or_else(|| PdfError::FontError("Truncated TrueType cmap format 4".into()))?;
        if remain_len % 2 != 0
            || reserved_pad != 0
            || start_code
                .iter()
                .zip(&end_code)
                .any(|(start, end)| start > end)
            || end_code.windows(2).any(|p| p[0] >= p[1])
            || start_code
                .iter()
                .skip(1)
                .zip(&end_code)
                .any(|(start, previous_end)| start <= previous_end)
            || end_code.last() != Some(&0xffff)
            || start_code.last() != Some(&0xffff)
            || id_range_offset.iter().any(|offset| offset % 2 != 0)
        {
            return Err(PdfError::FontError(
                "Invalid TrueType cmap format 4 segments".into(),
            ));
        }
        let mut glyph_id_array = reserved_cmap_vec(remain_len / 2)?;
        for _i in 0..remain_len / 2 {
            let v = reader.read_be_u16()?;
            glyph_id_array.push(v);
        }
        Ok(Self {
            end_code,
            start_code,
            id_delta,
            id_range_offset,
            glyph_id_array,
        })
    }

    pub fn get_gid(&self, c: u16) -> u16 {
        let i = self.end_code.partition_point(|end| *end < c);
        if i < self.end_code.len() {
            let start = self.start_code[i];
            let end = self.end_code[i];
            if c >= start && c <= end {
                if self.id_range_offset[i] == 0 {
                    let gid = (c as i32 + self.id_delta[i] as i32) as u16;
                    return gid;
                } else {
                    let index =
                        (usize::from(self.id_range_offset[i]) / 2 + usize::from(c - start) + i)
                            .checked_sub(self.id_range_offset.len());
                    let gid = index
                        .and_then(|index| self.glyph_id_array.get(index))
                        .copied()
                        .unwrap_or(0);
                    return if gid == 0 {
                        0
                    } else {
                        gid.wrapping_add(self.id_delta[i] as u16)
                    };
                }
            }
        }
        0
    }
}

#[derive(Debug, Clone)]
pub enum SubCmap {
    F0(Format0Cmap),
    F4(Format4Cmap),
    F12(Format12Cmap),
}

#[derive(Debug, Clone)]
pub struct Format12Cmap {
    groups: Vec<(u32, u32, u32)>,
}

impl Format12Cmap {
    fn try_new(reader: &mut TrueTypeReader, length: u32) -> PdfResult<Self> {
        let length = usize::try_from(length)
            .map_err(|_| PdfError::FontError("cmap length overflow".into()))?;
        if length < 16 || length - 12 > reader.remaining() {
            return Err(PdfError::FontError("Truncated cmap format 12".into()));
        }
        let count = usize::try_from(reader.read_be_u32()?)
            .map_err(|_| PdfError::FontError("cmap group count overflow".into()))?;
        if count
            .checked_mul(12)
            .and_then(|bytes| bytes.checked_add(16))
            != Some(length)
        {
            return Err(PdfError::FontError(
                "Invalid cmap format 12 group count/length".into(),
            ));
        }
        let mut groups = reserved_cmap_vec(count)?;
        let mut previous_end = None;
        for _ in 0..count {
            let start = reader.read_be_u32()?;
            let end = reader.read_be_u32()?;
            let gid = reader.read_be_u32()?;
            if start > end
                || previous_end.is_some_and(|previous| start <= previous)
                || gid.checked_add(end - start).is_none()
            {
                return Err(PdfError::FontError(
                    "Invalid cmap format 12 group range".into(),
                ));
            }
            groups.push((start, end, gid));
            previous_end = Some(end);
        }
        Ok(Self { groups })
    }

    pub fn get_gid(&self, code: u32) -> u32 {
        let index = self.groups.partition_point(|(_, end, _)| *end < code);
        match self.groups.get(index) {
            Some((start, end, gid)) if (*start..=*end).contains(&code) => gid + (code - start),
            _ => 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TCmap {
    platform_id: u16,
    encoding_id: u16,
    submap: SubCmap,
}
impl TCmap {
    pub fn try_new(
        platform_id: u16,
        encoding_id: u16,
        offset: u32,
        reader: &mut TrueTypeReader,
    ) -> PdfResult<Self> {
        reader.reset_offset(offset as usize);
        let format = reader.read_be_u16()?;
        if format == 12 {
            let reserved = reader.read_be_u16()?;
            let length = reader.read_be_u32()?;
            let language = reader.read_be_u32()?;
            if reserved != 0 || (platform_id != 1 && language != 0) {
                return Err(PdfError::FontError("Invalid cmap format 12 header".into()));
            }
            let submap = SubCmap::F12(Format12Cmap::try_new(reader, length)?);
            return Ok(Self {
                platform_id,
                encoding_id,
                submap,
            });
        }
        let length = reader.read_be_u16()?;
        let _language = reader.read_be_u16()?;
        if length < 6 || usize::from(length) - 6 > reader.remaining() {
            return Err(PdfError::FontError(
                "TrueType cmap declared length exceeds data".into(),
            ));
        }
        match format {
            0 => {
                if length != 262 {
                    return Err(PdfError::FontError(
                        "Invalid TrueType cmap format 0 length".into(),
                    ));
                }
                let sb = Format0Cmap::try_new(reader)?;
                let submap = SubCmap::F0(sb);
                Ok(TCmap {
                    platform_id,
                    encoding_id,
                    submap,
                })
            }
            4 => {
                let sb = Format4Cmap::try_new(reader, length)?;
                let submap = SubCmap::F4(sb);
                Ok(TCmap {
                    platform_id,
                    encoding_id,
                    submap,
                })
            }
            _ => Err(PdfError::UnsupportedFeature(format!(
                "TrueType cmap format {format}"
            ))),
        }
    }

    pub fn get_gid(&self, charcode: u16) -> u16 {
        u16::try_from(self.get_gid_u32(u32::from(charcode))).unwrap_or(0)
    }

    /// Full-width character lookup; never truncates supplementary-plane codes.
    pub fn get_gid_u32(&self, charcode: u32) -> u32 {
        match &self.submap {
            SubCmap::F0(f) => {
                if charcode > 255 {
                    0
                } else {
                    u32::from(f.get_gid(charcode as u8))
                }
            }
            SubCmap::F4(f) => u16::try_from(charcode)
                .map(|code| u32::from(f.get_gid(code)))
                .unwrap_or(0),
            SubCmap::F12(f) => f.get_gid(charcode),
        }
    }

    pub fn platform_id(&self) -> u16 {
        self.platform_id
    }

    pub fn encoding_id(&self) -> u16 {
        self.encoding_id
    }
}
