use super::{content_parser::ContentParser, operator::Operator};
use crate::{
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    parser::{options::ParseOptions, syntax::SyntaxParser},
};
use std::{
    io::{Cursor, Read, Seek},
    ops::Range,
};

impl<R: Read + Seek> super::PdfPage<'_, R> {
    /// Encode Unicode using an explicitly selected existing font resource.
    /// Does not infer the active Tf, register fonts, shape text or prove glyph coverage.
    /// Composite fonts require existing ToUnicode and non-.notdef Encoding codes.
    pub fn encode_text_for_font(&self, resource_name: &str, text: &str) -> PdfResult<Vec<u8>> {
        let limits = self.ctx.limits();
        if resource_name.len() > limits.max_token_bytes {
            return Err(PdfError::ResourceLimit("font resource name bytes".into()));
        }
        let object = self
            .get_resource_font(resource_name)?
            .ok_or_else(|| PdfError::FontError("font resource not found".into()))?;
        let dict = object
            .as_dict()
            .ok_or_else(|| PdfError::FontError("font resource must be a dictionary".into()))?;
        match crate::font::pdf_font::Font::try_new(dict, self.ctx)? {
            crate::font::pdf_font::Font::Simple(font) => font.encode_text_with_limits(text, limits),
            crate::font::pdf_font::Font::Type0(font) => font.encode_text_with_limits(text, limits),
        }
    }

    /// Parse the latest page Contents without interpretation, preserving decoded
    /// stream boundaries and unknown operations. Source offsets are not file offsets.
    pub fn content_program(&self) -> PdfResult<ContentProgram> {
        self.content_program_inner(None).map(|(program, _)| program)
    }

    /// Track source, tables, tokens and recursively parsed operand payloads
    /// throughout collection, parsing and return. Resolved document objects
    /// and native allocations remain outside this incomplete operation budget.
    pub fn content_program_with_budget(
        &self,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<BudgetedContentProgram> {
        let (program, leases) = self.content_program_inner(Some(budget))?;
        Ok(BudgetedContentProgram {
            program,
            _leases: leases,
        })
    }

    fn content_program_inner(
        &self,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<(ContentProgram, ProgramLeases)> {
        let options = self.ctx.options();
        let (page_dict, page_lease) = if let Some(budget) = budget {
            let (dict, lease) = self.pagedict_with_budget(budget)?;
            (dict, Some(lease))
        } else {
            (self.pagedict()?, None)
        };
        let contents = page_dict
            .get("Contents")
            .map(|contents| {
                let (object, lease) = if let Some(budget) = budget {
                    let (object, lease) = self.ctx.resolve_owned_with_budget(contents, budget)?;
                    (object, Some(lease))
                } else {
                    (self.ctx.resolve_owned(contents)?, None)
                };
                Ok::<_, PdfError>((object, contents.as_object_id(), lease))
            })
            .transpose()?;
        drop(page_dict);
        drop(page_lease);
        let count = match contents.as_ref().map(|(object, _, _)| object) {
            Some(crate::objects::PdfObject::PdfArray(array)) => array.len(),
            Some(crate::objects::PdfObject::PdfStream(_)) => 1,
            None | Some(crate::objects::PdfObject::PdfNull) => 0,
            _ => return Err(PdfError::PageContentIsNotStream),
        };
        if count > options.limits.max_objects {
            return Err(PdfError::ResourceLimit("page program stream count".into()));
        }
        let mut source_lease = budget.map(|budget| budget.reserve(0)).transpose()?;
        let mut ranges_lease = reserve_metadata::<Range<usize>>(budget, count)?;
        let mut ids_lease =
            reserve_metadata::<Option<crate::objects::object_id::ObjectId>>(budget, count)?;
        let mut source = Vec::new();
        let mut stream_ranges = Vec::new();
        let mut stream_ids = Vec::new();
        stream_ranges
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("page program ranges allocation".into()))?;
        stream_ids
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("page program IDs allocation".into()))?;
        reconcile_metadata::<Range<usize>>(&mut ranges_lease, stream_ranges.capacity())?;
        reconcile_metadata::<Option<crate::objects::object_id::ObjectId>>(
            &mut ids_lease,
            stream_ids.capacity(),
        )?;
        let mut total = 0usize;
        let mut push = |stream: &crate::objects::pdf_stream::PdfStream, id| -> PdfResult<()> {
            if stream_ranges.len() >= options.limits.max_objects {
                return Err(PdfError::ResourceLimit("page program stream count".into()));
            }
            let (bytes, bytes_lease) = if let Some(budget) = budget {
                let (data, lease) = stream
                    .decode_data_with_budget(self.ctx, budget)?
                    .into_reserved_parts();
                (data, Some(lease))
            } else {
                (stream.decode_data(self.ctx)?, None)
            };
            total = total
                .checked_add(bytes.len())
                .filter(|size| *size <= options.limits.max_decoded_bytes)
                .ok_or_else(|| PdfError::ResourceLimit("page program decoded bytes".into()))?;
            stream_ranges
                .try_reserve(1)
                .map_err(|_| PdfError::ResourceLimit("page program streams allocation".into()))?;
            let start = source.len();
            if stream_ranges.is_empty() {
                // Transfer the first decoded allocation instead of copying it.
                source = bytes;
                source_lease = bytes_lease;
            } else {
                if total > source.capacity() {
                    // Bounded geometric growth avoids copying the accumulated
                    // source for every tiny stream. Capacity remains distinct
                    // from decoded length for future budget accounting.
                    let capacity = source
                        .capacity()
                        .saturating_mul(2)
                        .max(total)
                        .min(options.limits.max_decoded_bytes);
                    if let Some(budget) = budget {
                        let mut lease = budget.reserve(capacity)?;
                        let mut replacement = Vec::new();
                        replacement.try_reserve_exact(capacity).map_err(|_| {
                            PdfError::ResourceLimit(
                                "page program combined source allocation".into(),
                            )
                        })?;
                        lease.resize(replacement.capacity())?;
                        replacement.extend_from_slice(&source);
                        source = replacement;
                        source_lease = Some(lease);
                    } else {
                        source
                            .try_reserve_exact(capacity - source.len())
                            .map_err(|_| {
                                PdfError::ResourceLimit(
                                    "page program combined source allocation".into(),
                                )
                            })?;
                    }
                }
                source.extend_from_slice(&bytes);
            }
            stream_ranges.push(start..source.len());
            stream_ids.try_reserve(1).map_err(|_| {
                PdfError::ResourceLimit("page program source IDs allocation".into())
            })?;
            stream_ids.push(id);
            Ok(())
        };
        if let Some((contents, contents_id, _contents_lease)) = contents {
            match contents {
                crate::objects::PdfObject::PdfArray(array) => {
                    if array.len() > options.limits.max_objects {
                        return Err(PdfError::ResourceLimit("page program stream count".into()));
                    }
                    for object in &array {
                        let id = object.as_object_id();
                        let (object, _lease) = if let Some(budget) = budget {
                            let (object, lease) =
                                self.ctx.resolve_owned_with_budget(object, budget)?;
                            (object, Some(lease))
                        } else {
                            (self.ctx.resolve_owned(object)?, None)
                        };
                        push(
                            object.as_stream().ok_or(PdfError::PageContentIsNotStream)?,
                            id,
                        )?;
                    }
                }
                crate::objects::PdfObject::PdfStream(stream) => push(&stream, contents_id)?,
                crate::objects::PdfObject::PdfNull => {}
                _ => return Err(PdfError::PageContentIsNotStream),
            }
        }
        let mut operations_lease = budget.map(|budget| budget.reserve(0)).transpose()?;
        let mut program = ContentProgram::parse_with_stream_ranges_budget(
            source,
            stream_ranges,
            options,
            budget,
            &mut operations_lease,
        )?;
        program.stream_object_ids = Some(stream_ids);
        program.source_page_id = self.page_node.as_object_id();
        program.source_stamp = Some(crate::document::identity::DocumentStamp {
            document: self.ctx.identity(),
            revision: self.ctx.revision(),
        });
        if let Some(owner) = source_lease.as_mut() {
            for lease in [&mut ranges_lease, &mut ids_lease, &mut operations_lease] {
                if let Some(lease) = lease.as_mut() {
                    owner.absorb(lease)?;
                }
            }
        }
        Ok((
            program,
            ProgramLeases {
                _allocation: source_lease,
            },
        ))
    }
}

