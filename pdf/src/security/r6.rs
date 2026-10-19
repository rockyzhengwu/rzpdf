use super::{Authentication, EncryptionOptions, Permissions, SecurityReport, normalized_password};
use crate::{
    budget::{MemoryLease, OperationBudget},
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, pdf_array::PdfArray, pdf_bool::PdfBool, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_number::PdfNumber, pdf_string::PdfString,
    },
    parser::options::Limits,
};
use aes::cipher::{
    BlockDecrypt, BlockDecryptMut, BlockEncrypt, BlockEncryptMut, KeyInit, KeyIvInit,
    block_padding::NoPadding,
};
use sha2::{Digest, Sha256, Sha384, Sha512};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Method {
    Identity,
    Aes256,
}

pub struct SecurityHandler {
    pub(crate) dictionary: PdfDict,
    pub(crate) key: Zeroizing<[u8; 32]>,
    pub(crate) strings: Method,
    pub(crate) streams: Method,
    pub(crate) embedded_files: Method,
    pub(crate) report: SecurityReport,
}

impl std::fmt::Debug for SecurityHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecurityHandler")
            .field("revision", &6)
            .field("key", &"[REDACTED]")
            .field("report", &self.report)
            .finish()
    }
}

fn malformed(field: &str) -> PdfError {
    PdfError::MalformedSecurity(field.into())
}
fn unsupported(field: &str) -> PdfError {
    PdfError::UnsupportedSecurity(field.into())
}

pub(crate) fn random(bytes: &mut [u8]) -> PdfResult<()> {
    #[cfg(test)]
    if FAIL_RANDOM.with(|failure| failure.get()) {
        return Err(PdfError::RandomnessUnavailable);
    }
    getrandom::fill(bytes).map_err(|_| PdfError::RandomnessUnavailable)
}

#[cfg(test)]
thread_local! { static FAIL_RANDOM: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

pub(crate) fn secret_buffer(
    length: usize,
    budget: &OperationBudget,
) -> PdfResult<(Zeroizing<Vec<u8>>, MemoryLease)> {
    let mut lease = budget.reserve(length)?;
    let mut bytes = Zeroizing::new(Vec::new());
    bytes
        .try_reserve_exact(length)
        .map_err(|_| PdfError::ResourceLimit("security buffer allocation".into()))?;
    lease.resize(bytes.capacity())?;
    bytes.resize(length, 0);
    Ok((bytes, lease))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_unwrapping_does_not_strip_trailing_zero_bytes() {
        let key = [1; 32];
        let file_key = [0; 32];
        let wrapped = wrapped_key(&key, &file_key, true).unwrap();
        assert_eq!(*wrapped_key(&key, &*wrapped, false).unwrap(), file_key);
    }

    #[test]
    fn rng_failure_is_explicit_and_does_not_mutate_document_or_leak_budget() {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                FAIL_RANDOM.with(|failure| failure.set(false));
            }
        }
        let document = crate::document::PdfDocument::new().unwrap();
        let stamp = document.stamp();
        let settings = EncryptionOptions::new("user", "owner").unwrap();
        let budget = OperationBudget::new(2 * 1024 * 1024);
        let source = document.save_to_bytes_with_security(Default::default(),
            crate::security::SaveSecurity::Set(&settings)).unwrap();
        let incremental = crate::document::Document::from_bytes_with_password_and_options(
            source.clone(), "owner", crate::parser::options::ParseOptions {
                mode: crate::parser::syntax::ParseMode::Strict, ..Default::default()
            }).unwrap();
        let protected = crate::document::Document::from_bytes_with_password(source.clone(),"owner").unwrap();
        FAIL_RANDOM.with(|failure| failure.set(true));
        let _reset = Reset;
        // No-op does not allocate a structural ID or request randomness.
        let layout = crate::document::save_options::IncrementalSaveLayout::XrefStream;
        assert_eq!(incremental.save_incremental_to_bytes_with_layout(Default::default(), layout).unwrap(), source);
        incremental.set_info_text(crate::document::metadata::InfoTextField::Title, Some("private update")).unwrap();
        let incremental_stamp = incremental.stamp();
        assert!(matches!(incremental.save_incremental_with_layout_and_budget(
            Default::default(), layout, &budget), Err(PdfError::RandomnessUnavailable)));
        assert_eq!(incremental.stamp(), incremental_stamp);
        assert_eq!(budget.live_bytes(), 0);
        assert!(matches!(
            document.save_with_security_and_budget(
                Default::default(),
                crate::security::SaveSecurity::Set(&settings),
                &budget
            ),
            Err(PdfError::RandomnessUnavailable)
        ));
        assert!(document.is_stamp_current(&stamp));
        assert_eq!(budget.live_bytes(), 0);
        for layout in [crate::document::save_options::SaveLayout::XrefStream,
            crate::document::save_options::SaveLayout::XrefStreamWithObjectStreams] {
            assert!(matches!(document.save_with_layout_and_security_and_budget(
                Default::default(), layout, crate::security::SaveSecurity::Set(&settings), &budget,
            ), Err(PdfError::RandomnessUnavailable)));
            assert_eq!(budget.live_bytes(),0);
            assert!(document.is_stamp_current(&stamp));
            assert!(matches!(protected.save_with_layout_and_security_and_budget(
                Default::default(), layout, crate::security::SaveSecurity::Preserve, &budget,
            ), Err(PdfError::RandomnessUnavailable)));
            assert_eq!(budget.live_bytes(),0);
        }
        drop(_reset);
        let output = incremental.save_incremental_with_layout_and_budget(Default::default(), layout, &budget).unwrap();
        assert!(output.as_slice().starts_with(&source));
        assert_eq!(incremental.stamp(), incremental_stamp);
        drop(output);
        assert_eq!(budget.live_bytes(), 0);
    }
}

