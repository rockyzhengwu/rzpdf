//! Rewrite name ID 6 without dropping copyright, licensing, or language tags.
use crate::{
    budget::{BudgetedBytes, OperationBudget},
    error::{PdfError, PdfResult},
    parser::options::Limits,
};

fn word(data: &[u8], offset: usize) -> PdfResult<usize> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| PdfError::FontError("Truncated subset name table".into()))?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]) as usize)
}

pub(crate) fn rewrite_postscript_name(
    data: &[u8],
    name: &str,
    limits: &Limits,
    budget: &OperationBudget,
) -> PdfResult<BudgetedBytes> {
    if name.is_empty()
        || name.len() > 63
        || name
            .bytes()
            .any(|b| !(33..=126).contains(&b) || b"[](){}<>/%".contains(&b))
    {
        return Err(PdfError::FontError("Invalid subset PostScript name".into()));
    }
    let version = word(data, 0)?;
    let count = word(data, 2)?;
    let storage = word(data, 4)?;
    if version > 1 || count > limits.max_objects {
        return Err(PdfError::FontError(
            "Unsupported subset name directory".into(),
        ));
    }
    let records_end = 6 + count * 12;
    let tags = if version == 1 {
        word(data, records_end)?
    } else {
        0
    };
    let directory_end = records_end + if version == 1 { 2 + tags * 4 } else { 0 };
    if tags > limits.max_objects || storage < directory_end || storage > data.len() {
        return Err(PdfError::FontError("Invalid subset name storage".into()));
    }
    let range = |length: usize, offset: usize| -> PdfResult<()> {
        if offset
            .checked_add(length)
            .and_then(|end| storage.checked_add(end))
            .is_none_or(|end| end > data.len())
        {
            return Err(PdfError::FontError(
                "Subset name string exceeds storage".into(),
            ));
        }
        Ok(())
    };
    let mut replacements = 0;
    let mut extra = 0usize;
    for index in 0..count {
        let record = 6 + index * 12;
        range(word(data, record + 8)?, word(data, record + 10)?)?;
        let language = word(data, record + 4)?;
        let platform = word(data, record)?;
        if language >= 0x8000
            && !(240..=255).contains(&platform)
            && (version != 1 || language - 0x8000 >= tags)
        {
            return Err(PdfError::FontError(
                "Invalid subset name language reference".into(),
            ));
        }
        if word(data, record + 6)? == 6 {
            let platform = word(data, record)?;
            let encoding = word(data, record + 2)?;
            let unicode = platform == 0 || (platform == 3 && matches!(encoding, 0 | 1 | 10));
            if !unicode && !(platform == 1 && encoding == 0) {
                return Err(PdfError::FontError(
                    "Unsupported PostScript name encoding".into(),
                ));
            }
            replacements += 1;
            extra += name.len() * if unicode { 2 } else { 1 };
        }
    }
    for index in 0..tags {
        let record = records_end + 2 + index * 4;
        let length = word(data, record)?;
        if length % 2 != 0 {
            return Err(PdfError::FontError(
                "Invalid subset name language encoding".into(),
            ));
        }
        range(length, word(data, record + 2)?)?;
    }
    if replacements == 0 {
        return Err(PdfError::FontError("Missing PostScript name record".into()));
    }
    let total = data
        .len()
        .checked_add(extra)
        .filter(|&n| {
            n <= limits.max_decoded_bytes
                && n <= limits.max_raw_stream_bytes
                && n - storage <= u16::MAX as usize
        })
        .ok_or_else(|| PdfError::ResourceLimit("Subset name output bytes".into()))?;
    let mut lease = budget.reserve(total)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(total)
        .map_err(|_| PdfError::ResourceLimit("Subset name allocation".into()))?;
    lease.resize(output.capacity())?;
    output.extend_from_slice(data);
    for index in 0..count {
        let record = 6 + index * 12;
        if word(data, record + 6)? != 6 {
            continue;
        }
        let unicode = word(data, record)? != 1;
        let offset = output.len() - storage;
        let length = name.len() * if unicode { 2 } else { 1 };
        output[record + 8..record + 10].copy_from_slice(&(length as u16).to_be_bytes());
        output[record + 10..record + 12].copy_from_slice(&(offset as u16).to_be_bytes());
        for byte in name.bytes() {
            if unicode {
                output.push(0);
            }
            output.push(byte);
        }
    }
    BudgetedBytes::from_reserved_vec(output, lease)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_one_language_tags_and_mac_names_survive_and_bad_ranges_fail() {
        let mut data = vec![0, 1, 0, 1, 0, 24];
        for value in [1u16, 0, 0x8000, 6, 1, 0, 1, 4, 1] {
            data.extend_from_slice(&value.to_be_bytes());
        }
        data.extend_from_slice(&[b'A', 0, b'e', 0, b'n']);
        let budget = OperationBudget::new(4096);
        let output =
            rewrite_postscript_name(&data, "ABCDEF+Demo", &Limits::default(), &budget).unwrap();
        assert_eq!(&output.as_slice()[18..29], &data[18..29]);
        assert_eq!(&output.as_slice()[29..], b"ABCDEF+Demo");
        drop(output);
        assert_eq!(budget.live_bytes(), 0);
        for (offset, value) in [(10usize, 0x8001u16), (22, 100), (20, 3), (16, 100)] {
            let mut invalid = data.clone();
            invalid[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
            assert!(
                rewrite_postscript_name(&invalid, "Demo", &Limits::default(), &budget).is_err()
            );
            assert_eq!(budget.live_bytes(), 0);
        }
        for length in 0..data.len() {
            assert!(
                rewrite_postscript_name(&data[..length], "Demo", &Limits::default(), &budget)
                    .is_err()
            );
            assert_eq!(budget.live_bytes(), 0);
        }
    }
    #[test]
    fn preserves_license_and_rewrites_unicode_name_with_budget_cleanup() {
        let mut data = vec![0, 0, 0, 2, 0, 30];
        for (id, length, offset) in [(6u16, 2u16, 0u16), (13, 3, 2)] {
            for value in [3u16, 1, 0x409, id, length, offset] {
                data.extend_from_slice(&value.to_be_bytes());
            }
        }
        data.extend_from_slice(&[0, b'A', b'L', b'I', b'C']);
        let budget = OperationBudget::new(4096);
        let output =
            rewrite_postscript_name(&data, "ABCDEF+Demo", &Limits::default(), &budget).unwrap();
        assert_eq!(&output.as_slice()[18..35], &data[18..35]);
        assert_eq!(word(output.as_slice(), 14).unwrap(), 22);
        assert_eq!(word(output.as_slice(), 16).unwrap(), 5);
        assert_eq!(&output.as_slice()[35..39], &[0, b'A', 0, b'B']);
        drop(output);
        assert_eq!(budget.live_bytes(), 0);
        assert!(rewrite_postscript_name(&data, "Bad/Name", &Limits::default(), &budget).is_err());
        let tiny = OperationBudget::new(1);
        assert!(rewrite_postscript_name(&data, "Demo", &Limits::default(), &tiny).is_err());
        assert_eq!(tiny.live_bytes(), 0);
    }
}
