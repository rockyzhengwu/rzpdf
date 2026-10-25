//! Object-boundary encryption/decryption. Compressed objects are never passed
//! through this module: their containing object stream is decrypted once.
use super::{
    SecurityHandler,
    r6::{Method, random},
};
use crate::{
    budget::{BudgetedBytes, MemoryLease, OperationBudget},
    error::{PdfError, PdfResult},
    objects::{
        PdfObject,
        object_id::ObjectId,
        pdf_array::PdfArray,
        pdf_dict::{BudgetedDictBuilder, PdfDict},
        pdf_name::PdfName,
        pdf_stream::{
            PdfStream, resolve_filter_setting_with_budget, resolve_setting_reference_with_budget,
        },
    },
    pdf_context::PDFContext,
};
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use std::io::{Read, Seek};
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Copy)]
struct ObjectCrypt<'a> {
    key: &'a [u8],
    encrypt: bool,
}

fn malformed(message: &str) -> PdfError {
    PdfError::MalformedSecurity(message.into())
}

fn crypt_bytes(
    method: Method,
    key: &[u8],
    input: &[u8],
    encrypt: bool,
    limit: usize,
    budget: &OperationBudget,
) -> PdfResult<(Vec<u8>, MemoryLease)> {
    if method == Method::Identity {
        if input.len() > limit {
            return Err(PdfError::ResourceLimit("security payload bytes".into()));
        }
        return Ok(BudgetedBytes::copy_from_slice(budget, input)?.into_reserved_parts());
    }
    let length = if encrypt {
        input
            .len()
            .checked_div(16)
            .and_then(|n| n.checked_add(1))
            .and_then(|n| n.checked_mul(16))
            .and_then(|n| n.checked_add(16))
            .ok_or_else(|| PdfError::ResourceLimit("AES ciphertext length".into()))?
    } else {
        if input.len() < 32 || !(input.len() - 16).is_multiple_of(16) {
            return Err(malformed("IV-prefixed AES ciphertext length"));
        }
        input.len() - 16
    };
    if length > limit {
        return Err(PdfError::ResourceLimit("AES payload bytes".into()));
    }
    let mut lease = budget.reserve(length)?;
    let mut output = Zeroizing::new(Vec::new());
    output
        .try_reserve_exact(length)
        .map_err(|_| PdfError::ResourceLimit("AES payload allocation".into()))?;
    lease.resize(output.capacity())?;
    output.resize(length, 0);
    if encrypt {
        let (iv, payload) = output.split_at_mut(16);
        random(iv)?;
        payload[..input.len()].copy_from_slice(input);
        if method != Method::Aes256 {
            return Err(PdfError::UnsupportedSecurity(
                "R4 AES-128 is read-only".into(),
            ));
        }
        cbc::Encryptor::<aes::Aes256>::new_from_slices(key, iv)
            .map_err(|_| malformed("AES key or IV"))?
            .encrypt_padded_mut::<Pkcs7>(payload, input.len())
            .map_err(|_| malformed("AES encryption buffer"))?;
    } else {
        output.copy_from_slice(&input[16..]);
        let result = if method == Method::Aes128 {
            cbc::Decryptor::<aes::Aes128>::new_from_slices(key, &input[..16])
                .map_err(|_| malformed("AES-128 key or IV"))?
                .decrypt_padded_mut::<Pkcs7>(&mut output)
                .map(|data| data.len())
        } else {
            cbc::Decryptor::<aes::Aes256>::new_from_slices(key, &input[..16])
                .map_err(|_| malformed("AES key or IV"))?
                .decrypt_padded_mut::<Pkcs7>(&mut output)
                .map(|data| data.len())
        };
        match result {
            Ok(length) => output.truncate(length),
            Err(_) => {
                output.zeroize();
                return Err(malformed("AES ciphertext padding"));
            }
        }
    }
    Ok((std::mem::take(&mut *output), lease))
}

struct Settings {
    name: Option<String>,
    filter_count: usize,
    array: bool,
    params: Option<PdfObject>,
    _lease: MemoryLease,
}