/// Non-Clone source-budget owner. Legacy cloning/editing of the borrowed
/// program does not inherit this lease and remains outside this interface.
#[derive(Debug)]
pub struct BudgetedContentProgram {
    program: ContentProgram,
    _leases: ProgramLeases,
}

#[derive(Debug)]
struct ProgramLeases {
    _allocation: Option<crate::budget::MemoryLease>,
}

fn reserve_metadata<T>(
    budget: Option<&crate::budget::OperationBudget>,
    count: usize,
) -> PdfResult<Option<crate::budget::MemoryLease>> {
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| PdfError::ResourceLimit("page program metadata size".into()))?;
    budget.map(|budget| budget.reserve(bytes)).transpose()
}

fn reconcile_metadata<T>(
    lease: &mut Option<crate::budget::MemoryLease>,
    capacity: usize,
) -> PdfResult<()> {
    if let Some(lease) = lease {
        lease.resize(
            capacity
                .checked_mul(std::mem::size_of::<T>())
                .ok_or_else(|| PdfError::ResourceLimit("page program metadata capacity".into()))?,
        )?;
    }
    Ok(())
}

impl BudgetedContentProgram {
    pub fn program(&self) -> &ContentProgram {
        &self.program
    }
}

impl AsRef<ContentProgram> for BudgetedContentProgram {
    fn as_ref(&self) -> &ContentProgram {
        self.program()
    }
}

#[cfg(test)]
mod decoded_budget_tests {
    use super::*;
    use crate::budget::{BudgetedBytes, OperationBudget};

    #[test]
    fn decoded_source_parser_transfers_quota_and_cleans_failure_and_unwind() {
        let data = b"% retained\n(AB) Tj [1 (CD)] TJ q Q";
        let legacy = ContentProgram::from_decoded_bytes(data.to_vec(), Default::default()).unwrap();
        let probe = OperationBudget::new(65536);
        let parsed = ContentProgram::from_decoded_bytes_with_budget(
            BudgetedBytes::copy_from_slice(&probe, data).unwrap(),
            Default::default(),
            &probe,
        )
        .unwrap();
        assert_eq!(parsed.program().source_bytes(), legacy.source_bytes());
        assert_eq!(parsed.program().stream_ranges(), legacy.stream_ranges());
        for (left, right) in parsed
            .program()
            .operations()
            .iter()
            .zip(legacy.operations())
        {
            assert_eq!(left.operator.name(), right.operator.name());
            assert_eq!(left.source_range, right.source_range);
        }
        assert_eq!(
            parsed.program().operations().len(),
            legacy.operations().len()
        );
        assert_eq!(
            probe.live_bytes(),
            parsed._leases._allocation.as_ref().unwrap().bytes()
        );
        assert!(probe.live_bytes() > data.len());
        let peak = probe.peak_bytes();
        drop(parsed);
        assert_eq!(probe.live_bytes(), 0);
        let limited = OperationBudget::new(peak - 1);
        assert!(ContentProgram::from_decoded_bytes_with_budget(
            BudgetedBytes::copy_from_slice(&limited, data).unwrap(),
            Default::default(),
            &limited,
        )
        .is_err());
        assert_eq!(limited.live_bytes(), 0);
        drop(
            ContentProgram::from_decoded_bytes_with_budget(
                BudgetedBytes::copy_from_slice(&limited, b"q").unwrap(),
                Default::default(),
                &limited,
            )
            .unwrap(),
        );
        assert_eq!(limited.live_bytes(), 0);
        assert!(ContentProgram::from_decoded_bytes_with_budget(
            BudgetedBytes::copy_from_slice(&probe, b"[(unterminated").unwrap(),
            Default::default(),
            &probe,
        )
        .is_err());
        assert_eq!(probe.live_bytes(), 0);
        assert!(ContentProgram::from_decoded_bytes_with_budget(
            BudgetedBytes::copy_from_slice(&probe, data).unwrap(),
            Default::default(),
            &limited,
        )
        .is_err());
        assert_eq!(probe.live_bytes(), 0);
        assert_eq!(limited.live_bytes(), 0);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _parsed = ContentProgram::from_decoded_bytes_with_budget(
                BudgetedBytes::copy_from_slice(&probe, data).unwrap(),
                Default::default(),
                &probe,
            )
            .unwrap();
            panic!("decoded source owner unwind");
        }));
        assert!(caught.is_err());
        assert_eq!(probe.live_bytes(), 0);
    }
}

impl AsRef<ContentProgram> for ContentProgram {
    fn as_ref(&self) -> &ContentProgram {
        self
    }
}

