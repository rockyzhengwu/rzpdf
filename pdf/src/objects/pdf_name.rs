use crate::error::{PdfError, PdfResult};
use crate::parser::parse_utility::hex_to_u8;

#[derive(Debug, PartialEq, Clone, Eq, Hash, Default)]
pub struct PdfName {
    bytes: Vec<u8>,
    name: String,
}

impl PdfName {
    pub fn new(name: String) -> Self {
        let bytes = name.as_bytes().to_vec();
        PdfName { bytes, name }
    }

    pub fn new_from_buffer(buffer: &[u8]) -> PdfResult<Self> {
        if buffer.first() != Some(&b'/') {
            return Err(PdfError::ParserError("name token must begin with /".into()));
        }
        let mut i = 1;
        let mut data = Vec::new();
        while i < buffer.len() {
            data.try_reserve(1)
                .map_err(|_| PdfError::ResourceLimit("PDF name byte allocation".into()))?;
            let c = buffer[i];
            if c == b'#' {
                if buffer.len() - i < 3 {
                    return Err(PdfError::ParserError(format!(
                        "incomplete name escape at byte {i}"
                    )));
                }
                let hi = buffer[i + 1];
                let lo = buffer[i + 2];
                if !hi.is_ascii_hexdigit() || !lo.is_ascii_hexdigit() {
                    return Err(PdfError::ParserError(format!(
                        "invalid name escape sequence: {:?}",
                        &buffer[i..=usize::min(i + 2, buffer.len() - 1)]
                    )));
                }
                let v = (hex_to_u8(hi) << 4) | hex_to_u8(lo);
                if v == 0 {
                    return Err(PdfError::ParserError(format!(
                        "name contains zero byte at {i}"
                    )));
                }
                data.push(v);
                i += 3;
            } else {
                if c == 0 {
                    return Err(PdfError::ParserError(format!(
                        "name contains zero byte at {i}"
                    )));
                }
                data.push(c);
                i += 1;
            }
        }
        let name = String::from_utf8_lossy(&data).into_owned();
        Ok(PdfName { bytes: data, name })
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub(crate) fn clone_with_budget(
        &self,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(Self, crate::budget::MemoryLease)> {
        let (bytes, mut lease) =
            crate::budget::BudgetedBytes::copy_from_slice(budget, &self.bytes)?
                .into_reserved_parts();
        let (name, mut name_lease) = crate::budget::copy_string(&self.name, budget)?;
        lease.absorb(&mut name_lease)?;
        Ok((Self { bytes, name }, lease))
    }

    pub(crate) fn into_budgeted_key(
        self,
        mut lease: crate::budget::MemoryLease,
    ) -> PdfResult<(String, crate::budget::MemoryLease)> {
        let Self { bytes, name } = self;
        drop(bytes);
        lease.resize(name.capacity())?;
        Ok((name, lease))
    }

    pub(crate) fn new_from_buffer_with_budget(
        buffer: &[u8],
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(Self, crate::budget::MemoryLease)> {
        fn visit(buffer: &[u8], mut emit: impl FnMut(u8)) -> PdfResult<()> {
            if buffer.first() != Some(&b'/') {
                return Err(PdfError::ParserError("name token must begin with /".into()));
            }
            let mut index = 1;
            while index < buffer.len() {
                let byte = if buffer[index] == b'#' {
                    let escape = buffer
                        .get(index + 1..index + 3)
                        .ok_or_else(|| PdfError::ParserError("incomplete name escape".into()))?;
                    if !escape.iter().all(u8::is_ascii_hexdigit) {
                        return Err(PdfError::ParserError("invalid name escape".into()));
                    }
                    index += 3;
                    (hex_to_u8(escape[0]) << 4) | hex_to_u8(escape[1])
                } else {
                    let byte = buffer[index];
                    index += 1;
                    byte
                };
                if byte == 0 {
                    return Err(PdfError::ParserError("name contains zero byte".into()));
                }
                emit(byte);
            }
            Ok(())
        }
        let mut length = 0usize;
        visit(buffer, |_| length += 1)?;
        let mut lease = budget.reserve(length)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| PdfError::ResourceLimit("PDF name bytes".into()))?;
        lease.resize(bytes.capacity())?;
        visit(buffer, |byte| bytes.push(byte))?;
        // Plan and write lossy UTF-8 without allocating a temporary Cow/String.
        fn lossy(bytes: &[u8], mut emit: impl FnMut(&str)) {
            let mut rest = bytes;
            while !rest.is_empty() {
                match std::str::from_utf8(rest) {
                    Ok(valid) => {
                        emit(valid);
                        break;
                    }
                    Err(error) => {
                        let valid = error.valid_up_to();
                        emit(std::str::from_utf8(&rest[..valid]).unwrap());
                        emit("\u{fffd}");
                        match error.error_len() {
                            Some(length) => rest = &rest[valid + length..],
                            None => break,
                        }
                    }
                }
            }
        }
        let mut name_length = Some(0usize);
        lossy(&bytes, |part| {
            name_length = name_length.and_then(|length| length.checked_add(part.len()));
        });
        let name_length =
            name_length.ok_or_else(|| PdfError::ResourceLimit("PDF name string length".into()))?;
        let mut name_lease = budget.reserve(name_length)?;
        let mut name = String::new();
        name.try_reserve_exact(name_length)
            .map_err(|_| PdfError::ResourceLimit("PDF name string".into()))?;
        name_lease.resize(name.capacity())?;
        lossy(&bytes, |part| name.push_str(part));
        lease.absorb(&mut name_lease)?;
        Ok((Self { bytes, name }, lease))
    }

    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgeted_names_preserve_binary_and_lossy_utf8() {
        for source in [
            "/",
            "/ABC",
            "/A#23B",
            "/#ff#80",
            "/#e4#b8#ad",
            "/#e4#b8",
            "/#e4#41#80",
        ] {
            let budget = crate::budget::OperationBudget::new(1024);
            let (name, lease) =
                PdfName::new_from_buffer_with_budget(source.as_bytes(), &budget).unwrap();
            assert_eq!(name, PdfName::new_from_buffer(source.as_bytes()).unwrap());
            assert_eq!(
                budget.live_bytes(),
                name.bytes.capacity() + name.name.capacity()
            );
            drop(name);
            drop(lease);
            assert_eq!(budget.live_bytes(), 0);
        }
    }

    #[test]
    fn budgeted_name_failure_preserves_neighbor_quota() {
        let budget = crate::budget::OperationBudget::new(4);
        let neighbor = budget.reserve(1).unwrap();
        assert!(PdfName::new_from_buffer_with_budget(b"/#ff", &budget).is_err());
        assert_eq!(budget.live_bytes(), 1);
        for source in ["", "Name", "/A#", "/A#0", "/A#gg", "/#00", "/A\0B"] {
            assert!(PdfName::new_from_buffer_with_budget(source.as_bytes(), &budget).is_err());
            assert_eq!(budget.live_bytes(), 1);
        }
        drop(neighbor);
        let (name, lease) = PdfName::new_from_buffer_with_budget(b"/#ff", &budget).unwrap();
        assert_eq!(name.bytes(), &[255]);
        assert_eq!(budget.live_bytes(), 4);
        drop(name);
        drop(lease);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn large_escaped_name_preserves_all_binary_bytes() {
        let mut source = vec![b'/'];
        for _ in 0..4096 {
            source.extend_from_slice(b"A#ff#80");
        }
        let name = PdfName::new_from_buffer(&source).unwrap();
        assert_eq!(name.bytes().len(), 4096 * 3);
        assert!(name
            .bytes()
            .chunks_exact(3)
            .all(|chunk| chunk == [b'A', 255, 128]));
    }

    #[test]
    fn name_token_requires_prefix_and_complete_escape() {
        for source in ["", "Name", "/A#", "/A#0", "/A#gg", "/A#0z", "/#00", "/A\0B"] {
            assert!(
                PdfName::new_from_buffer(source.as_bytes()).is_err(),
                "{source}"
            );
        }
        for (source, expected) in [
            ("/", &b""[..]),
            ("/A#23B", &b"A#B"[..]),
            ("/#ff#80", &b"\xff\x80"[..]),
            ("/#2f#20", &b"/ "[..]),
        ] {
            assert_eq!(
                PdfName::new_from_buffer(source.as_bytes()).unwrap().bytes(),
                expected
            );
        }
    }
}