/// ISO 32000 algorithm 2.B. AES and hashes are delegated to RustCrypto;
/// the PDF-specific construction and termination condition are implemented here.
fn hash(
    password: &[u8],
    salt: &[u8],
    user: &[u8],
    budget: &OperationBudget,
) -> PdfResult<Zeroizing<[u8; 32]>> {
    let _key_lease = budget.reserve(64)?;
    let mut initial = Sha256::new();
    initial.update(password);
    initial.update(salt);
    initial.update(user);
    let mut key = Zeroizing::new(initial.finalize().to_vec());
    // Last encrypted byte is <=255, so the specified test must stop by 287.
    for round in 1usize..=288 {
        let unit = password.len() + key.len() + user.len();
        let (mut encrypted, _lease) = secret_buffer(unit * 64, budget)?;
        for block in encrypted.chunks_exact_mut(unit) {
            let split = password.len();
            block[..split].copy_from_slice(password);
            block[split..split + key.len()].copy_from_slice(&key);
            block[split + key.len()..].copy_from_slice(user);
        }
        let length = encrypted.len();
        cbc::Encryptor::<aes::Aes128>::new_from_slices(&key[..16], &key[16..32])
            .map_err(|_| malformed("R6 intermediate key"))?
            .encrypt_padded_mut::<NoPadding>(&mut encrypted, length)
            .map_err(|_| malformed("R6 intermediate block alignment"))?;
        let choice = encrypted[..16]
            .iter()
            .map(|v| usize::from(*v))
            .sum::<usize>()
            % 3;
        let last = usize::from(*encrypted.last().unwrap());
        let next = match choice {
            0 => Sha256::digest(&*encrypted).to_vec(),
            1 => Sha384::digest(&*encrypted).to_vec(),
            _ => Sha512::digest(&*encrypted).to_vec(),
        };
        key.zeroize();
        *key = next;
        if round >= 64 && last <= round - 32 {
            let mut result = Zeroizing::new([0; 32]);
            result.copy_from_slice(&key[..32]);
            return Ok(result);
        }
    }
    Err(malformed("R6 hashing termination"))
}

