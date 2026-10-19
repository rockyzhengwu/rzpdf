//! Standalone static TrueType SFNT container. Table semantics are caller-owned.
use super::sfnt::table_checksum;
use crate::{
    budget::{BudgetedBytes, OperationBudget},
    error::{PdfError, PdfResult},
    parser::options::Limits,
};

pub(crate) fn write_sfnt(
    tables: &[([u8; 4], &[u8])],
    limits: &Limits,
    budget: &OperationBudget,
) -> PdfResult<BudgetedBytes> {
    let count = tables.len();
    // searchRange must be representable as uint16, including its factor 16.
    if count == 0 || count > 4095 || count > limits.max_objects {
        return Err(PdfError::ResourceLimit("SFNT output table count".into()));
    }
    let mut sorted_lease = budget.reserve(std::mem::size_of_val(tables))?;
    let mut sorted = Vec::new();
    sorted
        .try_reserve_exact(count)
        .map_err(|_| PdfError::ResourceLimit("SFNT directory allocation".into()))?;
    sorted_lease.resize(sorted.capacity() * std::mem::size_of::<([u8; 4], &[u8])>())?;
    sorted.extend_from_slice(tables);
    sorted.sort_unstable_by_key(|entry| entry.0);
    let mut total = 12 + count * 16;
    let mut has_head = false;
    for (index, (tag, data)) in sorted.iter().enumerate() {
        let mut space = false;
        let mut nonspace = false;
        for &byte in tag {
            if !(0x20..=0x7e).contains(&byte) || (space && byte != b' ') {
                return Err(PdfError::FontError("Invalid SFNT output tag".into()));
            }
            space |= byte == b' ';
            nonspace |= byte != b' ';
        }
        if !nonspace || (index > 0 && sorted[index - 1].0 == *tag) {
            return Err(PdfError::FontError(
                "Duplicate or blank SFNT output tag".into(),
            ));
        }
        if tag == b"head" {
            if data.len() < 54 {
                return Err(PdfError::FontError("Truncated SFNT output head".into()));
            }
            has_head = true;
        }
        total = data
            .len()
            .checked_add(3)
            .map(|length| length & !3)
            .and_then(|length| total.checked_add(length))
            .filter(|&length| {
                length <= u32::MAX as usize
                    && length <= limits.max_raw_stream_bytes
                    && length <= limits.max_decoded_bytes
            })
            .ok_or_else(|| PdfError::ResourceLimit("SFNT output bytes".into()))?;
    }
    if !has_head {
        return Err(PdfError::FontError("SFNT output head missing".into()));
    }
    let mut lease = budget.reserve(total)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(total)
        .map_err(|_| PdfError::ResourceLimit("SFNT output allocation".into()))?;
    lease.resize(output.capacity())?;
    output.resize(total, 0);
    output[..4].copy_from_slice(&0x00010000u32.to_be_bytes());
    let power = 1usize << (usize::BITS - 1 - count.leading_zeros());
    for (offset, value) in [
        (4, count),
        (6, power * 16),
        (8, power.trailing_zeros() as usize),
        (10, count * 16 - power * 16),
    ] {
        output[offset..offset + 2].copy_from_slice(&(value as u16).to_be_bytes());
    }
    let mut offset = 12 + count * 16;
    let mut head_adjustment = 0;
    for (index, (tag, data)) in sorted.iter().enumerate() {
        output[offset..offset + data.len()].copy_from_slice(data);
        if tag == b"head" {
            head_adjustment = offset + 8;
            output[head_adjustment..head_adjustment + 4].fill(0);
        }
        let checksum = table_checksum(&output[offset..offset + data.len()]);
        let record = 12 + index * 16;
        output[record..record + 4].copy_from_slice(tag);
        for (position, value) in [(4, checksum), (8, offset as u32), (12, data.len() as u32)] {
            output[record + position..record + position + 4].copy_from_slice(&value.to_be_bytes());
        }
        offset += (data.len() + 3) & !3;
    }
    let adjustment = 0xB1B0AFBAu32.wrapping_sub(table_checksum(&output));
    output[head_adjustment..head_adjustment + 4].copy_from_slice(&adjustment.to_be_bytes());
    BudgetedBytes::from_reserved_vec(output, lease)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_tables_and_exact_peak_refusals_release_and_retry() {
        let head = [0u8; 54];
        let tables = [(*b"head", &head[..]), (*b"test", &[1][..])];
        let probe = OperationBudget::new(4096);
        drop(write_sfnt(&tables, &Limits::default(), &probe).unwrap());
        let peak = probe.peak_bytes();
        assert_eq!(probe.live_bytes(), 0);
        let pool = OperationBudget::new(peak + 6);
        let neighbor = pool.reserve(7).unwrap();
        assert!(matches!(write_sfnt(&tables, &Limits::default(), &pool),
            Err(PdfError::ResourceLimit(message)) if message == "operation live bytes"));
        assert_eq!(pool.live_bytes(), 7);
        drop(neighbor);
        drop(write_sfnt(&tables, &Limits::default(), &pool).unwrap());
        assert_eq!(pool.live_bytes(), 0);
        for invalid in [
            vec![(*b"head", &head[..]), (*b"head", &head[..])],
            vec![(*b"head", &head[..]), (*b"a b ", &head[..])],
            vec![(*b"test", &head[..])],
            vec![(*b"head", &head[..53])],
        ] {
            assert!(write_sfnt(&invalid, &Limits::default(), &probe).is_err());
            assert_eq!(probe.live_bytes(), 0);
        }
        let limits = Limits {
            max_raw_stream_bytes: 1,
            ..Limits::default()
        };
        assert!(write_sfnt(&tables, &limits, &probe).is_err());
        assert_eq!(probe.live_bytes(), 0);
    }

    #[test]
    fn directory_alignment_checksums_and_budget_survive_reparse() {
        let mut head = [0u8; 54];
        head[8..12].fill(255);
        let tables = [(*b"test", &[1, 2, 3][..]), (*b"head", &head[..])];
        let pool = OperationBudget::new(4096);
        let output = write_sfnt(&tables, &Limits::default(), &pool).unwrap();
        let bytes = output.as_slice();
        assert_eq!(table_checksum(bytes), 0xB1B0AFBA);
        assert_eq!(&bytes[12..16], b"head");
        assert_eq!(&bytes[28..32], b"test");
        let sfnt = super::super::sfnt::Sfnt::try_new(
            &mut super::super::reader::TrueTypeReader::new(bytes),
        )
        .unwrap();
        let test = sfnt.get_table("test").unwrap();
        assert_eq!(test.offset() % 4, 0);
        assert_eq!(test.length(), 3);
        assert_eq!(
            &bytes[test.offset() as usize..test.offset() as usize + 3],
            &[1, 2, 3]
        );
        assert!(pool.live_bytes() >= bytes.len());
        drop(output);
        assert_eq!(pool.live_bytes(), 0);
        assert_eq!(&head[8..12], &[255; 4]);
    }
}
