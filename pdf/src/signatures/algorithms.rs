//! Bounded ASN.1 algorithm profiles; cryptographic operations stay in ring.
use super::*;
use der::{Any, Decode, Reader, Tag, TagNumber, Tagged, asn1::ObjectIdentifier};

pub(super) enum Scheme {
    Rsa,
    Pss,
    Ec,
}

fn oid(value: &str) -> ObjectIdentifier {
    ObjectIdentifier::new(value).unwrap()
}

pub(super) fn scheme(signer: &cms::signed_data::SignerInfo, extended: bool) -> PdfResult<Scheme> {
    let algorithm = &signer.signature_algorithm;
    if [oid("1.2.840.113549.1.1.1"), oid("1.2.840.113549.1.1.11")].contains(&algorithm.oid) {
        if !valid_algorithm_parameters(&algorithm.parameters) {
            return Err(invalid("RSA algorithm parameters"));
        }
        return Ok(Scheme::Rsa);
    }
    if !extended {
        return Err(invalid("extended signature algorithm requires PDF 2.0"));
    }
    if algorithm.oid == oid("1.2.840.113549.1.1.10") {
        let parameters = algorithm
            .parameters
            .as_ref()
            .ok_or_else(|| invalid("explicit PSS parameters required"))?;
        if !pss(parameters)? {
            return Err(invalid("PSS requires SHA-256/MGF1 SHA-256/salt32/trailer1"));
        }
        return Ok(Scheme::Pss);
    }
    if algorithm.oid == oid("1.2.840.10045.4.3.2") && algorithm.parameters.is_none() {
        return Ok(Scheme::Ec);
    }
    Err(invalid("unsupported signature algorithm or parameters"))
}

fn algorithm(any: &Any) -> der::Result<(ObjectIdentifier, Option<Any>)> {
    any.sequence(|reader| {
        let id = reader.decode::<ObjectIdentifier>()?;
        let parameters = if reader.is_finished() {
            None
        } else {
            Some(reader.decode::<Any>()?)
        };
        Ok((id, parameters))
    })
}

fn hash(any: &Any) -> der::Result<bool> {
    let (id, parameters) = algorithm(any)?;
    Ok(id == oid("2.16.840.1.101.3.4.2.1") && valid_algorithm_parameters(&parameters))
}

fn pss(parameters: &Any) -> PdfResult<bool> {
    let parsed = parameters
        .sequence(|reader| {
            let mut fields = Vec::new();
            // At most four standard parameters; excess data is rejected by the
            // sequence reader's finish check, not allocated without a bound.
            while !reader.is_finished() && fields.len() < 4 {
                fields.push(reader.decode::<Any>()?);
            }
            Ok(fields)
        })
        .map_err(|_| malformed("PSS parameter sequence"))?;
    if parsed.len() != 3 {
        return Ok(false);
    }
    for (value, number) in parsed
        .iter()
        .zip([TagNumber::N0, TagNumber::N1, TagNumber::N2])
    {
        if value.tag()
            != (Tag::ContextSpecific {
                constructed: true,
                number,
            })
        {
            return Ok(false);
        }
    }
    let hash_algorithm =
        Any::from_der(parsed[0].value()).map_err(|_| malformed("PSS hash algorithm"))?;
    if !hash(&hash_algorithm).map_err(|_| malformed("PSS hash parameters"))? {
        return Ok(false);
    }
    let mask = Any::from_der(parsed[1].value()).map_err(|_| malformed("PSS MGF algorithm"))?;
    let (id, parameters) = algorithm(&mask).map_err(|_| malformed("PSS MGF parameters"))?;
    if id != oid("1.2.840.113549.1.1.8") {
        return Ok(false);
    }
    let Some(parameters) = parameters else {
        return Ok(false);
    };
    if !hash(&parameters).map_err(|_| malformed("PSS MGF hash"))? {
        return Ok(false);
    }
    let salt = u32::from_der(parsed[2].value()).map_err(|_| malformed("PSS salt integer"))?;
    // Omitted trailerField has the DER DEFAULT value 1. Explicit DEFAULT or
    // unknown trailing parameters are not accepted as this canonical profile.
    Ok(salt == 32)
}

pub(super) fn rsa_key_profile(bytes: &[u8]) -> PdfResult<()> {
    let key = der::AnyRef::from_der(bytes).map_err(|_| malformed("RSA public key DER"))?;
    let bits = key
        .sequence(|reader| {
            let modulus = reader.decode::<der::asn1::UintRef<'_>>()?;
            let _exponent = reader.decode::<der::asn1::UintRef<'_>>()?;
            let bytes = modulus.as_bytes();
            Ok(bytes
                .len()
                .saturating_mul(8)
                .saturating_sub(bytes.first().map_or(0, |b| b.leading_zeros() as usize)))
        })
        .map_err(|_| malformed("RSA public key integers"))?;
    if !(2048..=8192).contains(&bits) {
        return Err(invalid("RSA modulus outside 2048..8192 bits"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pss_profile_rejects_defaults_wrong_salt_duplicates_and_mgf_hash() {
        // SHA256, MGF1(SHA256), salt32, omitted DEFAULT trailer1.
        let bytes = [
            0x30, 0x34, 0xa0, 0x0f, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03,
            0x04, 0x02, 0x01, 0x05, 0x00, 0xa1, 0x1c, 0x30, 0x1a, 0x06, 0x09, 0x2a, 0x86, 0x48,
            0x86, 0xf7, 0x0d, 0x01, 0x01, 0x08, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01,
            0x65, 0x03, 0x04, 0x02, 0x01, 0x05, 0x00, 0xa2, 0x03, 0x02, 0x01, 0x20,
        ];
        assert!(pss(&Any::from_der(&bytes).unwrap()).unwrap());
        assert!(!pss(&Any::from_der(b"\x30\x00").unwrap()).unwrap());
        for (index, replacement) in [(53, 20), (19, 0xa0), (43, 2)] {
            let mut changed = bytes;
            changed[index] = replacement;
            assert!(!pss(&Any::from_der(&changed).unwrap()).unwrap());
        }
    }
}