fn wrapped_key(key: &[u8], file_key: &[u8], encrypt: bool) -> PdfResult<Zeroizing<[u8; 32]>> {
    let mut result = Zeroizing::new([0; 32]);
    result.copy_from_slice(file_key);
    if encrypt {
        cbc::Encryptor::<aes::Aes256>::new_from_slices(key, &[0; 16])
            .map_err(|_| malformed("R6 key wrapping"))?
            .encrypt_padded_mut::<NoPadding>(&mut *result, 32)
            .map_err(|_| malformed("R6 key wrapping"))?;
    } else {
        cbc::Decryptor::<aes::Aes256>::new_from_slices(key, &[0; 16])
            .map_err(|_| malformed("R6 key unwrapping"))?
            .decrypt_padded_mut::<NoPadding>(&mut *result)
            .map_err(|_| malformed("R6 key unwrapping"))?;
    }
    Ok(result)
}

fn bytes<const N: usize>(dict: &PdfDict, field: &str) -> PdfResult<[u8; N]> {
    dict.get(field)
        .and_then(PdfObject::as_string)
        .and_then(|value| value.bytes().try_into().ok())
        .ok_or_else(|| malformed(field))
}

fn method(dict: &PdfDict, selector: &str, default: Option<&str>) -> PdfResult<Method> {
    let selected = match dict.get(selector) {
        None | Some(PdfObject::PdfNull) => default.unwrap_or("Identity"),
        Some(value) => value.as_name().ok_or_else(|| malformed(selector))?.name(),
    };
    method_named(dict, selected)
}

fn method_named(dict: &PdfDict, selected: &str) -> PdfResult<Method> {
    if selected == "Identity" {
        return Ok(Method::Identity);
    }
    let filter = dict
        .get("CF")
        .and_then(PdfObject::as_dict)
        .and_then(|filters| filters.get(selected))
        .and_then(PdfObject::as_dict)
        .ok_or_else(|| unsupported("crypt filter must be a direct dictionary"))?;
    let algorithm = filter
        .get("CFM")
        .and_then(PdfObject::as_name)
        .ok_or_else(|| malformed("crypt-filter CFM"))?
        .name();
    if let Some(event) = filter.get("AuthEvent") {
        if event.as_name().is_none_or(|name| name.name() != "DocOpen") {
            return Err(unsupported("crypt-filter AuthEvent"));
        }
    }
    match algorithm {
        "None" => Ok(Method::Identity),
        "AESV3" => {
            if filter
                .get("Length")
                .is_some_and(|length| length.as_u32() != Some(32))
            {
                return Err(malformed("AESV3 filter Length must be 32 bytes"));
            }
            Ok(Method::Aes256)
        }
        _ => Err(unsupported(
            "crypt-filter method (only AESV3 and Identity supported)",
        )),
    }
}

impl SecurityHandler {
    pub(crate) fn named_method(&self, name: &str) -> PdfResult<Method> {
        method_named(&self.dictionary, name)
    }
    /// Compatibility entry point for the old security-handler stub. R=6 key
    /// derivation does not use the file ID, but the document validates its ID.
    pub fn try_new(encrypt: &PdfDict, _ids: PdfArray, password: Option<&[u8]>) -> PdfResult<Self> {
        Self::authenticate(
            encrypt,
            password,
            Limits::default(),
            &Limits::default().operation_budget(),
        )
    }

