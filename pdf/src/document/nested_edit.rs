use super::{resource_manager::ResourceKind, PdfDocument};
use crate::{
    error::{PdfError, PdfResult},
    objects::{
        object_id::ObjectId, pdf_dict::PdfDict, pdf_name::PdfName, pdf_reference::PdfReference,
        pdf_stream::PdfStream, PdfObject,
    },
    page::{content_program::ContentProgram, source_provenance::NestedDrawSource},
};
use std::io::{Read, Seek};

enum NestedImageEdit<'a> {
    Remove,
    Transform([f32; 6]),
    Replace(&'a super::identity::ObjectHandle),
}

enum NestedTextEdit {
    WholeString {
        item: Option<usize>,
    },
    StringRange {
        item: Option<usize>,
        range: std::ops::Range<usize>,
    },
    ArrayRange(std::ops::Range<(usize, usize)>),
}

impl<R: Read + Seek> PdfDocument<R> {
    /// Replace one geometry operation belonging to a nested path event.
    /// Coordinates remain in the leaf operation's current user space.
    pub fn replace_nested_path_numbers(
        &self,
        page: u32,
        root: &ContentProgram,
        source: &NestedDrawSource,
        geometry_operation: usize,
        values: &[f32],
    ) -> PdfResult<()> {
        let chain = self.get_page(page)?.prepare_form_chain(root, source)?;
        let leaf = &chain
            .last()
            .ok_or_else(|| PdfError::PageResourceError("Nested leaf absent".into()))?
            .program;
        let paint = &leaf.operations()[source.operation_index].operator;
        if paint.num_operands() != 0
            || !matches!(
                paint.name(),
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*"
            )
            || !source
                .geometry_operation_indices
                .contains(&geometry_operation)
        {
            return Err(PdfError::ObjectError(
                "Nested geometry must belong to the selected path".into(),
            ));
        }
        // Painting and n consume the current path; q/Q do not save it.
        // Reconstruct membership without trusting the caller's index list.
        let prefix = &leaf.operations()[..source.operation_index];
        for operation in prefix {
            let op = &operation.operator;
            if !crate::page::opcode::is_op(op.name().as_bytes()) {
                return Err(PdfError::UnsupportedFeature(
                    "nested path edit across unknown operator".into(),
                ));
            }
            if crate::page::opcode::operand_count(op.name())
                .is_some_and(|count| op.num_operands() != count)
            {
                return Err(PdfError::ContentParseError(
                    "nested path prefix operand count".into(),
                ));
            }
        }
        let start = prefix
            .iter()
            .rposition(|operation| {
                matches!(
                    operation.operator.name(),
                    "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n"
                )
            })
            .map_or(0, |index| index + 1);
        let actual = prefix
            .iter()
            .enumerate()
            .skip(start)
            .filter_map(|(index, operation)| {
                matches!(
                    operation.operator.name(),
                    "m" | "l" | "c" | "v" | "y" | "re" | "h"
                )
                .then_some(index)
            });
        if !actual.eq(source.geometry_operation_indices.iter().copied()) {
            return Err(PdfError::ObjectError(
                "Nested path geometry source mismatch".into(),
            ));
        }
        let target = &leaf.operations()[geometry_operation].operator;
        let replacement =
            super::content_edit::path_parameter_replacement(target, values, self.ctx.limits())?;
        let edited =
            leaf.with_replaced_operation(geometry_operation, &replacement, self.ctx.options())?;
        self.commit_nested_edit(page, root, source, chain, edited)
    }

    /// Stop one nested Form path paint, preserving geometry and pending clipping.
    /// Copies every selected ancestor and rebinds only that invocation. Original
    /// streams/resources remain available; this is logical removal, not erasure.
    pub fn remove_nested_path_draw(
        &self,
        page: u32,
        root: &ContentProgram,
        source: &NestedDrawSource,
    ) -> PdfResult<()> {
        let chain = self.get_page(page)?.prepare_form_chain(root, source)?;
        let leaf = &chain
            .last()
            .ok_or_else(|| PdfError::PageResourceError("Nested leaf absent".into()))?
            .program;
        let paint = &leaf.operations()[source.operation_index].operator;
        if paint.num_operands() != 0
            || !matches!(
                paint.name(),
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*"
            )
        {
            return Err(PdfError::ObjectError(
                "Nested path removal requires a painting operation".into(),
            ));
        }
        let edited =
            leaf.with_replaced_operation(source.operation_index, b"n", self.ctx.options())?;
        self.commit_nested_edit(page, root, source, chain, edited)
    }

