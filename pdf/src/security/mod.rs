//! Standard AES-256/R6 protection and read-only AES-128/R4 interoperability.
//! Permissions are advisory PDF policy, not DRM. Ciphertext is not authenticated
//! content; signatures are a separate API. Controlled secret buffers are cleared,
//! but caller input, persistent plaintext caches and every temporary cannot be.
mod r4;
mod r6;
pub(crate) mod transform;
pub use r6::SecurityHandler;
pub(crate) use r6::random as r6_random;

use crate::error::{PdfError, PdfResult};
use zeroize::Zeroizing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Authentication {
    User,
    Owner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions {
    pub print: bool,
    pub modify: bool,
    pub copy: bool,
    pub annotate: bool,
    pub fill_forms: bool,
    pub accessibility: bool,
    pub assemble: bool,
    pub high_quality_print: bool,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            print: true,
            modify: true,
            copy: true,
            annotate: true,
            fill_forms: true,
            accessibility: true,
            assemble: true,
            high_quality_print: true,
        }
    }
}

impl Permissions {
    pub(crate) fn bits(self) -> i32 {
        let mut bits = 0xffff_f0c0u32;
        for (allowed, bit) in [
            (self.print, 2),
            (self.modify, 3),
            (self.copy, 4),
            (self.annotate, 5),
            (self.fill_forms, 8),
            (self.accessibility, 9),
            (self.assemble, 10),
            (self.high_quality_print, 11),
        ] {
            if allowed {
                bits |= 1 << bit;
            }
        }
        bits as i32
    }

    pub(crate) fn from_bits(bits: i32) -> PdfResult<Self> {
        if bits as u32 & 0xffff_f0c3 != 0xffff_f0c0 {
            return Err(PdfError::MalformedSecurity(
                "reserved permission bits".into(),
            ));
        }
        let has = |bit: u32| bits as u32 & (1u32 << bit) != 0u32;
        Ok(Self {
            print: has(2),
            modify: has(3),
            copy: has(4),
            annotate: has(5),
            fill_forms: has(8),
            accessibility: has(9),
            assemble: has(10),
            high_quality_print: has(11),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecurityReport {
    /// Authenticated Standard handler revision (currently 4 or 6).
    pub revision: u32,
    pub authentication: Authentication,
    pub permissions: Permissions,
    pub encrypt_metadata: bool,
    /// Default crypt-filter methods. Explicit Crypt/Identity streams and
    /// clear metadata remain exceptions, not confidentiality guarantees.
    pub strings_use_aes256: bool,
    pub streams_use_aes256: bool,
    pub embedded_files_use_aes256: bool,
}

/// R=6 output policy. Passwords are SASLprep UTF-8, at most 127 normalized
/// bytes. Longer input is rejected, not silently truncated. Owner must be
/// nonempty; empty user passwords are explicitly supported.
pub struct EncryptionOptions {
    user: Zeroizing<Vec<u8>>,
    owner: Zeroizing<Vec<u8>>,
    pub permissions: Permissions,
    pub encrypt_metadata: bool,
}

impl std::fmt::Debug for EncryptionOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptionOptions")
            .field("passwords", &"[REDACTED]")
            .field("permissions", &self.permissions)
            .field("encrypt_metadata", &self.encrypt_metadata)
            .finish()
    }
}

pub(crate) fn normalized_password(password: &[u8]) -> PdfResult<Zeroizing<Vec<u8>>> {
    // Bound even the normalization input before allocating an expanded result.
    if password.len() > 1024 {
        return Err(PdfError::UnsupportedSecurity(
            "password input length".into(),
        ));
    }
    let text = std::str::from_utf8(password)
        .map_err(|_| PdfError::MalformedSecurity("password must be UTF-8".into()))?;
    let prepared = stringprep::saslprep(text)
        .map_err(|_| PdfError::MalformedSecurity("prohibited password characters".into()))?;
    let prepared = Zeroizing::new(prepared.into_owned());
    if prepared.len() > 127 {
        return Err(PdfError::UnsupportedSecurity(
            "normalized password exceeds 127 bytes".into(),
        ));
    }
    Ok(Zeroizing::new(prepared.as_bytes().to_vec()))
}

impl EncryptionOptions {
    pub fn new(user_password: &str, owner_password: &str) -> PdfResult<Self> {
        let user = normalized_password(user_password.as_bytes())?;
        let owner = normalized_password(owner_password.as_bytes())?;
        if owner.is_empty() {
            return Err(PdfError::UnsupportedSecurity(
                "empty output owner password".into(),
            ));
        }
        Ok(Self {
            user,
            owner,
            permissions: Permissions::default(),
            encrypt_metadata: true,
        })
    }
}

/// Explicit full-rewrite security policy. The default preserves existing
/// protection regardless of edits to trailer Encrypt. Changing/removing an
/// existing protected document's security requires owner authentication.
#[derive(Debug, Clone, Copy, Default)]
pub enum SaveSecurity<'a> {
    #[default]
    Preserve,
    Remove,
    Set(&'a EncryptionOptions),
}

pub(crate) use transform::PendingStreamCrypt;