    pub(crate) fn authenticate(
        dict: &PdfDict,
        password: Option<&[u8]>,
        limits: Limits,
        budget: &OperationBudget,
    ) -> PdfResult<Self> {
        if dict
            .get("Filter")
            .and_then(PdfObject::as_name)
            .is_none_or(|name| name.name() != "Standard")
            || dict.get("V").and_then(PdfObject::as_u32) != Some(5)
            || dict.get("R").and_then(PdfObject::as_u32) != Some(6)
        {
            return Err(unsupported("requires Standard handler V=5 / R=6"));
        }
        if dict.get("SubFilter").is_some() {
            return Err(unsupported("security SubFilter"));
        }
        if dict.get("Length").and_then(PdfObject::as_u32) != Some(256) {
            return Err(malformed("R6 Length must be 256 bits"));
        }
        let strings = method(dict, "StrF", None)?;
        let streams = method(dict, "StmF", None)?;
        let stream_name = dict
            .get("StmF")
            .and_then(PdfObject::as_name)
            .map(|name| name.name());
        let embedded_files = method(dict, "EFF", stream_name)?;
        let o = bytes::<48>(dict, "O")?;
        let u = bytes::<48>(dict, "U")?;
        let oe = bytes::<32>(dict, "OE")?;
        let ue = bytes::<32>(dict, "UE")?;
        let perms = bytes::<16>(dict, "Perms")?;
        let bits = dict
            .get("P")
            .and_then(PdfObject::as_number)
            .ok_or_else(|| malformed("P"))?
            .as_i32_checked()
            .map_err(|_| malformed("P must be signed i32"))?;
        let permissions = Permissions::from_bits(bits)?;
        let encrypt_metadata = match dict.get("EncryptMetadata") {
            None => true,
            Some(PdfObject::PdfBool(value)) => value.value(),
            _ => return Err(malformed("EncryptMetadata must be boolean")),
        };
        let normalized = normalized_password(password.unwrap_or(b""))?;
        let owner_hash = hash(&normalized, &o[32..40], &u, budget)?;
        let (authentication, key) = if bool::from(owner_hash.as_slice().ct_eq(&o[..32])) {
            let key = hash(&normalized, &o[40..48], &u, budget)?;
            (Authentication::Owner, wrapped_key(&*key, &oe, false)?)
        } else {
            let user_hash = hash(&normalized, &u[32..40], &[], budget)?;
            if !bool::from(user_hash.as_slice().ct_eq(&u[..32])) {
                return Err(if password.is_none() {
                    PdfError::PasswordRequired
                } else {
                    PdfError::InvalidPassword
                });
            }
            let key = hash(&normalized, &u[40..48], &[], budget)?;
            (Authentication::User, wrapped_key(&*key, &ue, false)?)
        };
        let mut decrypted = aes::cipher::Block::<aes::Aes256>::clone_from_slice(&perms);
        aes::Aes256::new_from_slice(&*key)
            .map_err(|_| malformed("R6 file key"))?
            .decrypt_block(&mut decrypted);
        let mut expected = [0xff; 12];
        expected[..4].copy_from_slice(&bits.to_le_bytes());
        expected[8] = if encrypt_metadata { b'T' } else { b'F' };
        expected[9..12].copy_from_slice(b"adb");
        if !bool::from(decrypted[..12].ct_eq(&expected)) {
            decrypted.zeroize();
            return Err(malformed("Perms authentication mismatch"));
        }
        decrypted.zeroize();
        let (dictionary, _lease) = crate::objects::clone_dict_with_budget(dict, budget, limits, 0)?;
        Ok(Self {
            dictionary,
            key,
            strings,
            streams,
            embedded_files,
            report: SecurityReport {
                authentication,
                permissions,
                encrypt_metadata,
                strings_use_aes256: strings == Method::Aes256,
                streams_use_aes256: streams == Method::Aes256,
                embedded_files_use_aes256: embedded_files == Method::Aes256,
            },
        })
    }

    pub fn report(&self) -> SecurityReport {
        self.report
    }

