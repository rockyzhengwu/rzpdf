//! Bounded synchronous reads from an immutable browser Blob in a Worker.
use super::*;
use std::io::{self, Read, Seek, SeekFrom};

pub const MAX_SOURCE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub const SOURCE_BLOCK_BYTES: usize = 64 * 1024;
#[cfg(target_os = "emscripten")]
unsafe extern "C" {
    fn rz_blob_read(id: u32, offset: f64, target: *mut u8, length: u32) -> i32;
    fn rz_blob_release(id: u32);
}
struct BlobReader {
    id: u32,
    length: u64,
    position: u64,
}
impl Read for BlobReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let count = output
            .len()
            .min(SOURCE_BLOCK_BYTES)
            .min(self.length.saturating_sub(self.position) as usize);
        if count == 0 {
            return Ok(0);
        }
        #[cfg(target_os = "emscripten")]
        // The callback receives only this valid mutable output region. It may
        // fill at most count bytes and cannot retain a Rust-owned view.
        let result = unsafe {
            rz_blob_read(
                self.id,
                self.position as f64,
                output.as_mut_ptr(),
                count as u32,
            )
        };
        #[cfg(not(target_os = "emscripten"))]
        let result = -1;
        if result < 1 || result as usize > count {
            return Err(io::Error::other("browser source read failed"));
        }
        self.position += result as u64;
        Ok(result as usize)
    }
}
impl Seek for BlobReader {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(offset) => i128::from(offset),
            SeekFrom::Current(offset) => i128::from(self.position) + i128::from(offset),
            SeekFrom::End(offset) => i128::from(self.length) + i128::from(offset),
        };
        if !(0..=i128::from(self.length)).contains(&position) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "browser source seek outside input",
            ));
        }
        self.position = position as u64;
        Ok(self.position)
    }
}
impl Drop for BlobReader {
    fn drop(&mut self) {
        let _ = self.id;
        #[cfg(target_os = "emscripten")]
        // Releasing a module-local opaque ID is idempotent in the SDK.
        unsafe {
            rz_blob_release(self.id)
        };
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn rz_open_blob(id: u32, length: f64, options: u32) -> u32 {
    call(|state| {
        // Construct the owner first so every failure releases its source ID.
        let mut reader = BlobReader {
            id,
            length: 0,
            position: 0,
        };
        if id == 0
            || !length.is_finite()
            || length.fract() != 0.
            || length < 1.
            || length > MAX_SOURCE_BYTES as f64
        {
            return Err("invalid browser source length or ID".into());
        }
        #[cfg(not(target_os = "emscripten"))]
        {
            reader.length = length as u64;
            let _ = (state, reader, options);
            Err(ApiError::new(5, "browser source requires Emscripten"))
        }
        #[cfg(target_os = "emscripten")]
        {
            let (mode, policy, password) = crate::api::open_options(state, options)?;
            reader.length = length as u64;
            let reader = std::io::BufReader::with_capacity(SOURCE_BLOCK_BYTES, reader);
            let options = ParseOptions {
                mode,
                limits: policy,
            };
            let doc = match password {
                Some(password) => {
                    Document::from_reader_with_password_and_options(reader, &password, options)
                }
                None => Document::from_reader_with_options(reader, options),
            }?;
            state.insert(Resource::Document(Rc::new(doc), 0))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(not(target_os = "emscripten"))]
    fn host_bridge_reports_unsupported_instead_of_font_failure() {
        assert_eq!(rz_open_blob(1, 1., 0), 0);
        assert_eq!(crate::api::rz_error_code(), 5);
    }

    #[test]
    fn seeks_are_checked_without_wrapping_and_reads_stop_at_eof() {
        let mut reader = BlobReader {
            id: 1,
            length: MAX_SOURCE_BYTES,
            position: 0,
        };
        assert_eq!(
            reader.seek(SeekFrom::End(-1)).unwrap(),
            MAX_SOURCE_BYTES - 1
        );
        assert!(reader.seek(SeekFrom::Current(i64::MAX)).is_err());
        assert_eq!(reader.position, MAX_SOURCE_BYTES - 1);
        assert!(reader.seek(SeekFrom::Start(u64::MAX)).is_err());
        assert_eq!(reader.seek(SeekFrom::End(0)).unwrap(), MAX_SOURCE_BYTES);
        assert_eq!(reader.read(&mut [0; 16]).unwrap(), 0);
    }
}