/// Parsed operation plus its source chunk, including leading whitespace/comments.
/// Operator.byte_offset identifies the operator token itself within the source.
#[derive(Debug, Clone)]
pub struct SourceOperation {
    pub operator: Operator,
    pub source_range: Range<usize>,
    /// Operand/operator bytes excluding leading whitespace and comments.
    /// Whitespace/comments between operands remain part of the operation.
    pub lexical_range: Range<usize>,
}

/// Nonempty intersection of one operation source chunk with one decoded stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationSourceFragment {
    pub stream_index: usize,
    pub stream_range: Range<usize>,
    pub combined_range: Range<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentResourceKind {
    Font,
    XObject,
    ExtGState,
    ColorSpace,
    Shading,
    Properties,
}

/// Borrowed resource name as decoded by PDF name syntax; no payload allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentResourceUse<'a> {
    pub kind: ContentResourceKind,
    pub name: &'a str,
    pub operand_index: usize,
}

impl SourceOperation {
    /// Identify statically named resource operands. Pattern scn/SCN requires
    /// graphics-state tracking and is deliberately not inferred here. Unknown
    /// operators stay preserved but have no guessed resource interpretation.
    pub fn named_resource_use(&self) -> PdfResult<Option<ContentResourceUse<'_>>> {
        let (kind, index) = match self.operator.name() {
            "Tf" => (ContentResourceKind::Font, 0),
            "Do" => (ContentResourceKind::XObject, 0),
            "gs" => (ContentResourceKind::ExtGState, 0),
            "sh" => (ContentResourceKind::Shading, 0),
            "CS" | "cs" => (ContentResourceKind::ColorSpace, 0),
            "BDC" | "DP" => {
                let operand = self.operator.operand(1)?;
                if operand.as_dict().is_some() {
                    return Ok(None);
                }
                (ContentResourceKind::Properties, 1)
            }
            _ => return Ok(None),
        };
        let name = self.operator.get_as_name(index)?.name();
        if kind == ContentResourceKind::ColorSpace
            && matches!(name, "DeviceGray" | "DeviceRGB" | "DeviceCMYK" | "Pattern")
        {
            return Ok(None);
        }
        Ok(Some(ContentResourceUse {
            kind,
            name,
            operand_index: index,
        }))
    }
}

/// Lossless source representation of one decoded content byte sequence.
/// Unknown operators and original lexical spelling are retained. This is not
/// yet a resource provenance graph or a validated editing program.
#[derive(Debug, Clone)]
pub struct ContentProgram {
    source: Vec<u8>,
    operations: Vec<SourceOperation>,
    stream_ranges: Vec<Range<usize>>,
    stream_object_ids: Option<Vec<Option<crate::objects::object_id::ObjectId>>>,
    source_stamp: Option<crate::document::identity::DocumentStamp>,
    pub(crate) source_page_id: Option<crate::objects::object_id::ObjectId>,
}

impl ContentProgram {
    /// Clear a text-show event with one source scan and one final program parse.
    /// Keeps quote effects, TJ numbers/comments and all neighboring operations.
    pub(crate) fn with_cleared_text_draw(
        &self,
        index: usize,
        options: ParseOptions,
    ) -> PdfResult<Self> {
        let operation = self.operations.get(index).ok_or_else(|| {
            PdfError::ContentParseError("text operation index out of range".into())
        })?;
        if matches!(operation.operator.name(), "Tj" | "'" | "\"") {
            return self.with_replaced_text_codes(index, b"", options);
        }
        if operation.operator.name() != "TJ" || operation.operator.num_operands() != 1 {
            return Err(PdfError::ContentParseError(
                "text removal requires Tj, TJ or quote".into(),
            ));
        }
        let array = operation.operator.get_as_array(0)?;
        if array.len() > options.limits.max_objects {
            return Err(PdfError::ResourceLimit("TJ array items".into()));
        }
        for item in array {
            if item.as_string().is_none() && item.as_number().is_none() {
                return Err(PdfError::ContentParseError(
                    "TJ item must be string or number".into(),
                ));
            }
        }
        let source = &self.source[operation.lexical_range.clone()];
        if source.len() > options.limits.max_decoded_bytes
            || source.len() > options.limits.max_raw_stream_bytes
        {
            return Err(PdfError::ResourceLimit("TJ source bytes".into()));
        }
        // Borrow the source instead of cloning the whole program or array per item.
        let mut parser =
            SyntaxParser::with_options(StreamReader::try_new(Cursor::new(source))?, options);
        if parser.get_next_word()?.raw() != b"[" {
            return Err(PdfError::ContentParseError(
                "TJ array source is invalid".into(),
            ));
        }
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(source.len())
            .map_err(|_| PdfError::ResourceLimit("TJ clearing allocation".into()))?;
        let mut copied = 0;
        for item in array {
            let start = usize::try_from(parser.position())
                .map_err(|_| PdfError::ResourceLimit("TJ item offset".into()))?;
            let parsed = parser.get_object()?;
            let end = usize::try_from(parser.position())
                .map_err(|_| PdfError::ResourceLimit("TJ item offset".into()))?;
            if end > source.len() || &parsed != item {
                return Err(PdfError::ContentParseError(
                    "TJ item source mismatch".into(),
                ));
            }
            if item.as_string().is_some() {
                let token_start = skip_source_prefix(source, start, end);
                if token_start < copied || end - token_start < 2 {
                    return Err(PdfError::ContentParseError(
                        "TJ string source range mismatch".into(),
                    ));
                }
                replacement.extend_from_slice(&source[copied..token_start]);
                replacement.extend_from_slice(b"<>");
                copied = end;
            }
        }
        replacement.extend_from_slice(&source[copied..]);
        self.with_replaced_operation(index, &replacement, options)
    }

    /// Replace one string item in a TJ array, keeping every other source byte
    /// inside that operation (including spacing numbers and comments).
    /// item_index counts both strings and numbers. Advances are not compensated;
    /// codes must be valid for the currently selected font.
    pub fn with_replaced_text_array_item_codes(
        &self,
        index: usize,
        item_index: usize,
        codes: &[u8],
        options: ParseOptions,
    ) -> PdfResult<Self> {
        self.with_replaced_text_array_items_codes(index, &[(item_index, codes)], options)
    }

    /// Replace multiple TJ strings in one source pass. Indices include numbers
    /// and must be strictly increasing. Untouched tokens, numbers and comments
    /// remain byte-identical; character advances are not compensated.
    pub fn with_replaced_text_array_items_codes(
        &self,
        index: usize,
        replacements: &[(usize, &[u8])],
        options: ParseOptions,
    ) -> PdfResult<Self> {
        self.with_replaced_text_array_items_codes_inner(
            index,
            replacements,
            options,
            None,
            &mut None,
        )
    }