    /// Clear strings in one nested text-show event. Keeps TJ adjustments and
    /// quote state effects; removed glyph advances are not compensated.
    pub fn remove_nested_text_draw(
        &self,
        page: u32,
        root: &ContentProgram,
        source: &NestedDrawSource,
    ) -> PdfResult<()> {
        let chain = self.get_page(page)?.prepare_form_chain(root, source)?;
        let leaf = &chain
            .last()
            .ok_or_else(|| PdfError::PageResourceError("Nested leaf absent".into()))?
            .program;
        let edited = leaf.with_cleared_text_draw(source.operation_index, self.ctx.options())?;
        self.commit_nested_edit(page, root, source, chain, edited)
    }

    /// Replace nested Tj/quote text using its effective existing font encoding.
    /// Tracks inherited caller fonts through the selected Form invocation chain.
    pub fn replace_nested_text(
        &self,
        page: u32,
        root: &ContentProgram,
        source: &NestedDrawSource,
        text: &str,
    ) -> PdfResult<()> {
        self.replace_nested_unicode(
            page,
            root,
            source,
            NestedTextEdit::WholeString { item: None },
            text,
        )
    }

    /// Encode Unicode into one nested TJ string item, preserving adjustments.
    pub fn replace_nested_text_array_item(
        &self,
        page: u32,
        root: &ContentProgram,
        source: &NestedDrawSource,
        item: usize,
        text: &str,
    ) -> PdfResult<()> {
        self.replace_nested_unicode(
            page,
            root,
            source,
            NestedTextEdit::WholeString { item: Some(item) },
            text,
        )
    }

    /// Replace a character-code byte range within one nested text string.
    /// Range endpoints must be complete code boundaries, not Unicode indices.
    pub fn replace_nested_text_range(
        &self,
        page: u32,
        root: &ContentProgram,
        source: &NestedDrawSource,
        item: Option<usize>,
        range: std::ops::Range<usize>,
        text: &str,
    ) -> PdfResult<()> {
        self.replace_nested_unicode(
            page,
            root,
            source,
            NestedTextEdit::StringRange { item, range },
            text,
        )
    }

    /// Replace a range across nested TJ strings, retaining numeric adjustments.
    /// Endpoints are (array index, decoded code-byte offset); no compensation.
    pub fn replace_nested_text_array_range(
        &self,
        page: u32,
        root: &ContentProgram,
        source: &NestedDrawSource,
        range: std::ops::Range<(usize, usize)>,
        text: &str,
    ) -> PdfResult<()> {
        self.replace_nested_unicode(page, root, source, NestedTextEdit::ArrayRange(range), text)
    }

