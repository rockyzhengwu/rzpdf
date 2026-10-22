//! Read-only Standard revision 4 / AESV2 interoperability.
use super::{Authentication, Permissions, SecurityHandler, SecurityReport, r6::method};
use crate::{
    budget::OperationBudget,
    error::{PdfError, PdfResult},
    objects::{PdfObject, object_id::ObjectId, pdf_dict::PdfDict},
    parser::options::Limits,
};
use md5::{Digest, Md5};
use rc4::{KeyInit, Rc4, StreamCipher, consts::U16};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;
const PAD: [u8; 32] = [
    0x28, 0xbf, 0x4e, 0x5e, 0x4e, 0x75, 0x8a, 0x41, 0x64, 0x00, 0x4e, 0x56, 0xff, 0xfa, 0x01, 0x08,
    0x2e, 0x2e, 0x00, 0xb6, 0xd0, 0x68, 0x3e, 0x80, 0x2f, 0x0c, 0xa9, 0xfe, 0x64, 0x53, 0x69, 0x7a,
];
fn malformed(s: &str) -> PdfError {
    PdfError::MalformedSecurity(s.into())
}
fn padded(password: &[u8]) -> Zeroizing<[u8; 32]> {
    let mut p = Zeroizing::new(PAD);
    let n = password.len().min(32);
    p[..n].copy_from_slice(&password[..n]);
    if n < 32 {
        p[n..].copy_from_slice(&PAD[..32 - n]);
    }
    p
}
fn rc4(key: &[u8; 16], data: &mut [u8]) {
    Rc4::<U16>::new(key.into()).apply_keystream(data);
}
fn file_key(
    password: &[u8],
    owner: &[u8; 32],
    permissions: i32,
    id: &[u8],
    metadata: bool,
) -> Zeroizing<[u8; 16]> {
    let mut h = Md5::new();
    h.update(&*padded(password));
    h.update(owner);
    h.update(permissions.to_le_bytes());
    h.update(id);
    if !metadata {
        h.update([255; 4]);
    }
    let mut digest = Zeroizing::new(<[u8; 16]>::from(h.finalize()));
    for _ in 0..50 {
        *digest = Md5::digest(&*digest).into();
    }
    digest
}
fn user_valid(key: &[u8; 16], user: &[u8; 32], id: &[u8]) -> bool {
    let mut h = Md5::new();
    h.update(PAD);
    h.update(id);
    let mut value = Zeroizing::new(<[u8; 16]>::from(h.finalize()));
    rc4(key, &mut *value);
    for i in 1..=19 {
        let k = Zeroizing::new(key.map(|b| b ^ i));
        rc4(&k, &mut *value);
    }
    bool::from(value.as_slice().ct_eq(&user[..16]))
}
fn owner_user_password(password: &[u8], owner: &[u8; 32]) -> Zeroizing<[u8; 32]> {
    let mut digest = Zeroizing::new(<[u8; 16]>::from(Md5::digest(&*padded(password))));
    for _ in 0..50 {
        *digest = Md5::digest(&*digest).into();
    }
    let mut value = Zeroizing::new(*owner);
    for i in (0..=19).rev() {
        let key = Zeroizing::new(digest.map(|b| b ^ i));
        rc4(&key, &mut *value);
    }
    value
}
pub(super) fn object_key(key: &[u8], id: ObjectId) -> Zeroizing<[u8; 16]> {
    let mut hash = Md5::new();
    hash.update(&key[..16]);
    hash.update(&id.obj_num().to_le_bytes()[..3]);
    hash.update(&id.gen_num().to_le_bytes()[..2]);
    hash.update(b"sAlT");
    Zeroizing::new(hash.finalize().into())
}
pub(super) fn authenticate(
    dict: &PdfDict,
    password: Option<&[u8]>,
    id: Option<&[u8]>,
    limits: Limits,
    budget: &OperationBudget,
) -> PdfResult<SecurityHandler> {
    let _scratch = budget.reserve(4096)?;
    if dict.get("SubFilter").is_some()
        || dict.get("Length").and_then(PdfObject::as_u32) != Some(128)
    {
        return Err(PdfError::UnsupportedSecurity(
            "R4 reader requires a 128-bit Standard AESV2 handler without SubFilter".into(),
        ));
    }
    let id = id
        .filter(|id| !id.is_empty() && id.len() <= limits.max_token_bytes)
        .ok_or_else(|| malformed("R4 requires a nonempty first file ID"))?;
    let password_bytes = password.unwrap_or(b"");
    if !password_bytes.is_ascii() || password_bytes.len() > 1024 {
        return Err(PdfError::UnsupportedSecurity(
            "R4 password API currently supports ASCII passwords up to 1024 input bytes".into(),
        ));
    }
    let strings = method(dict, "StrF", None, 4)?;
    let streams = method(dict, "StmF", None, 4)?;
    let stream_name = dict
        .get("StmF")
        .and_then(PdfObject::as_name)
        .map(|n| n.name());
    let embedded_files = method(dict, "EFF", stream_name, 4)?;
    let field = |name: &str| -> PdfResult<[u8; 32]> {
        dict.get(name)
            .and_then(PdfObject::as_string)
            .and_then(|s| s.bytes().try_into().ok())
            .ok_or_else(|| malformed(name))
    };
    let owner = field("O")?;
    let user = field("U")?;
    let bits = dict
        .get("P")
        .and_then(PdfObject::as_number)
        .ok_or_else(|| malformed("P"))?
        .as_i32_checked()
        .map_err(|_| malformed("P must be signed i32"))?;
    let permissions = Permissions::from_bits(bits)?;
    let metadata = match dict.get("EncryptMetadata") {
        None => true,
        Some(PdfObject::PdfBool(b)) => b.value(),
        _ => return Err(malformed("EncryptMetadata must be boolean")),
    };
    let mut key = file_key(password_bytes, &owner, bits, id, metadata);
    let authentication = if user_valid(&key, &user, id) {
        Authentication::User
    } else {
        let user_password = owner_user_password(password_bytes, &owner);
        key = file_key(&*user_password, &owner, bits, id, metadata);
        if !user_valid(&key, &user, id) {
            return Err(if password.is_none() {
                PdfError::PasswordRequired
            } else {
                PdfError::InvalidPassword
            });
        }
        Authentication::Owner
    };
    let mut file_key = Zeroizing::new([0; 32]);
    file_key[..16].copy_from_slice(&*key);
    let (dictionary, _lease) = crate::objects::clone_dict_with_budget(dict, budget, limits, 0)?;
    Ok(SecurityHandler {
        dictionary,
        key: file_key,
        revision: 4,
        strings,
        streams,
        embedded_files,
        report: SecurityReport {
            revision: 4,
            authentication,
            permissions,
            encrypt_metadata: metadata,
            strings_use_aes256: false,
            streams_use_aes256: false,
            embedded_files_use_aes256: false,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aesv2_object_key_uses_object_number_generation_and_salt() {
        // Independently calculated with Python hashlib, not an R4 roundtrip.
        let key = object_key(&(0..16).collect::<Vec<u8>>(), ObjectId::new(42, 7));
        assert_eq!(
            *key,
            [
                0xe5, 0x89, 0x97, 0xc1, 0x58, 0x74, 0xe1, 0x93, 0xbb, 0x01, 0xf8, 0x82, 0x3e, 0xc5,
                0x13, 0x14
            ]
        );
        assert_ne!(
            *key,
            *object_key(&(0..16).collect::<Vec<u8>>(), ObjectId::new(42, 0))
        );
    }
}