    pub(crate) fn generate(
        options: &EncryptionOptions,
        limits: Limits,
        budget: &OperationBudget,
    ) -> PdfResult<(Self, MemoryLease)> {
        if !options.permissions.accessibility {
            return Err(unsupported("PDF 2.0 output cannot restrict accessibility"));
        }
        // A fixed conservative envelope reserves before constructing these
        // bounded public dictionaries; their final clone is charged exactly.
        let _setup = budget.reserve(4096 + 64 * std::mem::size_of::<(String, PdfObject)>())?;
        let mut key = Zeroizing::new([0; 32]);
        random(&mut *key)?;
        let mut salts = [0u8; 32];
        random(&mut salts)?;
        let mut u = [0u8; 48];
        u[..32].copy_from_slice(&*hash(&options.user, &salts[..8], &[], budget)?);
        u[32..].copy_from_slice(&salts[..16]);
        let ue = wrapped_key(
            &*hash(&options.user, &salts[8..16], &[], budget)?,
            &*key,
            true,
        )?;
        let mut o = [0u8; 48];
        o[..32].copy_from_slice(&*hash(&options.owner, &salts[16..24], &u, budget)?);
        o[32..].copy_from_slice(&salts[16..]);
        let oe = wrapped_key(
            &*hash(&options.owner, &salts[24..], &u, budget)?,
            &*key,
            true,
        )?;
        let bits = options.permissions.bits();
        let mut perms = [0xff; 16];
        perms[..4].copy_from_slice(&bits.to_le_bytes());
        perms[8] = if options.encrypt_metadata { b'T' } else { b'F' };
        perms[9..12].copy_from_slice(b"adb");
        random(&mut perms[12..])?;
        let mut block = aes::cipher::Block::<aes::Aes256>::clone_from_slice(&perms);
        aes::Aes256::new_from_slice(&*key)
            .map_err(|_| malformed("R6 file key"))?
            .encrypt_block(&mut block);
        let mut dict = PdfDict::default();
        for (name, value) in [("V", 5), ("R", 6), ("Length", 256)] {
            dict.try_insert(
                name.into(),
                PdfObject::PdfNumber(PdfNumber::new(value as f32)),
            )?;
        }
        dict.try_insert(
            "P".into(),
            PdfObject::PdfNumber(PdfNumber::new_from_lexeme(bits.to_string().as_bytes())?),
        )?;
        for (name, value) in [
            ("O", o.as_slice()),
            ("U", u.as_slice()),
            ("OE", oe.as_slice()),
            ("UE", ue.as_slice()),
            ("Perms", block.as_slice()),
        ] {
            dict.try_insert(
                name.into(),
                PdfObject::PdfString(PdfString::new(value.to_vec(), true)),
            )?;
        }
        dict.try_insert(
            "EncryptMetadata".into(),
            PdfObject::PdfBool(PdfBool::new(if options.encrypt_metadata {
                b"true"
            } else {
                b"false"
            })),
        )?;
        let mut filter = PdfDict::default();
        filter.try_insert(
            "CFM".into(),
            PdfObject::PdfName(PdfName::new("AESV3".into())),
        )?;
        filter.try_insert("Length".into(), PdfObject::PdfNumber(PdfNumber::new(32.0)))?;
        filter.try_insert(
            "AuthEvent".into(),
            PdfObject::PdfName(PdfName::new("DocOpen".into())),
        )?;
        let mut filters = PdfDict::default();
        filters.try_insert("StdCF".into(), PdfObject::PdfDict(filter))?;
        dict.try_insert("CF".into(), PdfObject::PdfDict(filters))?;
        for (field, name) in [
            ("Filter", "Standard"),
            ("StmF", "StdCF"),
            ("StrF", "StdCF"),
            ("EFF", "StdCF"),
        ] {
            dict.try_insert(field.into(), PdfObject::PdfName(PdfName::new(name.into())))?;
        }
        let (dictionary, lease) = crate::objects::clone_dict_with_budget(&dict, budget, limits, 0)?;
        Ok((
            Self {
                dictionary,
                key,
                strings: Method::Aes256,
                streams: Method::Aes256,
                embedded_files: Method::Aes256,
                report: SecurityReport {
                    authentication: Authentication::Owner,
                    permissions: options.permissions,
                    encrypt_metadata: options.encrypt_metadata,
                    strings_use_aes256: true,
                    streams_use_aes256: true,
                    embedded_files_use_aes256: true,
                },
            },
            lease,
        ))
    }
}