    fn replace_nested_unicode(
        &self,
        page: u32,
        root: &ContentProgram,
        source: &NestedDrawSource,
        edit: NestedTextEdit,
        text: &str,
    ) -> PdfResult<()> {
        let current_page = self.get_page(page)?;
        let budget = self.ctx.limits().operation_budget();
        let chain = current_page.prepare_form_chain_with_budget(root, source, &budget)?;
        let target = &chain
            .last()
            .and_then(|leaf| leaf.program.operations().get(source.operation_index))
            .ok_or_else(|| PdfError::ObjectError("nested text operation absent".into()))?
            .operator;
        match &edit {
            NestedTextEdit::WholeString { item } | NestedTextEdit::StringRange { item, .. } => {
                let string = match (target.name(), *item) {
                    ("TJ", Some(index)) => target
                        .get_as_array(0)?
                        .get(index)
                        .and_then(PdfObject::as_string)
                        .ok_or_else(|| {
                            PdfError::ObjectError("TJ target requires a string item".into())
                        })?,
                    ("Tj" | "'", None) => target.get_as_string(0)?,
                    ("\"", None) => target.get_as_string(2)?,
                    _ => {
                        return Err(PdfError::ObjectError(
                            "nested text target/item mismatch".into(),
                        ))
                    }
                };
                if let NestedTextEdit::StringRange { range, .. } = &edit {
                    if range.start > range.end || range.end > string.bytes().len() {
                        return Err(PdfError::ObjectError(
                            "text code range out of bounds".into(),
                        ));
                    }
                }
            }
            NestedTextEdit::ArrayRange(range) => {
                if target.name() != "TJ" || target.num_operands() != 1 {
                    return Err(PdfError::ObjectError(
                        "invalid nested TJ range target".into(),
                    ));
                }
                super::content_edit::validate_text_array_range(
                    target.get_as_array(0)?,
                    range,
                    self.ctx.limits(),
                )?;
            }
        }
        let (page_scope, _page_scope_lease) = current_page.resource_with_budget(&budget)?;
        let mut font = None;
        for (index, call) in source.invocations.iter().enumerate() {
            let (program, scope) = if index == 0 {
                (root, &page_scope)
            } else {
                (&*chain[index - 1].program, &chain[index - 1].resource_scope)
            };
            font = current_page.font_before_operation_in_scope_with_budget(
                program,
                call.operation_index,
                scope,
                font,
                &budget,
            )?;
        }
        let leaf = chain
            .last()
            .ok_or_else(|| PdfError::PageResourceError("Nested leaf absent".into()))?;
        let font = current_page
            .font_before_operation_in_scope_with_budget(
                &leaf.program,
                source.operation_index,
                &leaf.resource_scope,
                font,
                &budget,
            )?
            .ok_or_else(|| PdfError::FontError("Nested text has no effective font".into()))?;
        let mut codes = font.encode_text_with_budget(text, self.ctx.limits(), &budget)?;
        let mut range_lease = budget.reserve(0)?;
        let (item, range) = match edit {
            NestedTextEdit::WholeString { item } => (item, None),
            NestedTextEdit::StringRange { item, range } => (item, Some(range)),
            NestedTextEdit::ArrayRange(range) => {
                let target = &leaf.program.operations()[source.operation_index].operator;
                if target.name() != "TJ" || target.num_operands() != 1 {
                    return Err(PdfError::ObjectError("array range requires TJ".into()));
                }
                let replacements = super::content_edit::budgeted_text_array_range_replacements(
                    &budget,
                    &font,
                    target.get_as_array(0)?,
                    range,
                    codes.as_slice(),
                    self.ctx.limits(),
                )?;
                let borrowed =
                    super::content_edit::BudgetedTextItems::new(&budget, replacements.as_slice())?;
                let edited = leaf
                    .program
                    .with_replaced_text_array_items_codes_with_budget(
                        source.operation_index,
                        borrowed.as_slice(),
                        self.ctx.options(),
                        &budget,
                    )?;
                return self.commit_nested_edit(page, root, source, chain, edited);
            }
        };
        if let Some(range) = range {
            let target = &leaf.program.operations()[source.operation_index].operator;
            let string = match (target.name(), item) {
                ("TJ", Some(item)) => target
                    .get_as_array(0)?
                    .get(item)
                    .and_then(PdfObject::as_string)
                    .ok_or_else(|| {
                        PdfError::ObjectError("TJ range requires a string array item".into())
                    })?,
                ("Tj" | "'", None) => target.get_as_string(0)?,
                ("\"", None) => target.get_as_string(2)?,
                _ => {
                    return Err(PdfError::ObjectError(
                        "text range target/item mismatch".into(),
                    ))
                }
            };
            let old = string.bytes();
            if range.start > range.end || range.end > old.len() {
                return Err(PdfError::ObjectError(
                    "text code range out of bounds".into(),
                ));
            }
            let size = old
                .len()
                .checked_sub(range.len())
                .and_then(|size| size.checked_add(codes.as_slice().len()))
                .ok_or_else(|| PdfError::ResourceLimit("text range replacement bytes".into()))?;
            range_lease.resize(size)?;
            let replacement = super::content_edit::text_range_replacement(
                &font,
                old,
                range,
                codes.as_slice(),
                self.ctx.limits(),
            )?;
            // Construction charges the new capacity while the old codes are
            // still owned; assignment drops the old allocation and its lease.
            codes = crate::budget::BudgetedBytes::from_reserved_vec(replacement, range_lease)?;
        }
        let edited = match item {
            Some(item) => leaf
                .program
                .with_replaced_text_array_items_codes_with_budget(
                    source.operation_index,
                    &[(item, codes.as_slice())],
                    self.ctx.options(),
                    &budget,
                )?,
            None => leaf.program.with_replaced_text_codes_with_budget(
                source.operation_index,
                codes.as_slice(),
                self.ctx.options(),
                &budget,
            )?,
        };
        self.commit_nested_edit(page, root, source, chain, edited)
    }

