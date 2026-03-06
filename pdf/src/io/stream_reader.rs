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
        inner
            .seek(SeekFrom::Start(0))
            .map_err(|e| PdfError::ReaderError(format!("StreamReader failed seek start :{:?}", e)))
            .unwrap();
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
        let mut buf = vec![0u8; len];
        self.inner
            .read_exact(&mut buf)
            .map_err(|e| PdfError::ReaderError(format!("StreamReader failed read byte:{:?}", e)))?;
        Ok(buf)
    }

    pub fn length(&self) -> u64 {
        self.length
    }
    pub fn seek(&mut self, pos: u64) -> PdfResult<()> {
        self.inner
            .seek(SeekFrom::Start(pos))
            .map_err(|e| PdfError::ReaderError(format!("StreamReader Seek error:{:?}", e)))?;
        Ok(())
    }
}