    /// Budget TJ item verification, generated tokens and the rebuilt result.
    /// The original program and borrowed replacement list/codes are excluded.
    pub fn with_replaced_text_array_items_codes_with_budget(
        &self,
        index: usize,
        replacements: &[(usize, &[u8])],
        options: ParseOptions,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<BudgetedContentProgram> {
        let mut allocation = Some(budget.reserve(0)?);
        let program = self.with_replaced_text_array_items_codes_inner(
            index,
            replacements,
            options,
            Some(budget),
            &mut allocation,
        )?;
        Ok(BudgetedContentProgram {
            program,
            _leases: ProgramLeases {
                _allocation: allocation,
            },
        })
    }

    fn with_replaced_text_array_items_codes_inner(
        &self,
        index: usize,
        replacements: &[(usize, &[u8])],
        options: ParseOptions,
        budget: Option<&crate::budget::OperationBudget>,
        allocation: &mut Option<crate::budget::MemoryLease>,
    ) -> PdfResult<Self> {
        let operation = self.operations.get(index).ok_or_else(|| {
            PdfError::ContentParseError("content operation index out of range".into())
        })?;
        if operation.operator.name() != "TJ" || operation.operator.num_operands() != 1 {
            return Err(PdfError::UnsupportedFeature(
                "array text replacement requires TJ".into(),
            ));
        }
        let array = operation.operator.get_as_array(0)?;
        if array.len() > options.limits.max_objects
            || replacements.len() > options.limits.max_objects
        {
            return Err(PdfError::ResourceLimit("TJ array items".into()));
        }
        for item in array {
            if item.as_string().is_none() && item.as_number().is_none() {
                return Err(PdfError::ContentParseError(
                    "TJ item must be string or number".into(),
                ));
            }
        }
        let mut previous = None;
        for &(item, _) in replacements {
            if previous.is_some_and(|previous| item <= previous)
                || array.get(item).and_then(|item| item.as_string()).is_none()
            {
                return Err(PdfError::ContentParseError(
                    "TJ replacement indices must be ordered string items".into(),
                ));
            }
            previous = Some(item);
        }
        let source = &self.source[operation.lexical_range.clone()];
        let limit = options
            .limits
            .max_decoded_bytes
            .min(options.limits.max_raw_stream_bytes);
        if source.len() > limit {
            return Err(PdfError::ResourceLimit("TJ source bytes".into()));
        }
        let mut parser =
            SyntaxParser::with_options(StreamReader::try_new(Cursor::new(source))?, options);
        let opening = if let Some(budget) = budget {
            parser.get_next_word_with_budget(budget)?
        } else {
            parser.get_next_word()?
        };
        if opening.raw() != b"[" {
            return Err(PdfError::ContentParseError(
                "TJ array source is invalid".into(),
            ));
        }
        drop(opening);
        let mut replacement_lease = budget.map(|budget| budget.reserve(0)).transpose()?;
        let mut replacement = Vec::new();
        let mut copied = 0;
        let mut next = 0;
        for (item_index, item) in array.into_iter().enumerate() {
            let start = usize::try_from(parser.position())
                .map_err(|_| PdfError::ResourceLimit("TJ item offset".into()))?;
            let (parsed, _lease) = if let Some(budget) = budget {
                let (parsed, lease) = parser.get_content_object_with_budget(budget)?;
                (parsed, Some(lease))
            } else {
                (parser.get_object()?, None)
            };
            let end = usize::try_from(parser.position())
                .map_err(|_| PdfError::ResourceLimit("TJ item offset".into()))?;
            if end > source.len() || &parsed != item {
                return Err(PdfError::ContentParseError(
                    "TJ item source mismatch".into(),
                ));
            }
            if let Some(&(target, codes)) = replacements.get(next) {
                if item_index == target {
                    let start = skip_source_prefix(source, start, end);
                    if start < copied || start > end {
                        return Err(PdfError::ContentParseError(
                            "TJ string source range mismatch".into(),
                        ));
                    }
                    let mut builder = super::content_builder::ContentBuilder::new();
                    let encoded = budget
                        .map(|budget| {
                            super::content_builder::ContentBuilder::encoded_text_with_budget(
                                codes,
                                options.limits,
                                budget,
                            )
                        })
                        .transpose()?;
                    if encoded.is_none() {
                        builder.show_encoded_text_with_limits(codes, options.limits)?;
                    }
                    let bytes = encoded
                        .as_ref()
                        .map(|bytes| bytes.as_slice())
                        .unwrap_or(builder.as_bytes());
                    let token = &bytes[..bytes.len() - 4];
                    let size = replacement
                        .len()
                        .checked_add(start - copied)
                        .and_then(|n| n.checked_add(token.len()))
                        .filter(|&n| n <= limit)
                        .ok_or_else(|| PdfError::ResourceLimit("TJ replacement bytes".into()))?;
                    reserve_replacement_bytes(
                        &mut replacement,
                        size,
                        budget,
                        &mut replacement_lease,
                    )?;
                    replacement.extend_from_slice(&source[copied..start]);
                    replacement.extend_from_slice(token);
                    copied = end;
                    next += 1;
                }
            }
        }
        let size = replacement
            .len()
            .checked_add(source.len() - copied)
            .filter(|&n| n <= limit)
            .ok_or_else(|| PdfError::ResourceLimit("TJ replacement bytes".into()))?;
        reserve_replacement_bytes(&mut replacement, size, budget, &mut replacement_lease)?;
        replacement.extend_from_slice(&source[copied..]);
        self.with_replaced_operation_sequence_inner(
            index,
            &replacement,
            options,
            1,
            budget,
            allocation,
        )
    }

    /// Replace the string of Tj/'/" with caller-encoded font character codes.
    /// Does not encode Unicode, select/register a font, preserve old advances,
    /// reshape text or update ToUnicode. TJ arrays require a separate policy.
    pub fn with_replaced_text_codes(
        &self,
        index: usize,
        codes: &[u8],
        options: ParseOptions,
    ) -> PdfResult<Self> {
        self.with_replaced_text_codes_inner(index, codes, options, None, &mut None)
    }

    /// Budget string-token generation, operand verification and the rebuilt
    /// program for Tj, ' and ". Borrowed original/codes retain their owner.
    pub fn with_replaced_text_codes_with_budget(
        &self,
        index: usize,
        codes: &[u8],
        options: ParseOptions,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<BudgetedContentProgram> {
        let mut allocation = Some(budget.reserve(0)?);
        let program = self.with_replaced_text_codes_inner(
            index,
            codes,
            options,
            Some(budget),
            &mut allocation,
        )?;
        Ok(BudgetedContentProgram {
            program,
            _leases: ProgramLeases {
                _allocation: allocation,
            },
        })
    }

    fn with_replaced_text_codes_inner(
        &self,
        index: usize,
        codes: &[u8],
        options: ParseOptions,
        budget: Option<&crate::budget::OperationBudget>,
        allocation: &mut Option<crate::budget::MemoryLease>,
    ) -> PdfResult<Self> {
        let operation = self.operations.get(index).ok_or_else(|| {
            PdfError::ContentParseError("content operation index out of range".into())
        })?;
        let string_index = match operation.operator.name() {
            "Tj" | "'" if operation.operator.num_operands() == 1 => 0,
            "\"" if operation.operator.num_operands() == 3 => {
                operation.operator.get_as_f32(0)?;
                operation.operator.get_as_f32(1)?;
                2
            }
            _ => {
                return Err(PdfError::UnsupportedFeature(
                    "encoded text replacement requires Tj or a quote text operator".into(),
                ));
            }
        };
        operation.operator.get_as_string(string_index)?;
        let mut builder = super::content_builder::ContentBuilder::new();
        let encoded = budget
            .map(|budget| {
                super::content_builder::ContentBuilder::encoded_text_with_budget(
                    codes,
                    options.limits,
                    budget,
                )
            })
            .transpose()?;
        if encoded.is_none() {
            builder.show_encoded_text_with_limits(codes, options.limits)?;
        }
        let source = &self.source[operation.lexical_range.clone()];
        let mut parser =
            SyntaxParser::with_options(StreamReader::try_new(Cursor::new(source))?, options);
        let mut range = 0..0;
        for operand in 0..=string_index {
            let start = usize::try_from(parser.position())
                .map_err(|_| PdfError::ResourceLimit("text operand offset".into()))?;
            let (parsed, _lease) = if let Some(budget) = budget {
                let (parsed, lease) = parser.get_content_object_with_budget(budget)?;
                (parsed, Some(lease))
            } else {
                (parser.get_object()?, None)
            };
            let end = usize::try_from(parser.position())
                .map_err(|_| PdfError::ResourceLimit("text operand offset".into()))?;
            if end > source.len() || operation.operator.operand(operand)? != &parsed {
                return Err(PdfError::ContentParseError(
                    "text operand source mismatch".into(),
                ));
            }
            range = skip_source_prefix(source, start, end)..end;
        }
        let encoded_bytes = encoded
            .as_ref()
            .map(|bytes| bytes.as_slice())
            .unwrap_or(builder.as_bytes());
        let token = &encoded_bytes[..encoded_bytes.len() - 4];
        let size = source
            .len()
            .checked_sub(range.len())
            .and_then(|length| length.checked_add(token.len()))
            .filter(|length| *length <= options.limits.max_decoded_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("text replacement bytes".into()))?;
        let mut replacement_lease = reserve_metadata::<u8>(budget, size)?;
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(size)
            .map_err(|_| PdfError::ResourceLimit("text replacement allocation".into()))?;
        reconcile_metadata::<u8>(&mut replacement_lease, replacement.capacity())?;
        replacement.extend_from_slice(&source[..range.start]);
        replacement.extend_from_slice(token);
        replacement.extend_from_slice(&source[range.end..]);
        self.with_replaced_operation_sequence_inner(
            index,
            &replacement,
            options,
            1,
            budget,
            allocation,
        )
    }

    /// Rebuild a single-string text event as TJ with a caller-computed advance
    /// adjustment. Quote operators expand to their equivalent state operations;
    /// this method does not itself compute metrics or validate font coverage.
    pub fn with_adjusted_text_codes_with_budget(
        &self,
        index: usize,
        codes: &[u8],
        adjustment: f32,
        options: ParseOptions,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<BudgetedContentProgram> {
        let target = &self.operations.get(index).ok_or_else(||
            PdfError::ContentParseError("text operation index out of range".into()))?.operator;
        let count = match (target.name(), target.num_operands()) {
            ("Tj", 1) => { target.get_as_string(0)?; 1 }
            ("'", 1) => { target.get_as_string(0)?; 2 }
            ("\"", 3) => { target.get_as_f32(0)?; target.get_as_f32(1)?; target.get_as_string(2)?; 4 }
            _ => return Err(PdfError::UnsupportedFeature("Adjusted text requires Tj or quote operator".into())),
        };
        let adjusted = super::content_builder::ContentBuilder::adjusted_encoded_text_with_budget(
            codes, adjustment, options.limits, budget,
        )?;
        let prefix: [&[u8]; 5] = match target.name() {
            "\"" => [
                target.operand(0)?.as_number().unwrap().lexeme().as_bytes(), b" Tw\n",
                target.operand(1)?.as_number().unwrap().lexeme().as_bytes(), b" Tc\n", b"T*\n",
            ],
            "'" => [b"", b"", b"", b"", b"T*\n"],
            _ => [b""; 5],
        };
        let length = prefix.iter().try_fold(adjusted.as_slice().len(), |size, part|
            size.checked_add(part.len())).ok_or_else(|| PdfError::ResourceLimit("Adjusted text prefix size".into()))?;
        crate::filter::check_output_size(0, length,
            options.limits.max_raw_stream_bytes.min(options.limits.max_decoded_bytes), "adjusted text bytes")?;
        let mut replacement = crate::budget::BudgetedBytes::copy_from_slice(budget, b"")?;
        replacement.try_reserve_capacity(length)?;
        for part in prefix { replacement.try_extend_from_slice(part)?; }
        replacement.try_extend_from_slice(adjusted.as_slice())?;
        let mut allocation = Some(budget.reserve(0)?);
        let program = self.with_replaced_operation_sequence_inner(
            index, replacement.as_slice(), options, count, Some(budget), &mut allocation,
        )?;
        Ok(BudgetedContentProgram { program, _leases: ProgramLeases { _allocation: allocation } })
    }

    /// Return an independently parsed program with one lexical operation replaced.
    /// Preserves surrounding bytes and stream count; replacement is placed in
    /// the stream containing the operation start. A newline terminates its tail.
    /// Clears document provenance because the returned source is an offline edit.
    /// This validates syntax, not graphics state, font coverage or resource binding.
    pub fn with_replaced_operation(
        &self,
        index: usize,
        replacement: &[u8],
        options: ParseOptions,
    ) -> PdfResult<Self> {
        self.with_replaced_operation_sequence(index, replacement, options, 1)
    }

    /// Budget the replacement validation, rebuilt source and parsed result.
    /// The borrowed original program and replacement bytes are not charged.
    pub fn with_replaced_operation_with_budget(
        &self,
        index: usize,
        replacement: &[u8],
        options: ParseOptions,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<BudgetedContentProgram> {
        let mut allocation = Some(budget.reserve(0)?);
        let program = self.with_replaced_operation_sequence_inner(
            index,
            replacement,
            options,
            1,
            Some(budget),
            &mut allocation,
        )?;
        Ok(BudgetedContentProgram {
            program,
            _leases: ProgramLeases {
                _allocation: allocation,
            },
        })
    }

    /// Remove one lexical operation, preserving surrounding bytes/stream count.
    /// Syntax/neighbor validation does not prove deleting state operations is safe.
    pub fn without_operation(&self, index: usize, options: ParseOptions) -> PdfResult<Self> {
        self.with_replaced_operation_sequence(index, b"", options, 0)
    }

    /// Apply a relative image transform in a q/cm/Do/Q isolation block.
    /// Resource subtype is validated by the document-level editing interface.
    pub fn with_transformed_image_operation(
        &self,
        index: usize,
        matrix: [f32; 6],
        options: ParseOptions,
    ) -> PdfResult<Self> {
        self.with_transformed_xobject_operation(index, matrix, options)
    }

    /// Isolate a named XObject invocation with q/cm/Do/Q. This checks syntax,
    /// not the resource subtype or whether the program is a current snapshot.
    pub fn with_transformed_xobject_operation(
        &self,
        index: usize,
        matrix: [f32; 6],
        options: ParseOptions,
    ) -> PdfResult<Self> {
        let operation = self.operations.get(index).ok_or_else(|| {
            PdfError::ContentParseError("image operation index out of range".into())
        })?;
        if operation.operator.name() != "Do" || operation.operator.num_operands() != 1 {
            return Err(PdfError::ContentParseError(
                "image transform requires one named Do".into(),
            ));
        }
        let resource = operation
            .named_resource_use()?
            .ok_or_else(|| PdfError::ContentParseError("image resource missing".into()))?;
        let mut depth = 0usize;
        for (position, source) in self.operations.iter().enumerate() {
            if position == index && depth >= options.limits.max_depth {
                return Err(PdfError::ResourceLimit(
                    "image transform graphics state depth".into(),
                ));
            }
            match source.operator.name() {
                "q" | "Q" => {
                    if source.operator.num_operands() != 0 {
                        return Err(PdfError::ContentParseError(
                            "q/Q must have no operands".into(),
                        ));
                    }
                    if source.operator.name() == "q" {
                        if depth >= options.limits.max_depth {
                            return Err(PdfError::ResourceLimit(
                                "source graphics state depth".into(),
                            ));
                        }
                        depth += 1;
                    } else {
                        depth = depth.checked_sub(1).ok_or_else(|| {
                            PdfError::ContentParseError("Q without matching q".into())
                        })?;
                    }
                }
                _ => {}
            }
        }
        if depth != 0 {
            return Err(PdfError::ContentParseError(
                "unbalanced source q/Q for image transform".into(),
            ));
        }
        let mut builder = crate::page::content_builder::ContentBuilder::default();
        builder.draw_image_with_limits(resource.name, matrix, options.limits)?;
        self.with_replaced_operation_sequence(index, builder.as_bytes(), options, 4)
    }

    fn with_replaced_operation_sequence(
        &self,
        index: usize,
        replacement: &[u8],
        options: ParseOptions,
        replacement_count: usize,
    ) -> PdfResult<Self> {
        self.with_replaced_operation_sequence_inner(
            index,
            replacement,
            options,
            replacement_count,
            None,
            &mut None,
        )
    }

    fn with_replaced_operation_sequence_inner(
        &self,
        index: usize,
        replacement: &[u8],
        options: ParseOptions,
        replacement_count: usize,
        budget: Option<&crate::budget::OperationBudget>,
        allocation: &mut Option<crate::budget::MemoryLease>,
    ) -> PdfResult<Self> {
        let operation = self.operations.get(index).ok_or_else(|| {
            PdfError::ContentParseError("content operation index out of range".into())
        })?;
        let range = &operation.lexical_range;
        let total = self
            .source
            .len()
            .checked_sub(range.len())
            .and_then(|size| size.checked_add(replacement.len()))
            .and_then(|size| size.checked_add(1))
            .filter(|size| *size <= options.limits.max_decoded_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("edited content source bytes".into()))?;
        let mut fragment_source = reserve_metadata::<u8>(budget, replacement.len())?;
        let mut fragment_ranges = reserve_metadata::<Range<usize>>(budget, 1)?;
        let mut fragment_operations = budget.map(|budget| budget.reserve(0)).transpose()?;
        let mut replacement_bytes = Vec::new();
        replacement_bytes
            .try_reserve_exact(replacement.len())
            .map_err(|_| PdfError::ResourceLimit("replacement operation allocation".into()))?;
        reconcile_metadata::<u8>(&mut fragment_source, replacement_bytes.capacity())?;
        replacement_bytes.extend_from_slice(replacement);
        let mut replacement_ranges = Vec::new();
        replacement_ranges
            .try_reserve_exact(1)
            .map_err(|_| PdfError::ResourceLimit("replacement range allocation".into()))?;
        reconcile_metadata::<Range<usize>>(&mut fragment_ranges, replacement_ranges.capacity())?;
        replacement_ranges.push(0..replacement.len());
        let parsed = Self::parse_with_stream_ranges_budget(
            replacement_bytes,
            replacement_ranges,
            options,
            budget,
            &mut fragment_operations,
        )?;
        if parsed.operations.len() != replacement_count {
            return Err(PdfError::ContentParseError(
                "replacement operation count mismatch".into(),
            ));
        }
        if self.stream_ranges.len() > options.limits.max_objects {
            return Err(PdfError::ResourceLimit(
                "edited content stream count".into(),
            ));
        }
        let mut range_lease = reserve_metadata::<Range<usize>>(budget, self.stream_ranges.len())?;
        let mut source_lease = reserve_metadata::<u8>(budget, total)?;
        let mut operations_lease = budget.map(|budget| budget.reserve(0)).transpose()?;
        let mut ranges = Vec::new();
        ranges
            .try_reserve_exact(self.stream_ranges.len())
            .map_err(|_| PdfError::ResourceLimit("edited content streams allocation".into()))?;
        reconcile_metadata::<Range<usize>>(&mut range_lease, ranges.capacity())?;
        let mut source = Vec::new();
        source
            .try_reserve_exact(total)
            .map_err(|_| PdfError::ResourceLimit("edited content source allocation".into()))?;
        reconcile_metadata::<u8>(&mut source_lease, source.capacity())?;
        for stream in &self.stream_ranges {
            let prefix_end = range.start.clamp(stream.start, stream.end);
            let suffix_start = range.end.clamp(stream.start, stream.end);
            let insert = stream.start <= range.start && range.start < stream.end;
            let start = source.len();
            source.extend_from_slice(&self.source[stream.start..prefix_end]);
            if insert {
                source.extend_from_slice(replacement);
                source.push(b'\n');
            }
            source.extend_from_slice(&self.source[suffix_start..stream.end]);
            ranges.push(start..source.len());
        }
        let edited = Self::parse_with_stream_ranges_budget(
            source,
            ranges,
            options,
            budget,
            &mut operations_lease,
        )?;
        let expected_count = self
            .operations
            .len()
            .checked_sub(1)
            .and_then(|count| count.checked_add(replacement_count))
            .ok_or_else(|| PdfError::ResourceLimit("edited operation count".into()))?;
        if edited.source.len() != total || edited.operations.len() != expected_count {
            return Err(PdfError::ContentParseError(
                "replacement changed adjacent operation boundaries".into(),
            ));
        }
        for (position, actual) in edited.operations.iter().enumerate() {
            let expected = if position >= index && position - index < replacement_count {
                &parsed.operations[position - index]
            } else if position >= index {
                &self.operations[position - replacement_count + 1]
            } else {
                &self.operations[position]
            };
            if actual.operator.name() != expected.operator.name()
                || actual.operator.num_operands() != expected.operator.num_operands()
            {
                return Err(PdfError::ContentParseError(
                    "replacement changed neighboring operations".into(),
                ));
            }
            for operand in 0..expected.operator.num_operands() {
                if actual.operator.operand(operand)? != expected.operator.operand(operand)? {
                    return Err(PdfError::ContentParseError(
                        "replacement changed neighboring operands".into(),
                    ));
                }
            }
        }
        if let Some(owner) = allocation.as_mut() {
            for lease in [&mut source_lease, &mut range_lease, &mut operations_lease] {
                if let Some(lease) = lease.as_mut() {
                    owner.absorb(lease)?;
                }
            }
        }
        Ok(edited)
    }

    /// Concatenate decoded streams exactly, without adding separators. Tokens,
    /// strings and operations can cross stream boundaries; empty streams remain
    /// represented by empty ranges. Ranges refer to the combined source bytes.
    pub fn from_decoded_streams(streams: Vec<Vec<u8>>, options: ParseOptions) -> PdfResult<Self> {
        if streams.len() > options.limits.max_objects {
            return Err(PdfError::ResourceLimit("content source streams".into()));
        }
        let mut total = 0usize;
        for stream in &streams {
            total = total
                .checked_add(stream.len())
                .filter(|size| *size <= options.limits.max_decoded_bytes)
                .ok_or_else(|| PdfError::ResourceLimit("content combined source bytes".into()))?;
        }
        let mut source = Vec::new();
        source
            .try_reserve_exact(total)
            .map_err(|_| PdfError::ResourceLimit("content combined source allocation".into()))?;
        let mut ranges = Vec::new();
        ranges
            .try_reserve_exact(streams.len())
            .map_err(|_| PdfError::ResourceLimit("content stream ranges allocation".into()))?;
        for stream in streams {
            let start = source.len();
            source.extend_from_slice(&stream);
            ranges.push(start..source.len());
        }
        Self::parse_with_stream_ranges(source, ranges, options)
    }

    pub fn from_decoded_bytes(source: Vec<u8>, options: ParseOptions) -> PdfResult<Self> {
        if source.len() > options.limits.max_decoded_bytes {
            return Err(PdfError::ResourceLimit(
                "content program source bytes".into(),
            ));
        }
        let mut ranges = Vec::new();
        ranges
            .try_reserve_exact(1)
            .map_err(|_| PdfError::ResourceLimit("content stream range allocation".into()))?;
        ranges.push(0..source.len());
        Self::parse_with_stream_ranges(source, ranges, options)
    }

    /// Consume decoded bytes from this pool without copying the source.
    /// The offline result retains source, range, operation and operand leases.
    /// A source owned by another pool is rejected, not silently recharged.
    pub fn from_decoded_bytes_with_budget(
        source: crate::budget::BudgetedBytes,
        options: ParseOptions,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<BudgetedContentProgram> {
        if source.as_slice().len() > options.limits.max_decoded_bytes {
            return Err(PdfError::ResourceLimit(
                "content program source bytes".into(),
            ));
        }
        let mut lease = budget.reserve(0)?;
        let (source, mut source_lease) = source.into_reserved_parts();
        if let Err(error) = lease.absorb(&mut source_lease) {
            drop(source);
            drop(source_lease);
            return Err(error);
        }
        let mut ranges_lease = reserve_metadata::<Range<usize>>(Some(budget), 1)?;
        let mut ranges = Vec::new();
        ranges
            .try_reserve_exact(1)
            .map_err(|_| PdfError::ResourceLimit("content stream range allocation".into()))?;
        reconcile_metadata::<Range<usize>>(&mut ranges_lease, ranges.capacity())?;
        ranges.push(0..source.len());
        let mut operations_lease = Some(budget.reserve(0)?);
        let program = Self::parse_with_stream_ranges_budget(
            source,
            ranges,
            options,
            Some(budget),
            &mut operations_lease,
        )?;
        lease.absorb(ranges_lease.as_mut().unwrap())?;
        lease.absorb(operations_lease.as_mut().unwrap())?;
        Ok(BudgetedContentProgram {
            program,
            _leases: ProgramLeases {
                _allocation: Some(lease),
            },
        })
    }

    // Callers already own validated boundaries. Reuse them through parsing
    // rather than allocating a default range table only to discard it.
    fn parse_with_stream_ranges(
        source: Vec<u8>,
        stream_ranges: Vec<Range<usize>>,
        options: ParseOptions,
    ) -> PdfResult<Self> {
        Self::parse_with_stream_ranges_budget(source, stream_ranges, options, None, &mut None)
    }

    fn parse_with_stream_ranges_budget(
        source: Vec<u8>,
        stream_ranges: Vec<Range<usize>>,
        options: ParseOptions,
        budget: Option<&crate::budget::OperationBudget>,
        operations_lease: &mut Option<crate::budget::MemoryLease>,
    ) -> PdfResult<Self> {
        if source.len() > options.limits.max_decoded_bytes {
            return Err(PdfError::ResourceLimit(
                "content program source bytes".into(),
            ));
        }
        // Parsing produces owned operators; borrow the retained source bytes
        // instead of allocating a second complete input buffer.
        let mut operands_lease = budget.map(|budget| budget.reserve(0)).transpose()?;
        let mut parser = ContentParser::new(SyntaxParser::with_options(
            StreamReader::try_new(Cursor::new(source.as_slice()))?,
            options,
        ));
        let mut operations = Vec::new();
        loop {
            let start = usize::try_from(parser.position())
                .map_err(|_| PdfError::ResourceLimit("content source position".into()))?;
            let operator = if let (Some(budget), Some(lease)) = (budget, operands_lease.as_mut()) {
                parser.read_operator_with_budget(budget, lease)?
            } else {
                parser.read_operator()?
            };
            let Some(operator) = operator else {
                break;
            };
            if operations.len() >= options.limits.max_content_operations {
                return Err(PdfError::ResourceLimit("content program operations".into()));
            }
            let end = usize::try_from(parser.position())
                .map_err(|_| PdfError::ResourceLimit("content source position".into()))?;
            if start > end || end > source.len() {
                return Err(PdfError::ContentParseError(
                    "content source range is invalid".into(),
                ));
            }
            if let Some(budget) = budget {
                if operations.len() == operations.capacity() {
                    let capacity = operations
                        .capacity()
                        .saturating_mul(2)
                        .max(1)
                        .min(options.limits.max_content_operations);
                    let bytes = capacity
                        .checked_mul(std::mem::size_of::<SourceOperation>())
                        .ok_or_else(|| {
                            PdfError::ResourceLimit("content operation table size".into())
                        })?;
                    let mut lease = budget.reserve(bytes)?;
                    let mut replacement = Vec::new();
                    replacement.try_reserve_exact(capacity).map_err(|_| {
                        PdfError::ResourceLimit("content program operations allocation".into())
                    })?;
                    lease.resize(
                        replacement
                            .capacity()
                            .checked_mul(std::mem::size_of::<SourceOperation>())
                            .ok_or_else(|| {
                                PdfError::ResourceLimit("content operation table capacity".into())
                            })?,
                    )?;
                    replacement.append(&mut operations);
                    operations = replacement;
                    *operations_lease = Some(lease);
                }
            } else {
                operations.try_reserve(1).map_err(|_| {
                    PdfError::ResourceLimit("content program operations allocation".into())
                })?;
            }
            operations.push(SourceOperation {
                operator,
                source_range: start..end,
                lexical_range: skip_source_prefix(&source, start, end)..end,
            });
        }
        drop(parser);
        if let (Some(owner), Some(operands)) = (operations_lease.as_mut(), operands_lease.as_mut())
        {
            owner.absorb(operands)?;
        }
        Ok(Self {
            source,
            operations,
            stream_ranges,
            stream_object_ids: None,
            source_stamp: None,
            source_page_id: None,
        })
    }

    /// Original bytes, including comments, spacing, unknown operators and tail.
    pub fn source_bytes(&self) -> &[u8] {
        &self.source
    }
    pub fn operations(&self) -> &[SourceOperation] {
        &self.operations
    }

    /// Locate an operation across original decoded streams without allocation.
    /// Includes its leading whitespace/comments. Empty streams yield no fragment.
    /// Ranges are decoded byte offsets, not offsets in compressed file streams.
    pub fn operation_source_fragments(
        &self,
        index: usize,
    ) -> PdfResult<impl Iterator<Item = OperationSourceFragment> + '_> {
        let operation = self.operations.get(index).ok_or_else(|| {
            PdfError::ContentParseError("content operation index out of range".into())
        })?;
        let range = operation.source_range.clone();
        Ok(self
            .stream_ranges
            .iter()
            .enumerate()
            .filter_map(move |(stream_index, stream)| {
                let start = stream.start.max(range.start);
                let end = stream.end.min(range.end);
                (start < end).then(|| OperationSourceFragment {
                    stream_index,
                    stream_range: start - stream.start..end - stream.start,
                    combined_range: start..end,
                })
            }))
    }

    pub fn stream_ranges(&self) -> &[Range<usize>] {
        &self.stream_ranges
    }

    /// Document/revision provenance for page-created programs; absent for raw bytes.
    pub fn source_stamp(&self) -> Option<&crate::document::identity::DocumentStamp> {
        self.source_stamp.as_ref()
    }

    /// Original indirect stream ID. Direct streams and raw byte sources have no ID.
    /// IDs are meaningful only with source_stamp's document identity/revision.
    pub fn stream_object_id(&self, index: usize) -> Option<crate::objects::object_id::ObjectId> {
        self.stream_object_ids
            .as_ref()?
            .get(index)
            .copied()
            .flatten()
    }

    pub fn stream_bytes(&self, index: usize) -> Option<&[u8]> {
        self.stream_ranges
            .get(index)
            .and_then(|range| self.source.get(range.clone()))
    }
}

fn reserve_replacement_bytes(
    bytes: &mut Vec<u8>,
    required: usize,
    budget: Option<&crate::budget::OperationBudget>,
    lease: &mut Option<crate::budget::MemoryLease>,
) -> PdfResult<()> {
    if required <= bytes.capacity() {
        return Ok(());
    }
    if let Some(budget) = budget {
        let mut allocation = budget.reserve(required)?;
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(required)
            .map_err(|_| PdfError::ResourceLimit("TJ replacement allocation".into()))?;
        allocation.resize(replacement.capacity())?;
        replacement.extend_from_slice(bytes);
        *bytes = replacement;
        *lease = Some(allocation);
    } else {
        bytes
            .try_reserve_exact(required - bytes.len())
            .map_err(|_| PdfError::ResourceLimit("TJ replacement allocation".into()))?;
    }
    Ok(())
}

fn skip_source_prefix(bytes: &[u8], mut position: usize, end: usize) -> usize {
    loop {
        while position < end && matches!(bytes[position], 0 | 9 | 10 | 12 | 13 | 32) {
            position += 1;
        }
        if position >= end || bytes[position] != b'%' {
            return position;
        }
        while position < end && !matches!(bytes[position], b'\n' | b'\r') {
            position += 1;
        }
    }
}
