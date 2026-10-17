//! Page labels indexed by zero-based physical page position.
use super::{
    PdfDocument,
    trees::{TreeKey, TreeKind},
};
use crate::{
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, pdf_dict::PdfDict, pdf_name::PdfName, pdf_number::PdfNumber,
        pdf_reference::PdfReference, pdf_string::PdfString,
    },
};
use std::io::{Read, Seek};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageLabelStyle {
    Decimal,
    RomanUpper,
    RomanLower,
    LettersUpper,
    LettersLower,
}

impl PageLabelStyle {
    fn name(self) -> &'static str {
        match self {
            Self::Decimal => "D",
            Self::RomanUpper => "R",
            Self::RomanLower => "r",
            Self::LettersUpper => "A",
            Self::LettersLower => "a",
        }
    }
    fn parse(name: &str) -> PdfResult<Self> {
        match name {
            "D" => Ok(Self::Decimal),
            "R" => Ok(Self::RomanUpper),
            "r" => Ok(Self::RomanLower),
            "A" => Ok(Self::LettersUpper),
            "a" => Ok(Self::LettersLower),
            _ => Err(PdfError::UnsupportedFeature(format!(
                "page label style {name}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageLabelRange {
    pub start_index: u32,
    /// None means prefix only, without a numeric suffix.
    pub style: Option<PageLabelStyle>,
    pub prefix: String,
    pub start_number: u32,
}

impl<R: Read + Seek> PdfDocument<R> {
    /// Plan page-bound labels before changing the page tree. None introduces
    /// a new page with its ordinary physical decimal label; Some copies the
    /// original page label. Unknown label dictionary extensions are not dropped.
    pub(crate) fn labels_after_page_mapping(
        &self,
        mapping: impl IntoIterator<Item = Option<u32>>,
    ) -> PdfResult<Option<Vec<PageLabelRange>>> {
        let root = self.ctx().get_root_owned()?;
        let Some(tree) = root.get("PageLabels") else {
            return Ok(None);
        };
        let tree_dict =
            self.ctx().resolve(tree)?.into_dict().ok_or_else(|| {
                PdfError::DocumentError("page label tree must be dictionary".into())
            })?;
        if (&tree_dict)
            .into_iter()
            .any(|(key, _)| !matches!(key.as_str(), "Nums" | "Kids" | "Limits"))
        {
            return Err(PdfError::UnsupportedFeature(
                "page label remapping with unknown tree-root extensions".into(),
            ));
        }
        for (_, value) in self.read_tree(tree, TreeKind::Number)? {
            let dict =
                self.ctx().resolve(&value)?.into_dict().ok_or_else(|| {
                    PdfError::DocumentError("page label must be dictionary".into())
                })?;
            if (&dict)
                .into_iter()
                .any(|(k, _)| !matches!(k.as_str(), "Type" | "S" | "P" | "St"))
            {
                return Err(PdfError::UnsupportedFeature(
                    "page label remapping with unknown dictionary keys".into(),
                ));
            }
        }
        let ranges = self.page_label_ranges()?;
        let count = self.try_page_count()?;
        let mut result: Vec<PageLabelRange> = Vec::new();
        for (index, original) in mapping.into_iter().enumerate() {
            let start_index = u32::try_from(index)
                .map_err(|_| PdfError::ResourceLimit("page label index".into()))?;
            let next = if let Some(original) = original {
                if original as usize >= count {
                    return Err(PdfError::PageNotExist);
                }
                let range = ranges
                    .iter()
                    .rev()
                    .find(|r| r.start_index <= original)
                    .ok_or_else(|| PdfError::DocumentError("page label range missing".into()))?;
                let start_number = if range.style.is_none() {
                    1
                } else {
                    range
                        .start_number
                        .checked_add(original - range.start_index)
                        .ok_or_else(|| {
                            PdfError::ResourceLimit("page label number overflow".into())
                        })?
                };
                PageLabelRange {
                    start_index,
                    style: range.style,
                    prefix: range.prefix.clone(),
                    start_number,
                }
            } else {
                PageLabelRange {
                    start_index,
                    style: Some(PageLabelStyle::Decimal),
                    prefix: String::new(),
                    start_number: start_index.checked_add(1).ok_or_else(|| {
                        PdfError::ResourceLimit("page label number overflow".into())
                    })?,
                }
            };
            if result.last().is_some_and(|previous| {
                previous.style == next.style
                    && previous.prefix == next.prefix
                    && (next.style.is_none()
                        || previous
                            .start_number
                            .checked_add(start_index - previous.start_index)
                            == Some(next.start_number))
            }) {
                continue;
            }
            if result.len() >= self.ctx().limits().max_objects / 2 {
                return Err(PdfError::ResourceLimit("page label ranges".into()));
            }
            result.push(next);
        }
        Ok(Some(result))
    }

    pub fn page_label_ranges(&self) -> PdfResult<Vec<PageLabelRange>> {
        let root = self.ctx().get_root_owned()?;
        let Some(tree) = root.get("PageLabels") else {
            return Ok(Vec::new());
        };
        let count = self.try_page_count()?;
        let mut result = Vec::new();
        for (key, object) in self.read_tree(tree, TreeKind::Number)? {
            let TreeKey::Number(start_index) = key else {
                unreachable!()
            };
            if start_index as usize >= count || (result.is_empty() && start_index != 0) {
                return Err(PdfError::DocumentError(
                    "page label range outside document or missing zero".into(),
                ));
            }
            let dict =
                self.ctx().resolve(&object)?.into_dict().ok_or_else(|| {
                    PdfError::DocumentError("page label must be a dictionary".into())
                })?;
            let style = dict
                .get("S")
                .map(|s| {
                    let s = self.ctx().resolve(s)?;
                    PageLabelStyle::parse(
                        s.as_name()
                            .ok_or_else(|| {
                                PdfError::DocumentError("page label S must be a name".into())
                            })?
                            .name(),
                    )
                })
                .transpose()?;
            let prefix = dict
                .get("P")
                .map(|p| {
                    self.ctx()
                        .resolve(p)?
                        .as_string()
                        .ok_or_else(|| {
                            PdfError::DocumentError("page label prefix must be a string".into())
                        })?
                        .text_content_with_limits(&self.ctx().limits())
                })
                .transpose()?
                .unwrap_or_default();
            let start_number = dict
                .get("St")
                .map(|s| {
                    self.ctx()
                        .resolve(s)?
                        .as_number()
                        .ok_or_else(|| {
                            PdfError::DocumentError("page label St must be an integer".into())
                        })?
                        .as_u32_checked()
                })
                .transpose()?
                .unwrap_or(1);
            if start_number == 0 {
                return Err(PdfError::DocumentError(
                    "page label start must be positive".into(),
                ));
            }
            result.push(PageLabelRange {
                start_index,
                style,
                prefix,
                start_number,
            });
        }
        if result.is_empty() {
            return Err(PdfError::DocumentError(
                "PageLabels must have an entry for page zero".into(),
            ));
        }
        Ok(result)
    }

    /// Replace all ranges. An empty slice removes PageLabels. Ranges are
    /// position-based; start at zero, then increase strictly within page count.
    pub fn set_page_label_ranges(&self, ranges: &[PageLabelRange]) -> PdfResult<()> {
        self.ctx().require_assembly_permission()?;
        if self.effective_version()? < super::save_options::PdfVersion::V1_3 {
            return Err(PdfError::UnsupportedFeature(
                "PageLabels requires PDF 1.3".into(),
            ));
        }
        self.edit_transaction(|doc| {
            let count = doc.try_page_count()?;
            let mut entries = Vec::new();
            if ranges.len() > doc.ctx().limits().max_objects / 2 {
                return Err(PdfError::ResourceLimit("page label ranges".into()));
            }
            for (i, range) in ranges.iter().enumerate() {
                if range.start_number == 0
                    || range.start_index as usize >= count
                    || (i == 0 && range.start_index != 0)
                    || (i > 0 && ranges[i - 1].start_index >= range.start_index)
                {
                    return Err(PdfError::DocumentError("invalid page label range".into()));
                }
                let mut dict = PdfDict::default();
                if let Some(style) = range.style {
                    dict.try_insert(
                        "S".into(),
                        PdfObject::PdfName(PdfName::new(style.name().into())),
                    )?;
                }
                dict.try_insert(
                    "P".into(),
                    PdfObject::PdfString(PdfString::from_text_with_limits(
                        &range.prefix,
                        &doc.ctx().limits(),
                    )?),
                )?;
                dict.try_insert(
                    "St".into(),
                    PdfObject::PdfNumber(PdfNumber::new_from_lexeme(
                        range.start_number.to_string().as_bytes(),
                    )?),
                )?;
                entries.push((TreeKey::Number(range.start_index), PdfObject::PdfDict(dict)));
            }
            let mut root = doc.ctx().get_root_owned()?;
            if entries.is_empty() {
                root.remove("PageLabels");
            } else {
                let tree = doc.build_tree(TreeKind::Number, &entries, PdfDict::default())?;
                root.try_insert("PageLabels".into(), PdfObject::PdfDict(tree))?;
            }
            let id = doc
                .ctx()
                .trailer()
                .get("Root")
                .and_then(PdfObject::as_reference)
                .map(PdfReference::id)
                .ok_or_else(|| PdfError::DocumentError("Root must be indirect".into()))?;
            doc.update_object(id, PdfObject::PdfDict(root))
        })
    }

    pub fn page_label(&self, page_index: u32) -> PdfResult<String> {
        self.page_location(page_index)?;
        let ranges = self.page_label_ranges()?;
        let Some(range) = ranges.iter().rev().find(|r| r.start_index <= page_index) else {
            return Ok((u64::from(page_index) + 1).to_string());
        };
        let number = u64::from(range.start_number) + u64::from(page_index - range.start_index);
        let limit = self.ctx().limits().max_token_bytes;
        let mut result = range.prefix.clone();
        match range.style {
            None => {}
            Some(PageLabelStyle::Decimal) => result.push_str(&number.to_string()),
            Some(style @ (PageLabelStyle::RomanUpper | PageLabelStyle::RomanLower)) => {
                let mut remaining = number;
                for (n, text) in [
                    (1000, "M"),
                    (900, "CM"),
                    (500, "D"),
                    (400, "CD"),
                    (100, "C"),
                    (90, "XC"),
                    (50, "L"),
                    (40, "XL"),
                    (10, "X"),
                    (9, "IX"),
                    (5, "V"),
                    (4, "IV"),
                    (1, "I"),
                ] {
                    let repeats = remaining / n;
                    if repeats > limit.saturating_sub(result.len()) as u64 / text.len() as u64 {
                        return Err(PdfError::ResourceLimit("page label text".into()));
                    }
                    for _ in 0..repeats {
                        if style == PageLabelStyle::RomanLower {
                            result.push_str(&text.to_ascii_lowercase());
                        } else {
                            result.push_str(text);
                        }
                    }
                    remaining %= n;
                }
            }
            Some(style) => {
                let repeats = (number - 1) / 26 + 1;
                if repeats > limit.saturating_sub(result.len()) as u64 {
                    return Err(PdfError::ResourceLimit("page label text".into()));
                }
                let base = if style == PageLabelStyle::LettersUpper {
                    b'A'
                } else {
                    b'a'
                };
                let letter = (base + ((number - 1) % 26) as u8) as char;
                for _ in 0..repeats {
                    result.push(letter);
                }
            }
        }
        if result.len() > limit {
            return Err(PdfError::ResourceLimit("page label text".into()));
        }
        Ok(result)
    }
}