fn settings<R: Read + Seek>(
    ctx: &PDFContext<R>,
    stream: &PdfStream,
    budget: &OperationBudget,
) -> PdfResult<Settings> {
    let mut lease = budget.reserve(0)?;
    let Some(filter) = stream
        .dict()
        .get("Filter")
        .filter(|value| !matches!(value, PdfObject::PdfNull))
    else {
        return Ok(Settings {
            name: None,
            filter_count: 0,
            array: false,
            params: None,
            _lease: lease,
        });
    };
    let (filter, mut filter_lease) =
        resolve_filter_setting_with_budget(filter, ctx, false, budget)?;
    lease.absorb(&mut filter_lease)?;
    let mut crypt = false;
    let (count, array) = match &filter {
        PdfObject::PdfNull => (0, false),
        PdfObject::PdfName(name) => {
            crypt = name.name() == "Crypt";
            (1, false)
        }
        PdfObject::PdfArray(array) => {
            for (index, value) in array.into_iter().enumerate() {
                let name = value
                    .as_name()
                    .ok_or_else(|| malformed("Filter array must contain names"))?;
                if name.name() == "Crypt" {
                    if index != 0 || crypt {
                        return Err(PdfError::UnsupportedSecurity(
                            "Crypt must be the unique first filter".into(),
                        ));
                    }
                    crypt = true;
                }
            }
            (array.len(), true)
        }
        _ => return Err(malformed("Filter must be a name or array")),
    };
    if !crypt {
        return Ok(Settings {
            name: None,
            filter_count: count,
            array,
            params: None,
            _lease: lease,
        });
    }
    let params = match stream.dict().get("DecodeParms") {
        Some(params) => {
            let (params, mut child) =
                resolve_filter_setting_with_budget(params, ctx, true, budget)?;
            lease.absorb(&mut child)?;
            Some(params)
        }
        None => None,
    };
    let first = match &params {
        None | Some(PdfObject::PdfNull) => None,
        Some(PdfObject::PdfDict(dict)) if !array || count == 1 => Some(dict),
        Some(PdfObject::PdfArray(items)) if array && items.len() == count => {
            match items.into_iter().next() {
                Some(PdfObject::PdfNull) => None,
                Some(PdfObject::PdfDict(dict)) => Some(dict),
                _ => return Err(malformed("Crypt DecodeParms entry")),
            }
        }
        _ => return Err(malformed("Crypt DecodeParms alignment")),
    };
    let (name, mut name_lease) = match first.and_then(|dict| dict.get("Name")) {
        Some(value) => {
            let (_lease, value) = {
                let (value, lease) = resolve_setting_reference_with_budget(value, ctx, budget)?;
                (lease, value)
            };
            let name = value
                .as_name()
                .ok_or_else(|| malformed("Crypt Name must be a name"))?;
            crate::budget::copy_string(name.name(), budget)?
        }
        None => crate::budget::copy_string("Identity", budget)?,
    };
    lease.absorb(&mut name_lease)?;
    Ok(Settings {
        name: Some(name),
        filter_count: count,
        array,
        params,
        _lease: lease,
    })
}

fn rewrite_crypt_name(
    stream: &PdfStream,
    settings: &Settings,
    name: &str,
    ctx_limits: crate::parser::options::Limits,
    budget: &OperationBudget,
) -> PdfResult<(PdfDict, MemoryLease)> {
    let old = match &settings.params {
        Some(PdfObject::PdfDict(dict)) => Some(dict),
        Some(PdfObject::PdfArray(items)) => items.into_iter().next().and_then(PdfObject::as_dict),
        _ => None,
    };
    let empty = PdfDict::default();
    let _token_lease = budget.reserve(name.len())?;
    let token = PdfObject::PdfName(PdfName::new(name.into()));
    let (dict, mut lease) = old
        .unwrap_or(&empty)
        .with_replaced_entry_with_budget("Name", &token, budget, ctx_limits)?;
    let parameter = PdfObject::PdfDict(dict);
    let parameter = if settings.array {
        let mut table = budget.reserve(
            settings
                .filter_count
                .checked_mul(std::mem::size_of::<PdfObject>())
                .ok_or_else(|| PdfError::ResourceLimit("Crypt parameters size".into()))?,
        )?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(settings.filter_count)
            .map_err(|_| PdfError::ResourceLimit("Crypt parameters allocation".into()))?;
        table.resize(values.capacity() * std::mem::size_of::<PdfObject>())?;
        lease.absorb(&mut table)?;
        values.push(parameter);
        for index in 1..settings.filter_count {
            let original = settings
                .params
                .as_ref()
                .and_then(PdfObject::as_array)
                .and_then(|items| items.into_iter().nth(index))
                .unwrap_or(&PdfObject::PdfNull);
            let (value, mut child) =
                crate::objects::clone_object_with_budget(original, budget, ctx_limits, 0)?;
            lease.absorb(&mut child)?;
            values.push(value);
        }
        PdfObject::PdfArray(PdfArray::from_items(values))
    } else {
        parameter
    };
    // parameter and lease stay alive until the complete replacement is built.
    let result = stream.dict().with_replaced_entry_with_budget(
        "DecodeParms",
        &parameter,
        budget,
        ctx_limits,
    )?;
    Ok(result)
}