    /// Replace raw PDF character-code bytes in a nested Tj/quote event.
    /// Uses the existing font; no Unicode encoding, shaping or advance compensation.
    pub fn replace_nested_text_codes(
        &self,
        page: u32,
        root: &ContentProgram,
        source: &NestedDrawSource,
        codes: &[u8],
    ) -> PdfResult<()> {
        let chain = self.get_page(page)?.prepare_form_chain(root, source)?;
        let leaf = &chain
            .last()
            .ok_or_else(|| PdfError::PageResourceError("Nested leaf absent".into()))?
            .program;
        let edited =
            leaf.with_replaced_text_codes(source.operation_index, codes, self.ctx.options())?;
        self.commit_nested_edit(page, root, source, chain, edited)
    }

    /// Replace one complete string item in a nested TJ array with raw codes.
    /// Item indices count numeric entries too; other adjustments remain unchanged.
    pub fn replace_nested_text_array_item_codes(
        &self,
        page: u32,
        root: &ContentProgram,
        source: &NestedDrawSource,
        item: usize,
        codes: &[u8],
    ) -> PdfResult<()> {
        let chain = self.get_page(page)?.prepare_form_chain(root, source)?;
        let leaf = &chain
            .last()
            .ok_or_else(|| PdfError::PageResourceError("Nested leaf absent".into()))?
            .program;
        let edited = leaf.with_replaced_text_array_item_codes(
            source.operation_index,
            item,
            codes,
            self.ctx.options(),
        )?;
        self.commit_nested_edit(page, root, source, chain, edited)
    }

    /// Remove one image Do in a Form leaf, retaining shared image resources.
    /// Inline images and Form Do targets are not accepted; not secure erasure.
    pub fn remove_nested_image_draw(
        &self,
        page: u32,
        root: &ContentProgram,
        source: &NestedDrawSource,
    ) -> PdfResult<()> {
        self.edit_nested_image_draw(page, root, source, NestedImageEdit::Remove)
    }

    /// Apply a relative current-user-space transform to one nested image Do.
    /// Copies its Form chain and isolates the transform with q/cm/Do/Q.
    pub fn transform_nested_image_draw(
        &self,
        page: u32,
        root: &ContentProgram,
        source: &NestedDrawSource,
        matrix: [f32; 6],
    ) -> PdfResult<()> {
        self.edit_nested_image_draw(page, root, source, NestedImageEdit::Transform(matrix))
    }

    /// Rebind one nested image call to a same-document image handle.
    /// Preserves the existing transform and original shared Form resources.
    pub fn replace_nested_image_draw(
        &self,
        page: u32,
        root: &ContentProgram,
        source: &NestedDrawSource,
        image: &super::identity::ObjectHandle,
    ) -> PdfResult<()> {
        self.edit_nested_image_draw(page, root, source, NestedImageEdit::Replace(image))
    }

