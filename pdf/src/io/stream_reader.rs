use std::io::{Read, Seek, SeekFrom};

use crate::error::{PdfError, PdfResult};

#[derive(Debug)]
pub struct StreamReader<R: Read + Seek> {
    inner: R,
    length: u64,
    position: u64,
    position_known: bool,
}

impl<R: Read + Seek> StreamReader<R> {
    pub fn try_new(mut inner: R) -> PdfResult<Self> {
        let length = inner
            .seek(SeekFrom::End(0))
            .map_err(|e| PdfError::ReaderError(format!("StreamReader Failed seek end:{:?}", e)))?;
        inner.seek(SeekFrom::Start(0)).map_err(|e| {
            PdfError::ReaderError(format!("StreamReader failed seek start :{:?}", e))
        })?;
        Ok(Self {
            inner,
            length,
            position: 0,
            position_known: true,
        })
    }

    pub fn read_byte(&mut self) -> PdfResult<u8> {
        let mut buf = [0u8; 1];
        if let Err(error) = self.inner.read_exact(&mut buf) {
            // read_exact may consume a prefix before failing. Query the source
            // only on recovery, never on ordinary lexer reads/lookbehind.
            self.position_known = false;
            return Err(PdfError::ReaderError(format!(
                "StreamReader failed read byte:{error:?}"
            )));
        }
        self.position = self.position.saturating_add(1);
        Ok(buf[0])
    }

    /// Reads n bytes from the stream
    pub fn read_bytes(&mut self, len: usize) -> PdfResult<Vec<u8>> {
        let position = self.current_position()?;
        if len as u64 > self.length.saturating_sub(position) {
            return Err(PdfError::ReaderError(
                "requested bytes exceed remaining input".into(),
            ));
        }
        let mut buf = Vec::new();
        buf.try_reserve_exact(len)
            .map_err(|_| PdfError::ResourceLimit("input read buffer allocation".into()))?;
        buf.resize(len, 0);
        if let Err(error) = self.inner.read_exact(&mut buf) {
            self.position_known = false;
            return Err(PdfError::ReaderError(format!(
                "StreamReader failed read byte:{error:?}"
            )));
        }
        self.position = position + len as u64;
        self.position_known = true;
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
        let position = self.current_position()?;
        if len as u64 > self.length.saturating_sub(position) {
            return Err(PdfError::ReaderError(
                "requested bytes exceed remaining input".into(),
            ));
        }
        let result = crate::budget::BudgetedBytes::read_exact_from(budget, &mut self.inner, len);
        self.position_known = result.is_ok();
        if self.position_known {
            self.position = position + len as u64;
        }
        result
    }
    fn current_position(&mut self) -> std::io::Result<u64> {
        if !self.position_known {
            self.position = self.inner.stream_position()?;
            self.position_known = true;
        }
        Ok(self.position)
    }
    pub fn seek(&mut self, pos: u64) -> PdfResult<()> {
        // BufReader discards its buffer on an absolute seek, including the
        // one-byte lookbehind used by the lexer. Relative seeks can stay in
        // the buffer. Preserve arbitrary Read+Seek inputs with an absolute
        // fallback when position queries or relative seeks aren't supported.
        let relative = (pos <= self.length)
            .then(|| self.current_position().ok())
            .flatten()
            .and_then(|current| i64::try_from(i128::from(pos) - i128::from(current)).ok());
        let result = if let Some(offset) = relative {
            self.inner
                .seek_relative(offset)
                .or_else(|_| self.inner.seek(SeekFrom::Start(pos)).map(|_| ()))
        } else {
            self.inner.seek(SeekFrom::Start(pos)).map(|_| ())
        };
        self.position_known = result.is_ok();
        if self.position_known {
            self.position = pos;
        }
        result.map_err(|e| PdfError::ReaderError(format!("StreamReader Seek error:{e:?}")))
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
    fn boxed_buffered_lookbehind_reuses_input_and_crosses_buffer_boundaries() {
        use std::{cell::Cell, rc::Rc};
        struct Counted {
            data: Cursor<Vec<u8>>,
            reads: Rc<Cell<usize>>,
            seeks: Rc<Cell<usize>>,
        }
        impl Read for Counted {
            fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                self.reads.set(self.reads.get() + 1);
                self.data.read(out)
            }
        }
        impl Seek for Counted {
            fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
                self.seeks.set(self.seeks.get() + 1);
                self.data.seek(position)
            }
        }
        let reads = Rc::new(Cell::new(0));
        let seeks = Rc::new(Cell::new(0));
        let input = Counted {
            data: Cursor::new((0..128).collect()),
            reads: reads.clone(),
            seeks: seeks.clone(),
        };
        trait Input: Read + Seek {}
        impl<T: Read + Seek> Input for T {}
        let boxed: Box<dyn Input> = Box::new(io::BufReader::with_capacity(32, input));
        let mut reader = StreamReader::try_new(boxed).unwrap();
        for position in 0..80 {
            assert_eq!(reader.read_byte().unwrap(), position as u8);
            reader.seek(position).unwrap();
            assert_eq!(reader.read_byte().unwrap(), position as u8);
        }
        assert!(
            reads.get() <= 4,
            "lexer lookbehind must not discard each buffer"
        );
        assert!(
            seeks.get() <= 4,
            "lookbehind must not query the source position per token"
        );
        reader.seek(3).unwrap();
        assert_eq!(reader.read_bytes(3).unwrap(), [3, 4, 5]);
        reader.seek(128).unwrap();
        assert!(reader.read_byte().is_err());
        reader.seek(127).unwrap();
        assert_eq!(reader.read_byte().unwrap(), 127);
    }

    #[test]
    fn absolute_only_readers_and_offsets_beyond_i64_keep_seek_support() {
        struct Absolute(Cursor<Vec<u8>>);
        impl Read for Absolute {
            fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                self.0.read(out)
            }
        }
        impl Seek for Absolute {
            fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
                match from {
                    SeekFrom::Current(_) => Err(io::Error::other("relative position unsupported")),
                    _ => self.0.seek(from),
                }
            }
        }
        let mut reader = StreamReader::try_new(Absolute(Cursor::new(b"abc".to_vec()))).unwrap();
        reader.seek(1).unwrap();
        assert_eq!(reader.read_byte().unwrap(), b'b');
        reader.seek(u64::MAX).unwrap();
        assert!(reader.read_byte().is_err());
        reader.seek(0).unwrap();
        assert_eq!(reader.read_byte().unwrap(), b'a');
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