pub(crate) fn plain_output<R: Read + Seek>(
    ctx: &PDFContext<R>,
    object: &PdfObject,
    budget: &OperationBudget,
) -> PdfResult<(PdfObject, MemoryLease)> {
    if let PdfObject::PdfStream(stream) = object
        && stream.crypt_processed()
    {
        let settings = settings(ctx, stream, budget)?;
        if settings.name.is_some() {
            let (dict, mut lease) =
                rewrite_crypt_name(stream, &settings, "Identity", ctx.limits(), budget)?;
            let (data, mut child) =
                BudgetedBytes::copy_from_slice(budget, stream.raw_data())?.into_reserved_parts();
            lease.absorb(&mut child)?;
            return Ok((PdfObject::PdfStream(PdfStream::new(dict, data)), lease));
        }
    }
    crate::objects::clone_object_with_budget(object, budget, ctx.limits(), 0)
}

impl SecurityHandler {
    /// Generated ObjStm payloads use the default stream filter, after member
    /// serialization and optional compression. Members are never encrypted
    /// independently; generated dictionaries contain no string values.
    pub(crate) fn encrypt_object_stream_bytes(
        &self,
        input: &[u8],
        limit: usize,
        budget: &OperationBudget,
    ) -> PdfResult<BudgetedBytes> {
        if self.revision != 6 {
            return Err(PdfError::UnsupportedSecurity(
                "R4 AES-128 is read-only".into(),
            ));
        }
        let (data, lease) = crypt_bytes(self.streams, &*self.key, input, true, limit, budget)?;
        BudgetedBytes::from_reserved_vec(data, lease)
    }

    pub(crate) fn transform_source<R: Read + Seek>(
        &self,
        ctx: &PDFContext<R>,
        object: &PdfObject,
        id: ObjectId,
        budget: &OperationBudget,
    ) -> PdfResult<(PdfObject, MemoryLease)> {
        if self.revision == 4 {
            let key = super::r4::object_key(&*self.key, id);
            self.transform_inner(
                ctx,
                object,
                ObjectCrypt {
                    key: &*key,
                    encrypt: false,
                },
                budget,
                0,
                &mut 0,
            )
        } else {
            self.transform(ctx, object, false, budget)
        }
    }

    pub(crate) fn transform<R: Read + Seek>(
        &self,
        ctx: &PDFContext<R>,
        object: &PdfObject,
        encrypt: bool,
        budget: &OperationBudget,
    ) -> PdfResult<(PdfObject, MemoryLease)> {
        if self.revision != 6 {
            return Err(PdfError::UnsupportedSecurity(
                "R4 AES-128 is read-only; decryption requires an object ID".into(),
            ));
        }
        self.transform_inner(
            ctx,
            object,
            ObjectCrypt {
                key: &*self.key,
                encrypt,
            },
            budget,
            0,
            &mut 0,
        )
    }

    fn transform_dict<R: Read + Seek>(
        &self,
        ctx: &PDFContext<R>,
        dict: &PdfDict,
        crypt: ObjectCrypt<'_>,
        budget: &OperationBudget,
        depth: usize,
        nodes: &mut usize,
    ) -> PdfResult<(PdfDict, MemoryLease)> {
        let mut builder = BudgetedDictBuilder::new(budget, ctx.limits().max_objects)?;
        for (key, value) in dict {
            let (key, key_lease) = crate::budget::copy_string(key, budget)?;
            let (value, lease) =
                self.transform_inner(ctx, value, crypt, budget, depth + 1, nodes)?;
            builder.insert(key, value, key_lease, lease)?;
        }
        builder.finish()
    }