    fn edit_nested_image_draw(
        &self,
        page: u32,
        root: &ContentProgram,
        source: &NestedDrawSource,
        edit: NestedImageEdit<'_>,
    ) -> PdfResult<()> {
        let mut chain = self.get_page(page)?.prepare_form_chain(root, source)?;
        let leaf = chain
            .last_mut()
            .ok_or_else(|| PdfError::PageResourceError("Nested leaf absent".into()))?;
        let operation = &leaf.program.operations()[source.operation_index].operator;
        if operation.name() != "Do" || operation.num_operands() != 1 {
            return Err(PdfError::ObjectError(
                "Nested image edit requires a named Do".into(),
            ));
        }
        let entries = self
            .ctx
            .resolve_owned(
                leaf.resource_scope
                    .get("XObject")
                    .ok_or(PdfError::PageXobjectNotFound)?,
            )?
            .into_dict()
            .ok_or(PdfError::PageXobjectIsNotDict)?;
        let image = self.ctx.resolve_owned(
            entries
                .get(operation.get_as_name(0)?.name())
                .ok_or(PdfError::PageXobjectNotFound)?,
        )?;
        let stream = image
            .as_stream()
            .ok_or_else(|| PdfError::ObjectError("Nested image target must be stream".into()))?;
        let subtype = stream
            .dict()
            .get("Subtype")
            .ok_or_else(|| PdfError::ObjectError("Nested image subtype missing".into()))?;
        if self
            .ctx
            .resolve_owned(subtype)?
            .as_name()
            .map(|name| name.name())
            != Some("Image")
        {
            return Err(PdfError::ObjectError(
                "Nested image target requires Subtype Image".into(),
            ));
        }
        let edited = match edit {
            NestedImageEdit::Replace(replacement) => {
                let id = self.validate_handle(replacement)?;
                let object = self.resolve_object(id)?;
                let stream = object.as_stream().ok_or_else(|| {
                    PdfError::ObjectError("replacement image must be a stream".into())
                })?;
                let subtype = stream.dict().get("Subtype").ok_or_else(|| {
                    PdfError::ObjectError("replacement image subtype missing".into())
                })?;
                if self
                    .ctx
                    .resolve_owned(subtype)?
                    .as_name()
                    .map(|name| name.name())
                    != Some("Image")
                {
                    return Err(PdfError::ObjectError(
                        "replacement requires Subtype Image".into(),
                    ));
                }
                // This only changes the prepared local dictionary; object writes
                // and ancestor registration happen in commit_nested_edit's transaction.
                let name = self.bind_local_xobject(&mut leaf.resource_scope, id)?;
                leaf.program.with_replaced_operation(
                    source.operation_index,
                    &form_do(&name)?,
                    self.ctx.options(),
                )?
            }
            NestedImageEdit::Transform(matrix) => leaf.program.with_transformed_xobject_operation(
                source.operation_index,
                matrix,
                self.ctx.options(),
            )?,
            NestedImageEdit::Remove => leaf
                .program
                .without_operation(source.operation_index, self.ctx.options())?,
        };
        self.commit_nested_edit(page, root, source, chain, edited)
    }

    fn commit_nested_edit<P: AsRef<ContentProgram>>(
        &self,
        page: u32,
        root: &ContentProgram,
        source: &NestedDrawSource,
        chain: Vec<crate::page::source_provenance::PreparedForm>,
        edited: P,
    ) -> PdfResult<()> {
        self.edit_transaction(|doc| {
            doc.commit_page_content_edit(page, root, || {
                let mut child = None;
                let mut bytes = 0usize;
                for (index, mut form) in chain.into_iter().enumerate().rev() {
                    let rebuilt;
                    let program = if let Some(child_id) = child {
                        let name = doc.bind_local_xobject(&mut form.resource_scope, child_id)?;
                        rebuilt = form.program.with_replaced_operation(
                            source.invocations[index + 1].operation_index,
                            &form_do(&name)?,
                            doc.ctx.options(),
                        )?;
                        &rebuilt
                    } else {
                        // Keep a budgeted leaf owner alive through the entire
                        // transaction; borrowing must not detach its leases.
                        edited.as_ref()
                    };
                    bytes = checked_edited_bytes(
                        bytes,
                        program.source_bytes().len(),
                        doc.ctx.limits().max_decoded_bytes,
                    )?;
                    // Materialize the effective scope, including legacy page
                    // fallback, so new page bindings cannot change copied names.
                    form.dictionary
                        .try_insert("Resources".into(), PdfObject::PdfDict(form.resource_scope))?;
                    let mut data = Vec::new();
                    data.try_reserve_exact(program.source_bytes().len())
                        .map_err(|_| {
                            PdfError::ResourceLimit("Nested edited stream allocation".into())
                        })?;
                    data.extend_from_slice(program.source_bytes());
                    child = Some(doc.try_insert_object(PdfObject::PdfStream(
                        PdfStream::from_decoded_data(form.dictionary, data)?,
                    ))?);
                }
                let name = doc.resources().register_existing(
                    page,
                    ResourceKind::Form,
                    child
                        .ok_or_else(|| PdfError::ObjectError("Nested copied root absent".into()))?,
                )?;
                let edited_root = root.with_replaced_operation(
                    source.invocations[0].operation_index,
                    &form_do(&name)?,
                    doc.ctx.options(),
                )?;
                checked_edited_bytes(
                    bytes,
                    edited_root.source_bytes().len(),
                    doc.ctx.limits().max_decoded_bytes,
                )?;
                Ok(edited_root)
            })
        })
    }

