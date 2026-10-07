use crate::error::{PdfError, PdfResult};
use crate::filter::predictor::apply_predictor;
use crate::objects::pdf_dict::PdfDict;
use flate2::bufread::ZlibDecoder;
use std::io::Read;

pub fn flate_decode(input: &[u8], params: Option<&PdfDict>) -> PdfResult<Vec<u8>> {
    flate_decode_limited(
        input,
        params,
        crate::parser::options::Limits::default().max_decoded_bytes,
    )
}

pub fn flate_decode_limited(
    input: &[u8],
    params: Option<&PdfDict>,
    limit: usize,
) -> PdfResult<Vec<u8>> {
    let decoder = ZlibDecoder::new(input);
    let mut decoder = decoder.take((limit as u64).saturating_add(1));
    let mut decompressed = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let length = decoder.read(&mut chunk).map_err(|e| {
            PdfError::FilterError(format!("FlateDecode zlib decompression failed: {e}"))
        })?;
        if length == 0 {
            break;
        }
        super::check_output_size(decompressed.len(), length, limit, "FlateDecode")?;
        decompressed
            .try_reserve(length)
            .map_err(|_| PdfError::ResourceLimit("FlateDecode output allocation".into()))?;
        decompressed.extend_from_slice(&chunk[..length]);
    }
    apply_predictor(decompressed, params, "FlateDecode")
}

/// Decode with output capacity and replacement overlap charged to one pool.
/// Zlib's internal state and the fixed stack read block are not accounted here.
pub fn flate_decode_with_budget(
    input: &[u8],
    params: Option<&PdfDict>,
    limit: usize,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<crate::budget::BudgetedBytes> {
    let mut lease = budget.reserve(0)?;
    let mut decompressed = Vec::new();
    let mut decoder = ZlibDecoder::new(input).take((limit as u64).saturating_add(1));
    let mut chunk = [0u8; 8192];
    loop {
        let length = decoder.read(&mut chunk).map_err(|e| {
            PdfError::FilterError(format!("FlateDecode zlib decompression failed: {e}"))
        })?;
        if length == 0 {
            break;
        }
        super::check_output_size(decompressed.len(), length, limit, "FlateDecode")?;
        let required = decompressed.len() + length;
        if required > decompressed.capacity() {
            let mut replacement_lease = budget.reserve(required)?;
            let mut replacement = Vec::new();
            replacement
                .try_reserve_exact(required)
                .map_err(|_| PdfError::ResourceLimit("FlateDecode output allocation".into()))?;
            replacement_lease.resize(replacement.capacity())?;
            replacement.extend_from_slice(&decompressed);
            decompressed = replacement;
            lease = replacement_lease;
        }
        decompressed.extend_from_slice(&chunk[..length]);
    }
    // TIFF and PNG reconstruction reuse this allocation; retain its capacity
    // reservation even when PNG removes the row filter bytes.
    let decoded = apply_predictor(decompressed, params, "FlateDecode")?;
    crate::budget::BudgetedBytes::from_reserved_vec(decoded, lease)
}

#[cfg(test)]
mod tests {
    use super::flate_decode;

    #[test]
    fn budgeted_flate_charges_growth_overlap_and_releases_failures() {
        use crate::budget::OperationBudget;
        use std::io::Write;
        let source = vec![b'X'; 8193];
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&source).unwrap();
        let encoded = encoder.finish().unwrap();
        // The first 8192-byte allocation coexists with its 8193-byte replacement.
        let budget = OperationBudget::new(8192 + 8193);
        let neighbor = budget.reserve(1).unwrap();
        assert!(matches!(
            super::flate_decode_with_budget(&encoded, None, source.len(), &budget),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        assert_eq!(budget.live_bytes(), 1);
        drop(neighbor);
        let decoded =
            super::flate_decode_with_budget(&encoded, None, source.len(), &budget).unwrap();
        assert_eq!(decoded.as_slice(), source);
        assert_eq!(budget.live_bytes(), decoded.capacity());
        drop(decoded);
        assert_eq!(budget.live_bytes(), 0);
        assert!(
            super::flate_decode_with_budget(&encoded, None, source.len() - 1, &budget).is_err()
        );
        assert_eq!(budget.live_bytes(), 0);
        let mut corrupt = encoded;
        *corrupt.last_mut().unwrap() ^= 1;
        assert!(super::flate_decode_with_budget(&corrupt, None, source.len(), &budget).is_err());
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn flate_limit_stops_expansion_and_accepts_exact_limit() {
        use std::io::Write;
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&vec![0; 64 * 1024]).unwrap();
        let encoded = encoder.finish().unwrap();
        assert!(matches!(
            super::flate_decode_limited(&encoded, None, 1024),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        assert_eq!(
            super::flate_decode_limited(&encoded, None, 64 * 1024)
                .unwrap()
                .len(),
            64 * 1024
        );
    }

    #[test]
    fn test_flated_decode() {
        let encoded = [
            0x78, 0x9c, 0x4b, 0xcb, 0xcf, 0x07, 0x00, 0x02, 0x82, 0x01, 0x45,
        ];
        let res = flate_decode(&encoded, None).unwrap();
        assert_eq!(res, b"foo");
    }

    #[test]
    fn test_flated_decode_rejects_truncated_stream() {
        let encoded = [
            0x78, 0x9c, 0x4b, 0xcb, 0xcf, 0x07, 0x00, 0x02, 0x82, 0x01, 0x46,
        ];
        assert!(flate_decode(&encoded, None).is_err());
    }

    #[test]
    fn flate_zero_and_chunk_boundary_limits() {
        use std::io::Write;
        for length in [0, 8191, 8192, 8193] {
            let source = vec![b'X'; length];
            let mut encoder =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(&source).unwrap();
            let encoded = encoder.finish().unwrap();
            assert_eq!(
                super::flate_decode_limited(&encoded, None, length).unwrap(),
                source
            );
            if length > 0 {
                assert!(matches!(
                    super::flate_decode_limited(&encoded, None, length - 1),
                    Err(crate::error::PdfError::ResourceLimit(_))
                ));
            }
        }
    }
}
