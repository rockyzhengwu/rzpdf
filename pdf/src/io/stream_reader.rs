use std::io::{Read, Seek, SeekFrom};

use crate::error::{PdfError, PdfResult};

#[derive(Debug)]
pub struct StreamReader<R: Read + Seek> {
    inner: R,
    length: u64,
}

impl<R: Read + Seek> StreamReader<R> {
    pub fn try_new(mut inner: R) -> PdfResult<Self> {
        let length = inner
            .seek(SeekFrom::End(0))
            .map_err(|e| PdfError::ReaderError(format!("StreamReader Failed seek end:{:?}", e)))?;
        inner.seek(SeekFrom::Start(0)).map_err(|e| {
            PdfError::ReaderError(format!("StreamReader failed seek start :{:?}", e))
        })?;
        Ok(Self { inner, length })
    }

    pub fn read_byte(&mut self) -> PdfResult<u8> {
        let mut buf = [0u8; 1];
        self.inner
            .read_exact(&mut buf)
            .map_err(|e| PdfError::ReaderError(format!("StreamReader failed read byte:{:?}", e)))?;
        Ok(buf[0])
    }

    /// Reads n bytes from the stream
    pub fn read_bytes(&mut self, len: usize) -> PdfResult<Vec<u8>> {
        let position = self.inner.stream_position()?;
        if len as u64 > self.length.saturating_sub(position) {
            return Err(PdfError::ReaderError(
                "requested bytes exceed remaining input".into(),
            ));
        }
        let mut buf = Vec::new();
        buf.try_reserve_exact(len)
            .map_err(|_| PdfError::ResourceLimit("input read buffer allocation".into()))?;
        buf.resize(len, 0);
        self.inner
            .read_exact(&mut buf)
            .map_err(|e| PdfError::ReaderError(format!("StreamReader failed read byte:{:?}", e)))?;
        Ok(buf)
    }

    pub fn length(&self) -> u64 {
        self.length
    }
    /// Capacity remains charged while the returned owner lives. Budget/input
    /// preflight failures do not advance; I/O failures may partially advance.
    pub fn read_bytes_with_budget(
        &mut self,
        len: usize,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<crate::budget::BudgetedBytes> {
        let position = self.inner.stream_position()?;
        if len as u64 > self.length.saturating_sub(position) {
            return Err(PdfError::ReaderError(
                "requested bytes exceed remaining input".into(),
            ));
        }
        crate::budget::BudgetedBytes::read_exact_from(budget, &mut self.inner, len)
    }
    pub fn seek(&mut self, pos: u64) -> PdfResult<()> {
        self.inner
            .seek(SeekFrom::Start(pos))
            .map_err(|e| PdfError::ReaderError(format!("StreamReader Seek error:{:?}", e)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Cursor};

    #[test]
    fn budgeted_reads_share_capacity_and_fail_without_advancing_on_preflight() {
        let budget = crate::budget::OperationBudget::new(3);
        let mut reader = StreamReader::try_new(Cursor::new(b"abcdef".as_slice())).unwrap();
        let first = reader.read_bytes_with_budget(2, &budget).unwrap();
        assert_eq!(first.as_slice(), b"ab");
        assert!(reader.read_bytes_with_budget(2, &budget).is_err());
        assert_eq!(reader.inner.position(), 2);
        assert_eq!(budget.live_bytes(), first.capacity());
        assert!(reader.read_bytes_with_budget(5, &budget).is_err());
        assert_eq!(reader.inner.position(), 2);
        drop(first);
        assert_eq!(budget.live_bytes(), 0);
        let second = reader.read_bytes_with_budget(3, &budget).unwrap();
        assert_eq!(second.as_slice(), b"cde");
        assert_eq!(budget.live_bytes(), second.capacity());
        drop(second);
        assert_eq!(budget.live_bytes(), 0);
    }

    // A seekable source declaring a large length without allocating backing bytes.
    struct PartialFailureReader(Cursor<Vec<u8>>);

    impl Read for PartialFailureReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.0.position() == 2 {
                return Err(io::Error::other("injected partial read failure"));
            }
            let length = buf.len().min(2);
            self.0.read(&mut buf[..length])
        }
    }

    impl Seek for PartialFailureReader {
        fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
            self.0.seek(from)
        }
    }

    #[test]
    fn budgeted_io_failure_releases_temporary_capacity_without_rewinding() {
        let budget = crate::budget::OperationBudget::new(7);
        let neighbor = crate::budget::BudgetedBytes::copy_from_slice(&budget, b"old").unwrap();
        let mut reader =
            StreamReader::try_new(PartialFailureReader(Cursor::new(b"abcdef".to_vec()))).unwrap();
        assert!(reader.read_bytes_with_budget(4, &budget).is_err());
        assert_eq!(reader.inner.stream_position().unwrap(), 2);
        assert_eq!(neighbor.as_slice(), b"old");
        assert_eq!(budget.live_bytes(), neighbor.capacity());
        reader.seek(3).unwrap();
        let recovered = reader.read_bytes_with_budget(2, &budget).unwrap();
        assert_eq!(recovered.as_slice(), b"de");
        assert_eq!(
            budget.live_bytes(),
            neighbor.capacity() + recovered.capacity()
        );
        drop(recovered);
        drop(neighbor);
        assert_eq!(budget.live_bytes(), 0);
    }

    struct DeclaredLengthReader {
        position: u64,
    }

    impl Read for DeclaredLengthReader {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Ok(0)
        }
    }

    impl Seek for DeclaredLengthReader {
        fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
            self.position = match from {
                SeekFrom::Start(pos) => pos,
                SeekFrom::End(0) => u64::MAX,
                SeekFrom::Current(0) => self.position,
                _ => return Err(io::Error::other("unsupported test seek")),
            };
            Ok(self.position)
        }
    }

    #[test]
    fn read_allocation_overflow_returns_resource_limit() {
        let mut reader = StreamReader::try_new(DeclaredLengthReader { position: 0 }).unwrap();
        assert!(matches!(
            reader.read_bytes(usize::MAX),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(reader.inner.stream_position().unwrap(), 0);
        assert!(reader.read_bytes(1).is_err());
        let budget = crate::budget::OperationBudget::new(usize::MAX);
        assert!(matches!(
            reader.read_bytes_with_budget(usize::MAX, &budget),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(budget.live_bytes(), 0);
        assert_eq!(reader.inner.stream_position().unwrap(), 0);
    }

    #[test]
    fn read_bounds_preserve_position_and_allow_exact_end() {
        let mut reader = StreamReader::try_new(Cursor::new(b"abc")).unwrap();
        assert!(reader.read_bytes(4).is_err());
        assert_eq!(reader.read_bytes(3).unwrap(), b"abc");
        assert_eq!(reader.read_bytes(0).unwrap(), b"");
        assert!(reader.read_bytes(1).is_err());
        reader.seek(0).unwrap();
        assert_eq!(reader.read_bytes(1).unwrap(), b"a");
    }
}