    /// Add a collision-free binding to a prepared local XObject dictionary.
    /// This mutates only the prepared scope; callers validate the subtype and
    /// commit the containing Form inside an edit transaction.
    fn bind_local_xobject(&self, resources: &mut PdfDict, id: ObjectId) -> PdfResult<String> {
        let mut entries = match resources.get("XObject") {
            Some(value) => self
                .ctx
                .resolve_owned(value)?
                .into_dict()
                .ok_or(PdfError::PageXobjectIsNotDict)?,
            None => PdfDict::default(),
        };
        if (&entries).into_iter().count() >= self.ctx.limits().max_objects {
            return Err(PdfError::ResourceLimit(
                "Nested Form resource entries".into(),
            ));
        }
        for index in 1..=self.ctx.limits().max_objects {
            let name = format!("FmEdit{index}");
            if name.len() > self.ctx.limits().max_token_bytes {
                return Err(PdfError::ResourceLimit(
                    "Nested Form resource name bytes".into(),
                ));
            }
            if entries.get(&name).is_none() {
                entries.try_insert(
                    name.clone(),
                    PdfObject::PdfReference(PdfReference::from_id(id)),
                )?;
                resources.try_insert("XObject".into(), PdfObject::PdfDict(entries))?;
                return Ok(name);
            }
        }
        Err(PdfError::ResourceLimit("Nested Form resource names".into()))
    }
}

