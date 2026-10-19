//! Typed binary xref records. Container indices are never generations.
use crate::error::{PdfError, PdfResult};

pub(super) const RECORD_BYTES: usize = 13; // /W [1 8 4]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum XrefRecord {
    Free { next: u32, generation: u16 },
    Uncompressed { offset: u64, generation: u16 },
    Compressed { container: u32, index: u32 },
}

impl XrefRecord {
    pub(super) fn encode(self) -> PdfResult<[u8; RECORD_BYTES]> {
        let (kind, field, last) = match self {
            Self::Free { next, generation } => (0, u64::from(next), u32::from(generation)),
            Self::Uncompressed { offset, generation } => (1, offset, u32::from(generation)),
            Self::Compressed { container: 0, .. } => {
                return Err(PdfError::ObjectError(
                    "xref object-stream number zero".into(),
                ));
            }
            Self::Compressed { container, index } => (2, u64::from(container), index),
        };
        let mut bytes = [0; RECORD_BYTES];
        bytes[0] = kind;
        bytes[1..9].copy_from_slice(&field.to_be_bytes());
        bytes[9..].copy_from_slice(&last.to_be_bytes());
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_fields_preserve_large_offsets_generations_and_container_indices() {
        for (record, kind, middle, last) in [
            (
                XrefRecord::Free {
                    next: 97,
                    generation: 65535,
                },
                0,
                97,
                65535,
            ),
            (
                XrefRecord::Uncompressed {
                    offset: u64::MAX,
                    generation: 7,
                },
                1,
                u64::MAX,
                7,
            ),
            (
                XrefRecord::Compressed {
                    container: 5,
                    index: 100000,
                },
                2,
                5,
                100000,
            ),
        ] {
            let bytes = record.encode().unwrap();
            assert_eq!(bytes[0], kind);
            assert_eq!(u64::from_be_bytes(bytes[1..9].try_into().unwrap()), middle);
            assert_eq!(u32::from_be_bytes(bytes[9..].try_into().unwrap()), last);
        }
        assert!(
            XrefRecord::Compressed {
                container: 0,
                index: 0
            }
            .encode()
            .is_err()
        );
    }
}
