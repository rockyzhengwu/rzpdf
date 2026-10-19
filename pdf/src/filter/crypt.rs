use crate::{
    error::{PdfError, PdfResult},
    objects::pdf_dict::PdfDict,
};

pub fn crypt_decode(input: &[u8], params: Option<&PdfDict>) -> PdfResult<Vec<u8>> {
    validate_identity(params)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(input.len())
        .map_err(|_| PdfError::ResourceLimit("Crypt identity output allocation".into()))?;
    output.extend_from_slice(input);
    Ok(output)
}

pub fn crypt_decode_with_budget(
    input: &[u8],
    params: Option<&PdfDict>,
    limit: usize,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<crate::budget::BudgetedBytes> {
    super::check_output_size(0, input.len(), limit, "Crypt")?;
    validate_identity(params)?;
    crate::budget::BudgetedBytes::copy_from_slice(budget, input)
}

fn validate_identity(params: Option<&PdfDict>) -> PdfResult<()> {
    let Some(params) = params else {
        return Ok(());
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
        return Ok(());
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
    fn budgeted_identity_respects_neighbors_limits_and_security_handler_requirement() {
        use crate::{
            budget::OperationBudget,
            error::PdfError,
            filter::{FilterContext, apply_filter_with_budget},
        };
        let budget = OperationBudget::new(3);
        let neighbor = budget.reserve(1).unwrap();
        assert!(matches!(
            super::crypt_decode_with_budget(b"abc", None, 3, &budget),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(budget.live_bytes(), 1);
        drop(neighbor);
        let result =
            apply_filter_with_budget("Crypt", b"abc", None, FilterContext::default(), &budget)
                .unwrap();
        assert_eq!(result.as_slice(), b"abc");
        assert_eq!(budget.live_bytes(), result.capacity());
        drop(result);
        assert_eq!(budget.live_bytes(), 0);
        assert!(super::crypt_decode_with_budget(b"abc", None, 2, &budget).is_err());
        let mut params = PdfDict::default();
        params.insert(
            "Name".into(),
            PdfObject::PdfName(PdfName::new("Secure".into())),
        );
        assert!(matches!(
            super::crypt_decode_with_budget(b"abc", Some(&params), 3, &budget),
            Err(PdfError::FilterError(_))
        ));
        assert_eq!(budget.live_bytes(), 0);
        params.insert(
            "Name".into(),
            PdfObject::PdfName(PdfName::new("Identity".into())),
        );
        assert_eq!(
            super::crypt_decode_with_budget(b"abc", Some(&params), 3, &budget)
                .unwrap()
                .as_slice(),
            b"abc"
        );
        assert_eq!(budget.live_bytes(), 0);
    }

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