fn checked_edited_bytes(retained: usize, next: usize, limit: usize) -> PdfResult<usize> {
    retained
        .checked_add(next)
        .filter(|total| *total <= limit)
        .ok_or_else(|| PdfError::ResourceLimit("Nested edited decoded bytes".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_leaf_rebuild_shares_encoding_peak_and_rejects_before_mutation() {
        use crate::{
            budget::OperationBudget,
            document::resource_manager::StandardFont,
            io::stream_reader::StreamReader,
            parser::{
                options::{Limits, ParseOptions},
                syntax::SyntaxParser,
            },
        };
        use std::io::Cursor;
        let fixture = |maximum| {
            let doc = PdfDocument::new_with_options(ParseOptions {
                limits: Limits {
                    max_operation_bytes: maximum,
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();
            doc.add_page(0).unwrap();
            let font = doc
                .resources()
                .register_standard_font(0, StandardFont::Helvetica)
                .unwrap();
            let mut dictionary = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(
                    b"<< /Subtype /Form /BBox [0 0 100 100] >>".to_vec(),
                ))
                .unwrap(),
            )
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap();
            dictionary.insert(
                "Resources".into(),
                PdfObject::PdfDict(doc.get_page(0).unwrap().resource().unwrap()),
            );
            let id = doc
                .try_insert_object(PdfObject::PdfStream(
                    PdfStream::from_decoded_data(
                        dictionary,
                        // Untouched source must coexist with the rebuilt result;
                        // keep this peak above font setup/encoding temporaries.
                        format!("%{}\nBT /{font} 12 Tf (AB) Tj ET", "p".repeat(131072))
                            .into_bytes(),
                    )
                    .unwrap(),
                ))
                .unwrap();
            let name = doc
                .resources()
                .register_existing(0, ResourceKind::Form, id)
                .unwrap();
            doc.replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes())
                .unwrap();
            doc
        };
        let probe = fixture(8 * 1024 * 1024);
        let mut page = probe.get_page(0).unwrap();
        let root = page.content_program().unwrap();
        let report = page.extract_characters_paint_order_report().unwrap();
        let source = report.characters[0].nested_source.as_ref().unwrap();
        let pool = OperationBudget::new(8 * 1024 * 1024);
        let chain = page
            .prepare_form_chain_with_budget(&root, source, &pool)
            .unwrap();
        let leaf = chain.last().unwrap();
        let text = "XYZ".to_owned();
        let (scope, scope_lease) = page.resource_with_budget(&pool).unwrap();
        let inherited = page
            .font_before_operation_in_scope_with_budget(
                &root,
                source.invocations[0].operation_index,
                &scope,
                None,
                &pool,
            )
            .unwrap();
        let font = page
            .font_before_operation_in_scope_with_budget(
                &leaf.program,
                source.operation_index,
                &leaf.resource_scope,
                inherited,
                &pool,
            )
            .unwrap()
            .unwrap();
        let retained = pool.live_bytes();
        let codes = font
            .encode_text_with_budget(&text, probe.ctx.limits(), &pool)
            .unwrap();
        let encoding_peak = pool.peak_bytes();
        let edited = leaf
            .program
            .with_replaced_text_codes_with_budget(
                source.operation_index,
                codes.as_slice(),
                probe.ctx.options(),
                &pool,
            )
            .unwrap();
        let peak = pool.peak_bytes();
        assert!(
            peak > encoding_peak,
            "combined={peak}, encoding={encoding_peak}"
        );
        assert!(pool.live_bytes() > codes.capacity());
        drop(edited);
        assert_eq!(pool.live_bytes(), retained + codes.capacity());
        drop(codes);
        assert_eq!(pool.live_bytes(), retained);
        drop(font);
        drop(scope_lease);
        drop(chain);
        assert_eq!(pool.live_bytes(), 0);
        for (maximum, succeeds) in [(peak - 1, false), (peak, true)] {
            let doc = fixture(maximum);
            let mut page = doc.get_page(0).unwrap();
            let root = page.content_program().unwrap();
            let report = page.extract_characters_paint_order_report().unwrap();
            let source = report.characters[0].nested_source.as_ref().unwrap();
            let revision = doc.revision();
            let resources = page.resource().unwrap();
            let result = doc.replace_nested_text(0, &root, source, &text);
            assert_eq!(result.is_ok(), succeeds);
            if !succeeds {
                assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
                assert_eq!(doc.revision(), revision);
                let unchanged = doc.get_page(0).unwrap().content_program().unwrap();
                assert_eq!(unchanged.source_bytes(), root.source_bytes());
                assert_eq!(unchanged.stream_object_id(0), root.stream_object_id(0));
                assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
                doc.replace_nested_text(0, &root, source, "X").unwrap();
            }
        }
    }

    #[test]
    fn nested_edited_root_and_form_bytes_share_exact_limit() {
        let forms = checked_edited_bytes(0, 10, 25).unwrap();
        let forms = checked_edited_bytes(forms, 5, 25).unwrap();
        assert_eq!(checked_edited_bytes(forms, 10, 25).unwrap(), 25);
        assert!(checked_edited_bytes(forms, 11, 25).is_err());
        assert!(checked_edited_bytes(usize::MAX, 1, usize::MAX).is_err());
        assert_eq!(checked_edited_bytes(0, 0, 0).unwrap(), 0);
    }
}

fn form_do(name: &str) -> PdfResult<Vec<u8>> {
    let mut bytes = Vec::new();
    crate::writer::write_name(&mut bytes, &PdfName::new(name.to_owned()))?;
    bytes.extend_from_slice(b" Do");
    Ok(bytes)
}