    fn transform_inner<R: Read + Seek>(
        &self,
        ctx: &PDFContext<R>,
        object: &PdfObject,
        crypt: ObjectCrypt<'_>,
        budget: &OperationBudget,
        depth: usize,
        nodes: &mut usize,
    ) -> PdfResult<(PdfObject, MemoryLease)> {
        let ObjectCrypt {
            key: cipher_key,
            encrypt,
        } = crypt;
        let limits = ctx.limits();
        if depth >= limits.max_depth || *nodes >= limits.max_objects {
            return Err(PdfError::ResourceLimit(
                "security object nodes/depth".into(),
            ));
        }
        *nodes += 1;
        match object {
            PdfObject::PdfString(string) => {
                let (bytes, lease) = crypt_bytes(
                    self.strings,
                    cipher_key,
                    string.bytes(),
                    encrypt,
                    limits.max_token_bytes,
                    budget,
                )?;
                Ok((PdfObject::PdfString(string.with_bytes(bytes)), lease))
            }
            PdfObject::PdfDict(dict) => {
                if dict.contains_key("ByteRange")
                    || dict.contains_key("DocMDP")
                    || dict.contains_key("FieldMDP")
                    || dict.get("SigFlags").is_some_and(|v| v.as_u32() != Some(0))
                    || ["FT", "Type"].iter().any(|key| {
                        dict.get(key)
                            .and_then(PdfObject::as_name)
                            .is_some_and(|name| name.name() == "Sig")
                    })
                {
                    return Err(PdfError::UnsupportedSecurity(
                        "encrypted/signature combination".into(),
                    ));
                }
                let (dict, lease) = self.transform_dict(ctx, dict, crypt, budget, depth, nodes)?;
                Ok((PdfObject::PdfDict(dict), lease))
            }
            PdfObject::PdfArray(items) => {
                let mut lease = budget.reserve(
                    items
                        .len()
                        .checked_mul(std::mem::size_of::<PdfObject>())
                        .ok_or_else(|| PdfError::ResourceLimit("security array size".into()))?,
                )?;
                let mut values = Vec::new();
                values
                    .try_reserve_exact(items.len())
                    .map_err(|_| PdfError::ResourceLimit("security array allocation".into()))?;
                lease.resize(values.capacity() * std::mem::size_of::<PdfObject>())?;
                for value in items {
                    let (value, mut child) =
                        self.transform_inner(ctx, value, crypt, budget, depth + 1, nodes)?;
                    lease.absorb(&mut child)?;
                    values.push(value);
                }
                Ok((PdfObject::PdfArray(PdfArray::from_items(values)), lease))
            }
            PdfObject::PdfStream(stream) => {
                if depth != 0 {
                    return Err(malformed("nested direct stream"));
                }
                let (_type_lease, ty) = {
                    let (value, lease) = match stream.dict().get("Type") {
                        Some(value) => resolve_setting_reference_with_budget(value, ctx, budget)?,
                        None => (PdfObject::PdfNull, budget.reserve(0)?),
                    };
                    (lease, value)
                };
                let ty = ty.as_name().map(|name| name.name());
                if ty == Some("XRef") {
                    return crate::objects::clone_object_with_budget(object, budget, limits, 0);
                }
                if ty == Some("Metadata") && !self.report.encrypt_metadata {
                    return if encrypt {
                        plain_output(ctx, object, budget)
                    } else {
                        crate::objects::clone_object_with_budget(object, budget, limits, 0)
                    };
                }
                let settings = settings(ctx, stream, budget)?;
                let mut replacement = None;
                let method = if let Some(name) = settings.name.as_deref() {
                    match self.named_method(name) {
                        Ok(method) => method,
                        Err(_)
                            if encrypt
                                && stream.crypt_processed()
                                && ctx.source_security().is_some_and(|handler| {
                                    handler.named_method(name).ok() == Some(Method::Aes256)
                                }) =>
                        {
                            let method = self.named_method("StdCF")?;
                            replacement = Some(rewrite_crypt_name(
                                stream, &settings, "StdCF", limits, budget,
                            )?);
                            method
                        }
                        Err(error) => return Err(error),
                    }
                } else if ty == Some("EmbeddedFile") {
                    self.embedded_files
                } else {
                    self.streams
                };
                let dict = replacement
                    .as_ref()
                    .map(|(dict, _)| dict)
                    .unwrap_or(stream.dict());
                let (dict, mut lease) =
                    self.transform_dict(ctx, dict, crypt, budget, depth, nodes)?;
                let (data, mut data_lease) = crypt_bytes(
                    method,
                    cipher_key,
                    stream.raw_data(),
                    encrypt,
                    limits.max_raw_stream_bytes,
                    budget,
                )?;
                lease.absorb(&mut data_lease)?;
                let mut result = PdfStream::new(dict, data);
                if !encrypt && settings.name.is_some() {
                    result.mark_crypt_processed();
                }
                Ok((PdfObject::PdfStream(result), lease))
            }
            _ => crate::objects::clone_object_with_budget(object, budget, limits, depth),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{document::PdfDocument, security::EncryptionOptions};

    #[test]
    fn object_stream_ciphertext_limits_include_iv_and_padding_and_release() {
        let pool = OperationBudget::new(4 * 1024 * 1024);
        let (handler, _lease) = SecurityHandler::generate(
            &EncryptionOptions::new("user", "owner").unwrap(),
            crate::parser::options::Limits::default(),
            &pool,
        )
        .unwrap();
        let live = pool.live_bytes();
        for (length, ciphertext) in [(0, 32), (15, 32), (16, 48), (31, 48), (32, 64)] {
            let input = vec![42; length];
            assert!(matches!(
                handler.encrypt_object_stream_bytes(&input, ciphertext - 1, &pool),
                Err(PdfError::ResourceLimit(_))
            ));
            assert_eq!(pool.live_bytes(), live);
            let bytes = handler
                .encrypt_object_stream_bytes(&input, ciphertext, &pool)
                .unwrap();
            assert_eq!(bytes.as_slice().len(), ciphertext);
            let (plain, lease) = crypt_bytes(
                handler.streams,
                &*handler.key,
                bytes.as_slice(),
                false,
                ciphertext,
                &pool,
            )
            .unwrap();
            assert_eq!(plain, input);
            drop(plain);
            drop(lease);
            drop(bytes);
            assert_eq!(pool.live_bytes(), live);
        }
    }

    #[test]
    fn explicit_crypt_is_processed_once_and_plain_output_uses_identity() {
        let doc = PdfDocument::new().unwrap();
        let budget = OperationBudget::new(2 * 1024 * 1024);
        let (handler, _lease) = SecurityHandler::generate(
            &EncryptionOptions::new("user", "owner").unwrap(),
            doc.ctx().limits(),
            &budget,
        )
        .unwrap();
        for name in ["StdCF", "Identity"] {
            for length in [0, 1, 15, 16, 17, 32] {
                let mut dict = PdfDict::default();
                dict.try_insert(
                    "Filter".into(),
                    PdfObject::PdfName(PdfName::new("Crypt".into())),
                )
                .unwrap();
                let mut params = PdfDict::default();
                params
                    .try_insert("Name".into(), PdfObject::PdfName(PdfName::new(name.into())))
                    .unwrap();
                dict.try_insert("DecodeParms".into(), PdfObject::PdfDict(params))
                    .unwrap();
                let bytes = vec![42; length];
                let object = PdfObject::PdfStream(PdfStream::new(dict, bytes.clone()));
                let (encrypted, _cipher_lease) = handler
                    .transform(doc.ctx(), &object, true, &budget)
                    .unwrap();
                assert_eq!(
                    encrypted.as_stream().unwrap().raw_data().len(),
                    if name == "Identity" {
                        length
                    } else {
                        16 + (length / 16 + 1) * 16
                    }
                );
                let (plain, _plain_lease) = handler
                    .transform(doc.ctx(), &encrypted, false, &budget)
                    .unwrap();
                assert_eq!(
                    plain.as_stream().unwrap().decode_data(doc.ctx()).unwrap(),
                    bytes
                );
                let (removed, _removed_lease) = plain_output(doc.ctx(), &plain, &budget).unwrap();
                assert!(!removed.as_stream().unwrap().crypt_processed());
                assert_eq!(
                    removed.as_stream().unwrap().decode_data(doc.ctx()).unwrap(),
                    bytes
                );
            }
        }
    }

    #[test]
    fn invalid_aes_length_and_padding_release_temporary_reservations() {
        let budget = OperationBudget::new(4096);
        assert!(crypt_bytes(Method::Aes256, &[0; 32], &[0; 31], false, 1024, &budget).is_err());
        assert_eq!(budget.live_bytes(), 0);
        let (mut ciphertext, lease) =
            crypt_bytes(Method::Aes256, &[0; 32], &[42; 15], true, 1024, &budget).unwrap();
        ciphertext[15] ^= 1; // PKCS#7's single padding byte becomes zero.
        assert!(matches!(
            crypt_bytes(Method::Aes256, &[0; 32], &ciphertext, false, 1024, &budget),
            Err(PdfError::MalformedSecurity(_))
        ));
        assert_eq!(budget.live_bytes(), lease.bytes());
        drop(ciphertext);
        drop(lease);
        assert_eq!(budget.live_bytes(), 0);
    }
}
