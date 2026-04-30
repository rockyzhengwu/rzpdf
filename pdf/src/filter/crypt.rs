use crate::{
    error::{PdfError, PdfResult},
    objects::pdf_dict::PdfDict,
};

pub fn crypt_decode(input: &[u8], params: Option<&PdfDict>) -> PdfResult<Vec<u8>> {
    let Some(params) = params else {
        return Ok(input.to_vec());
    };

    let name = params
        .get("Name")
        .and_then(|value| value.as_name())
        .map(|name| name.name())
        .unwrap_or("Identity");
    let _ty = params
        .get("Type")
        .and_then(|value| value.as_name())
        .map(|name| name.name())
        .unwrap_or("CryptFilterDecodeParms");

    if name == "Identity" {
        return Ok(input.to_vec());
    }

    Err(PdfError::FilterError(format!(
        "Crypt filter '{name}' requires a security handler implementation"
    )))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::objects::{PdfObject, pdf_dict::PdfDict, pdf_name::PdfName};

    use super::crypt_decode;

    #[test]
    fn test_crypt_identity_passthrough() {
        let mut params = PdfDict::new(HashMap::new());
        params.insert(
            "Name".to_string(),
            PdfObject::PdfName(PdfName::new("Identity".to_string())),
        );
        assert_eq!(crypt_decode(b"abc", Some(&params)).unwrap(), b"abc");
    }
}
