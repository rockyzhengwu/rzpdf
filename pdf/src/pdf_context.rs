use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::io::{Read, Seek};

use crate::document::cross_ref_table::ObjectState;
use crate::document::store::StoreEntry;
use crate::objects::object_streams::ObjectStreams;
use crate::{
    document::cross_ref_table::CrossRefTable,
    error::{PdfError, PdfResult},
    objects::{object_id::ObjectId, pdf_dict::PdfDict, pdf_reference::PdfReference, PdfObject},
    parser::parser::PdfParser,
};

#[derive(Debug)]
pub struct PDFContext<R: Seek + Read> {
    parser: Option<RefCell<PdfParser<R>>>,
    mode: crate::parser::syntax::ParseMode,
    cross_ref_table: CrossRefTable,
    indirect_objects: HashMap<ObjectId, OnceCell<PdfObject>>,
    limits: crate::parser::options::Limits,
    loading_objects: RefCell<Vec<ObjectId>>,
    decoding_depth: Cell<usize>,
    color_depth: Cell<usize>,
    cached_stream_bytes: Cell<usize>,
    source_resolution_depth: Cell<usize>,
    state: crate::document::state::DocumentState,
}

struct DecodeGuard<'a>(&'a Cell<usize>);
impl Drop for DecodeGuard<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

struct LoadingGuard<'a>(&'a RefCell<Vec<ObjectId>>);
impl Drop for LoadingGuard<'_> {
    fn drop(&mut self) {
        self.0.borrow_mut().pop();
    }
}

#[cfg(test)]
mod loading_tests {
    #[test]
    fn budgeted_owned_resolution_preserves_store_deletion_and_source_scope() {
        use crate::{
            budget::OperationBudget,
            objects::{pdf_reference::PdfReference, pdf_string::PdfString},
        };
        let ctx: super::PDFContext<std::io::Cursor<Vec<u8>>> =
            super::PDFContext::new_without_source(PdfDict::default(), Default::default());
        let object = PdfObject::PdfString(PdfString::new(b"abc".to_vec(), true));
        let id = ctx.try_insert_object(object.clone()).unwrap();
        let reference = PdfObject::PdfReference(PdfReference::from_id(id));
        let budget = OperationBudget::new(3);
        let neighbor = budget.reserve(1).unwrap();
        for input in [&object, &reference] {
            assert!(ctx.resolve_owned_with_budget(input, &budget).is_err());
            assert_eq!(budget.live_bytes(), 1);
        }
        drop(neighbor);
        for input in [&object, &reference] {
            let (copy, lease) = ctx.resolve_owned_with_budget(input, &budget).unwrap();
            assert_eq!(copy, object);
            assert_eq!(budget.live_bytes(), 3);
            drop(copy);
            drop(lease);
            assert_eq!(budget.live_bytes(), 0);
        }
        assert!(ctx
            .with_source_resolution(|| ctx.resolve_owned_with_budget(&reference, &budget))
            .is_err());
        assert_eq!(ctx.source_resolution_depth.get(), 0);
        assert_eq!(budget.live_bytes(), 0);
        ctx.delete_object(id).unwrap();
        assert!(ctx.resolve_owned_with_budget(&reference, &budget).is_err());
        assert_eq!(budget.live_bytes(), 0);
        let (copy, lease) = ctx.resolve_owned_with_budget(&object, &budget).unwrap();
        assert_eq!(copy, object);
        drop(copy);
        drop(lease);
        assert_eq!(budget.live_bytes(), 0);
    }

    use super::*;
    use crate::{
        document::cross_ref_table::ObjectInfo, io::stream_reader::StreamReader,
        parser::syntax::SyntaxParser,
    };

    #[test]
    fn revision_exhaustion_reserves_nested_rollback_capacity() {
        let ctx: PDFContext<std::io::Cursor<Vec<u8>>> =
            PDFContext::new_without_source(PdfDict::default(), Default::default());
        let id = ctx.try_insert_object(PdfObject::PdfNull).unwrap();
        ctx.state.revision.set(u64::MAX - 2);
        let result: PdfResult<()> = ctx.transaction(|| {
            ctx.update_object(id, PdfObject::PdfDict(PdfDict::default()))?;
            assert_eq!(ctx.revision(), u64::MAX - 1);
            assert!(matches!(
                ctx.update_object(id, PdfObject::PdfNull),
                Err(PdfError::ResourceLimit(_))
            ));
            assert!(matches!(
                ctx.transaction(|| Ok(())),
                Err(PdfError::ResourceLimit(_))
            ));
            Err(PdfError::DocumentError("rollback".into()))
        });
        assert!(result.is_err());
        assert_eq!(ctx.revision(), u64::MAX);
        assert_eq!(ctx.state.transaction_depth.get(), 0);
        assert_eq!(ctx.resolve_object_owned(id).unwrap(), PdfObject::PdfNull);
        assert!(matches!(
            ctx.try_insert_object(PdfObject::PdfNull),
            Err(PdfError::ResourceLimit(_))
        ));
    }

    #[test]
    fn parsed_trailer_is_owned_by_document_state_not_source_xref() {
        use crate::objects::pdf_name::PdfName;
        let reader = StreamReader::try_new(std::io::Cursor::new(Vec::<u8>::new())).unwrap();
        let parser = PdfParser::new(SyntaxParser::new(reader));
        let mut trailer = PdfDict::default();
        trailer
            .try_insert(
                "Custom".into(),
                PdfObject::PdfName(PdfName::new("preserved".into())),
            )
            .unwrap();
        let ctx = PDFContext::try_new(parser, CrossRefTable::new(HashMap::new(), trailer)).unwrap();
        assert!(ctx.cross_ref_table.trailer().get("Custom").is_none());
        assert_eq!(
            ctx.trailer()
                .get("Custom")
                .unwrap()
                .as_name()
                .unwrap()
                .bytes(),
            b"preserved"
        );
    }

    #[test]
    fn editing_object_limit_counts_deleted_entries_and_rolls_back() {
        use crate::parser::options::{Limits, ParseOptions};
        let reader = StreamReader::try_new(std::io::Cursor::new(Vec::<u8>::new())).unwrap();
        let syntax = SyntaxParser::with_options(
            reader,
            ParseOptions {
                limits: Limits {
                    max_objects: 2,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let ctx = PDFContext::try_new(
            PdfParser::new(syntax),
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        let first = ctx.try_insert_object(PdfObject::PdfNull).unwrap();
        let rolled_back: PdfResult<()> = ctx.transaction(|| {
            ctx.try_insert_object(PdfObject::PdfNull)?;
            ctx.try_insert_object(PdfObject::PdfNull)?;
            Ok(())
        });
        assert!(matches!(rolled_back, Err(PdfError::ResourceLimit(_))));
        let second = ctx.try_insert_object(PdfObject::PdfNull).unwrap();
        assert_eq!(second, ObjectId::new(2, 0));
        ctx.delete_object(first).unwrap();
        for _ in 0..2 {
            assert!(matches!(
                ctx.try_insert_object(PdfObject::PdfNull),
                Err(PdfError::ResourceLimit(_))
            ));
        }
        assert_eq!(ctx.state.store.borrow().entries().len(), 2);
    }

    #[test]
    fn context_rejects_exhausted_source_object_number() {
        let reader = StreamReader::try_new(std::io::Cursor::new(Vec::<u8>::new())).unwrap();
        let parser = PdfParser::new(SyntaxParser::new(reader));
        let entries = HashMap::from([(
            u32::MAX,
            ObjectInfo::new(u32::MAX, 0, 0, ObjectState::Normal),
        )]);
        assert!(matches!(
            PDFContext::try_new(parser, CrossRefTable::new(entries, PdfDict::default())),
            Err(PdfError::ResourceLimit(_))
        ));
    }

    #[test]
    fn editing_count_does_not_double_charge_source_updates() {
        use crate::parser::options::{Limits, ParseOptions};
        let reader = StreamReader::try_new(std::io::Cursor::new(Vec::<u8>::new())).unwrap();
        let syntax = SyntaxParser::with_options(
            reader,
            ParseOptions {
                limits: Limits {
                    max_objects: 2,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let id = ObjectId::new(1, 0);
        let entries = HashMap::from([(1, ObjectInfo::new(1, 0, 0, ObjectState::Normal))]);
        let ctx = PDFContext::try_new(
            PdfParser::new(syntax),
            CrossRefTable::new(entries, PdfDict::default()),
        )
        .unwrap();
        ctx.update_object(id, PdfObject::PdfNull).unwrap();
        assert_eq!(
            ctx.try_insert_object(PdfObject::PdfNull).unwrap(),
            ObjectId::new(2, 0)
        );
        assert!(matches!(
            ctx.try_insert_object(PdfObject::PdfNull),
            Err(PdfError::ResourceLimit(_))
        ));
    }

    #[test]
    fn compressed_object_cycle_returns_error_and_clears_loading_state() {
        let reader = StreamReader::try_new(std::io::Cursor::new(Vec::<u8>::new())).unwrap();
        let parser = PdfParser::new(SyntaxParser::new(reader));
        let mut entries = HashMap::new();
        entries.insert(1, ObjectInfo::new(1, 1, 0, ObjectState::Compressed));
        let ctx =
            PDFContext::try_new(parser, CrossRefTable::new(entries, PdfDict::default())).unwrap();
        for _ in 0..2 {
            assert!(matches!(
                ctx.resolve_object_owned(ObjectId::new(1, 0)),
                Err(PdfError::InvalidReference(_))
            ));
            assert!(ctx.loading_objects.borrow().is_empty());
        }
    }

    #[test]
    fn incorrect_xref_header_is_rejected_without_caching() {
        for header in ["2 0 obj 42 endobj", "1 1 obj 42 endobj"] {
            let reader = StreamReader::try_new(std::io::Cursor::new(header.as_bytes())).unwrap();
            let parser = PdfParser::new(SyntaxParser::new(reader));
            let ctx = PDFContext::try_new(
                parser,
                CrossRefTable::new(
                    HashMap::from([(1, ObjectInfo::new(1, 0, 0, ObjectState::Normal))]),
                    PdfDict::default(),
                ),
            )
            .unwrap();
            for _ in 0..2 {
                assert!(matches!(
                    ctx.resolve_object_owned(ObjectId::new(1, 0)),
                    Err(PdfError::InvalidReference(_))
                ));
                assert!(ctx.indirect_objects[&ObjectId::new(1, 0)].get().is_none());
                assert!(ctx.loading_objects.borrow().is_empty());
            }
        }
    }

    #[test]
    fn object_stream_cache_respects_current_xref_and_validates_before_commit() {
        let source = "10 0 obj << /Type /ObjStm /N 2 /First 8 /Length 13 >> stream\n1 0 2 3 11 22\nendstream\nendobj\n2 0 obj 99 endobj";
        for bad_index in [false, true] {
            let reader = StreamReader::try_new(std::io::Cursor::new(source.as_bytes())).unwrap();
            let entry2 = if bad_index {
                ObjectInfo::new(2, 10, 0, ObjectState::Compressed)
            } else {
                ObjectInfo::new(
                    2,
                    source.find("2 0 obj").unwrap() as u64,
                    0,
                    ObjectState::Normal,
                )
            };
            let ctx = PDFContext::try_new(
                PdfParser::new(SyntaxParser::new(reader)),
                CrossRefTable::new(
                    HashMap::from([
                        (10, ObjectInfo::new(10, 0, 0, ObjectState::Normal)),
                        (1, ObjectInfo::new(1, 10, 0, ObjectState::Compressed)),
                        (2, entry2),
                    ]),
                    PdfDict::default(),
                ),
            )
            .unwrap();
            if bad_index {
                for _ in 0..2 {
                    assert!(matches!(
                        ctx.resolve_object_owned(ObjectId::new(1, 0)),
                        Err(PdfError::InvalidReference(_))
                    ));
                    for id in [1, 2, 10] {
                        assert!(ctx.indirect_objects[&ObjectId::new(id, 0)].get().is_none());
                    }
                    assert_eq!(ctx.cached_stream_bytes.get(), 0);
                    assert!(ctx.loading_objects.borrow().is_empty());
                }
            } else {
                assert_eq!(
                    ctx.resolve_object_owned(ObjectId::new(1, 0))
                        .unwrap()
                        .as_number()
                        .unwrap()
                        .lexeme(),
                    "11"
                );
                assert_eq!(
                    ctx.resolve_object_owned(ObjectId::new(2, 0))
                        .unwrap()
                        .as_number()
                        .unwrap()
                        .lexeme(),
                    "99"
                );
            }
        }
    }

    #[test]
    fn object_stream_source_decode_ignores_edits_to_indirect_parameters() {
        use crate::objects::pdf_number::PdfNumber;
        use std::io::Write;
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(b"1 0 11").unwrap();
        let encoded = encoder.finish().unwrap();
        for (delete, array) in [(false, false), (true, false), (false, true), (true, true)] {
            let (filter, params) = if array {
                ("[7 0 R]", "[6 0 R]")
            } else {
                ("7 0 R", "6 0 R")
            };
            let mut source = format!("10 0 obj << /Type /ObjStm /N 1 /First 4 /Filter {filter} /DecodeParms {params} /Length {} >> stream\n", encoded.len()).into_bytes();
            source.extend_from_slice(&encoded);
            source.extend_from_slice(b"\nendstream\nendobj\n");
            let params_offset = source.len() as u64;
            source.extend_from_slice(b"9 0 obj << /Predictor 8 0 R >> endobj\n");
            let scalar_offset = source.len() as u64;
            source.extend_from_slice(b"8 0 obj 1 endobj\n");
            let filter_offset = source.len() as u64;
            source.extend_from_slice(b"7 0 obj /FlateDecode endobj\n");
            let chain_offset = source.len() as u64;
            source.extend_from_slice(b"6 0 obj 9 0 R endobj\n");
            let reader = StreamReader::try_new(std::io::Cursor::new(source)).unwrap();
            let ctx = PDFContext::try_new(
                PdfParser::new(SyntaxParser::new(reader)),
                CrossRefTable::new(
                    HashMap::from([
                        (10, ObjectInfo::new(10, 0, 0, ObjectState::Normal)),
                        (1, ObjectInfo::new(1, 10, 0, ObjectState::Compressed)),
                        (9, ObjectInfo::new(9, params_offset, 0, ObjectState::Normal)),
                        (8, ObjectInfo::new(8, scalar_offset, 0, ObjectState::Normal)),
                        (7, ObjectInfo::new(7, filter_offset, 0, ObjectState::Normal)),
                        (6, ObjectInfo::new(6, chain_offset, 0, ObjectState::Normal)),
                    ]),
                    PdfDict::default(),
                ),
            )
            .unwrap();
            let params_id = ObjectId::new(9, 0);
            if delete {
                ctx.delete_object(params_id).unwrap();
            } else {
                let mut params = PdfDict::default();
                params
                    .try_insert(
                        "Predictor".into(),
                        PdfObject::PdfNumber(PdfNumber::new(99.)),
                    )
                    .unwrap();
                ctx.update_object(params_id, PdfObject::PdfDict(params))
                    .unwrap();
            }
            assert_eq!(
                ctx.resolve_object_owned(ObjectId::new(1, 0))
                    .unwrap()
                    .as_number()
                    .unwrap()
                    .lexeme(),
                "11"
            );
            // Decoding the same stream normally still uses the latest view.
            let stream = ctx.resolve_object_owned(ObjectId::new(10, 0)).unwrap();
            assert!(stream.as_stream().unwrap().decode_data(&ctx).is_err());
            if delete {
                assert!(ctx.resolve_object_owned(params_id).is_err());
            } else {
                assert_eq!(
                    ctx.resolve_object_owned(params_id)
                        .unwrap()
                        .as_dict()
                        .unwrap()
                        .get("Predictor")
                        .unwrap()
                        .as_number()
                        .unwrap()
                        .lexeme(),
                    "99"
                );
            }
        }
    }

    #[test]
    fn source_resolution_scope_is_restored_after_error_and_unwind() {
        use std::panic::{catch_unwind, AssertUnwindSafe};
        let ctx: PDFContext<std::io::Cursor<Vec<u8>>> =
            PDFContext::new_without_source(PdfDict::default(), Default::default());
        let id = ctx.try_insert_object(PdfObject::PdfNull).unwrap();
        let result: PdfResult<()> = ctx.with_source_resolution(|| {
            assert!(ctx.resolve_object_owned(id).is_err());
            Err(PdfError::DocumentError("scope error".into()))
        });
        assert!(result.is_err());
        assert_eq!(ctx.source_resolution_depth.get(), 0);
        assert_eq!(ctx.resolve_object_owned(id).unwrap(), PdfObject::PdfNull);
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let _: PdfResult<()> = ctx.with_source_resolution(|| {
                panic!("scope unwind");
            });
        }))
        .is_err());
        assert_eq!(ctx.source_resolution_depth.get(), 0);
        assert_eq!(ctx.resolve_object_owned(id).unwrap(), PdfObject::PdfNull);
    }
}

impl<R: Seek + Read> PDFContext<R> {
    /// Initialize owned document state without constructing or parsing an input source.
    pub(crate) fn new_without_source(
        trailer: PdfDict,
        options: crate::parser::options::ParseOptions,
    ) -> Self {
        Self::new_without_source_with_version(
            trailer,
            options,
            crate::document::save_options::PdfVersion::V1_7,
        )
    }

    pub(crate) fn new_without_source_with_version(
        trailer: PdfDict,
        options: crate::parser::options::ParseOptions,
        version: crate::document::save_options::PdfVersion,
    ) -> Self {
        Self {
            parser: None,
            mode: options.mode,
            cross_ref_table: CrossRefTable::new_empty(),
            indirect_objects: HashMap::new(),
            state: crate::document::state::DocumentState::new(trailer, 1, version.as_str().into()),
            limits: options.limits,
            loading_objects: RefCell::new(Vec::new()),
            decoding_depth: Cell::new(0),
            color_depth: Cell::new(0),
            cached_stream_bytes: Cell::new(0),
            source_resolution_depth: Cell::new(0),
        }
    }

    /// Commit an edit group on success; restore edits and allocation state on
    /// error or unwinding. Nested groups act as savepoints.
    pub(crate) fn transaction<T>(&self, edit: impl FnOnce() -> PdfResult<T>) -> PdfResult<T> {
        self.state.transaction(edit)
    }

    pub(crate) fn transaction_with_budget<T>(
        &self,
        budget: &crate::budget::OperationBudget,
        edit: impl FnOnce() -> PdfResult<T>,
    ) -> PdfResult<T> {
        self.state
            .transaction_with_budget(budget, self.limits(), edit)
    }

    pub fn try_new(parser: PdfParser<R>, cross_ref_table: CrossRefTable) -> PdfResult<Self> {
        Self::try_new_with_version(parser, cross_ref_table, "1.7".into())
    }

    pub(crate) fn try_new_with_version(
        parser: PdfParser<R>,
        mut cross_ref_table: CrossRefTable,
        source_version: String,
    ) -> PdfResult<Self> {
        let limits = parser.limits();
        let mode = parser.mode();
        if cross_ref_table.objects().len() > limits.max_objects {
            return Err(PdfError::ResourceLimit("document object count".into()));
        }
        let mut indirect_objects = HashMap::new();
        for (objnum, objinfo) in cross_ref_table.objects() {
            if matches!(objinfo.state(), ObjectState::Free) {
                continue;
            }
            let generation = if matches!(objinfo.state(), ObjectState::Compressed) {
                0
            } else {
                objinfo.gennum()
            };
            indirect_objects.insert(ObjectId::new(*objnum, generation), OnceCell::new());
        }
        let next_obj_num = cross_ref_table
            .objects()
            .keys()
            .max()
            .copied()
            .map(|v| {
                v.checked_add(1)
                    .ok_or_else(|| PdfError::ResourceLimit("object number exhausted".into()))
            })
            .transpose()?
            .unwrap_or(1);
        let trailer = cross_ref_table.take_trailer();
        Ok(PDFContext {
            parser: Some(RefCell::new(parser)),
            mode,
            cross_ref_table,
            indirect_objects,
            state: crate::document::state::DocumentState::new(
                trailer,
                next_obj_num,
                source_version,
            ),
            limits,
            loading_objects: RefCell::new(Vec::new()),
            decoding_depth: Cell::new(0),
            color_depth: Cell::new(0),
            cached_stream_bytes: Cell::new(0),
            source_resolution_depth: Cell::new(0),
        })
    }

    pub fn parse_indirect_objects(&mut self) -> PdfResult<()> {
        for object_id in self.indirect_objects.keys() {
            let object_ref = PdfObject::PdfReference(PdfReference::from_id(*object_id));
            let _ = self.resolve(&object_ref)?;
        }
        Ok(())
    }

    pub fn get_root(&self) -> PdfResult<PdfDict> {
        let trailer = self.state.trailer.borrow();
        let root = trailer
            .get("Root")
            .ok_or(PdfError::DocumentError("Root not found".to_string()))?;

        let root = self.resolve(root)?;
        let root = root
            .into_dict()
            .ok_or(PdfError::DocumentError("Root is not a Dict".to_string()))?;
        Ok(root)
    }

    pub fn get_root_owned(&self) -> PdfResult<PdfDict> {
        let trailer = self.state.trailer.borrow();
        let root = trailer
            .get("Root")
            .ok_or(PdfError::DocumentError("Root not found".to_string()))?;

        let root = self.resolve_owned(root)?;
        let root = root
            .into_dict()
            .ok_or(PdfError::DocumentError("Root is not a Dict".to_string()))?;
        Ok(root)
    }

    pub fn trailer(&self) -> std::cell::Ref<'_, PdfDict> {
        self.state.trailer.borrow()
    }

    pub(crate) fn replace_trailer(&self, trailer: PdfDict) -> PdfResult<()> {
        let revision = self.next_revision()?;
        *self
            .state
            .trailer
            .try_borrow_mut()
            .map_err(|_| PdfError::DocumentError("Trailer is borrowed".into()))? = trailer;
        self.state.did_mutate(revision);
        Ok(())
    }

    pub fn revision(&self) -> u64 {
        self.state.revision.get()
    }

    pub fn commit_revision(&self) -> u64 {
        self.state.commit_revision.get()
    }

    pub fn identity(&self) -> crate::document::identity::DocumentIdentity {
        self.state.identity.clone()
    }

    pub fn source_version(&self) -> &str {
        &self.state.source_version
    }

    fn next_revision(&self) -> PdfResult<u64> {
        self.state.next_revision()
    }

    pub fn limits(&self) -> crate::parser::options::Limits {
        self.limits
    }

    pub fn options(&self) -> crate::parser::options::ParseOptions {
        crate::parser::options::ParseOptions {
            mode: self.mode,
            limits: self.limits,
        }
    }

    pub(crate) fn with_decode_depth<T>(
        &self,
        decode: impl FnOnce() -> PdfResult<T>,
    ) -> PdfResult<T> {
        if self.decoding_depth.get() >= self.limits.max_depth {
            return Err(PdfError::ResourceLimit(
                "nested stream decoding depth".into(),
            ));
        }
        self.decoding_depth.set(self.decoding_depth.get() + 1);
        let _guard = DecodeGuard(&self.decoding_depth);
        decode()
    }

    pub(crate) fn with_color_depth<T>(
        &self,
        construct: impl FnOnce() -> PdfResult<T>,
    ) -> PdfResult<T> {
        if self.color_depth.get() >= self.limits.max_depth {
            return Err(PdfError::ResourceLimit("color space nesting".into()));
        }
        self.color_depth.set(self.color_depth.get() + 1);
        let _guard = DecodeGuard(&self.color_depth);
        construct()
    }

    pub fn object_ids(&self) -> Vec<ObjectId> {
        let mut ids: Vec<_> = self.indirect_objects.keys().copied().collect();
        ids.sort_by_key(|id| (id.obj_num(), id.gen_num()));
        ids
    }

    pub fn serializable_object_ids(&self) -> Vec<ObjectId> {
        let mut ids = self.object_ids();
        for object_id in self.state.store.borrow().entries().keys() {
            if !ids.contains(object_id) {
                ids.push(*object_id);
            }
        }
        ids.sort_by_key(|id| (id.obj_num(), id.gen_num()));
        ids
    }

    pub fn materialize_all_objects(&self) -> PdfResult<()> {
        for object_id in self.object_ids() {
            self.load_indirect_object(object_id)?;
        }
        Ok(())
    }

    pub fn resolve(&self, obj: &PdfObject) -> PdfResult<PdfObject> {
        match obj {
            PdfObject::PdfReference(reference) => self.resolve_object_id(reference.id()),
            _ => Ok(obj.clone()),
        }
    }

    pub fn resolve_owned(&self, obj: &PdfObject) -> PdfResult<PdfObject> {
        match obj {
            PdfObject::PdfReference(reference) => self.resolve_object_owned(reference.id()),
            _ => Ok(obj.clone()),
        }
    }

    /// Budget the owned copy without first making an untracked Clone.
    /// Loading/caching an uncached source object remains a separate allocation.
    pub(crate) fn resolve_owned_with_budget(
        &self,
        obj: &PdfObject,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(PdfObject, crate::budget::MemoryLease)> {
        let Some(reference) = obj.as_reference() else {
            return crate::objects::clone_object_with_budget(obj, budget, self.limits, 0);
        };
        let id = reference.id();
        if self.source_resolution_depth.get() == 0 {
            let store = self.state.store.borrow();
            match store.entry(id) {
                Some(StoreEntry::Upsert(object)) => {
                    return crate::objects::clone_object_with_budget(object, budget, self.limits, 0)
                }
                Some(StoreEntry::Deleted) => {
                    return Err(PdfError::ObjectError(format!(
                        "object has been deleted: {:?}",
                        id
                    )))
                }
                None => {}
            }
        }
        crate::objects::clone_object_with_budget(
            self.load_indirect_object(id)?,
            budget,
            self.limits,
            0,
        )
    }

    pub fn resolve_object_id(&self, object_id: ObjectId) -> PdfResult<PdfObject> {
        self.resolve_object_owned(object_id)
    }

    /// Read the unmodified source object, ignoring edits in the document store.
    pub fn resolve_original_object(&self, object_id: ObjectId) -> PdfResult<&PdfObject> {
        self.load_indirect_object(object_id)
    }

    pub fn resolve_object_owned(&self, object_id: ObjectId) -> PdfResult<PdfObject> {
        if self.source_resolution_depth.get() != 0 {
            return Ok(self.load_indirect_object(object_id)?.clone());
        }
        match self.state.store.borrow().entry(object_id).cloned() {
            Some(StoreEntry::Upsert(object)) => Ok(object),
            Some(StoreEntry::Deleted) => Err(PdfError::ObjectError(format!(
                "object has been deleted: {:?}",
                object_id
            ))),
            None => Ok(self.load_indirect_object(object_id)?.clone()),
        }
    }

    pub fn object_for_write(&self, object_id: ObjectId) -> PdfResult<Option<PdfObject>> {
        if let Some(entry) = self.state.store.borrow().entry(object_id).cloned() {
            return match entry {
                StoreEntry::Upsert(object) => Ok(Some(object)),
                StoreEntry::Deleted => Ok(None),
            };
        }

        if !self.indirect_objects.contains_key(&object_id) {
            return Err(PdfError::ObjectError(format!(
                "object does not exist in document store: {:?}",
                object_id
            )));
        }
        Ok(Some(self.resolve_object_id(object_id)?.clone()))
    }

    pub fn insert_object(&self, object: PdfObject) -> ObjectId {
        self.try_insert_object(object)
            .expect("object allocation failed; use try_insert_object")
    }

    pub fn try_insert_object(&self, object: PdfObject) -> PdfResult<ObjectId> {
        let revision = self.next_revision()?;
        let mut store = self.state.store.borrow_mut();
        // Deleted entries still retain state and are counted until explicit cleanup.
        let added = store
            .entries()
            .keys()
            .filter(|id| !self.indirect_objects.contains_key(id))
            .count();
        let count = self
            .indirect_objects
            .len()
            .checked_add(added)
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| PdfError::ResourceLimit("document object count overflow".into()))?;
        if count > self.limits.max_objects {
            return Err(PdfError::ResourceLimit("document object count".into()));
        }
        let id = store.try_insert_object(object)?;
        self.state.did_mutate(revision);
        Ok(id)
    }

    pub fn update_object(&self, object_id: ObjectId, object: PdfObject) -> PdfResult<()> {
        let revision = self.next_revision()?;
        if !self.indirect_objects.contains_key(&object_id)
            && self.state.store.borrow().entry(object_id).is_none()
        {
            return Err(PdfError::ObjectError(format!(
                "cannot update missing object: {:?}",
                object_id
            )));
        }
        self.state
            .store
            .borrow_mut()
            .update_object(object_id, object)?;
        self.state.did_mutate(revision);
        Ok(())
    }

    pub fn delete_object(&self, object_id: ObjectId) -> PdfResult<()> {
        let revision = self.next_revision()?;
        if !self.indirect_objects.contains_key(&object_id)
            && self.state.store.borrow().entry(object_id).is_none()
        {
            return Err(PdfError::ObjectError(format!(
                "cannot delete missing object: {:?}",
                object_id
            )));
        }
        self.state.store.borrow_mut().delete_object(object_id)?;
        self.state.did_mutate(revision);
        Ok(())
    }

    fn load_indirect_object<'a>(&'a self, object_id: ObjectId) -> PdfResult<&'a PdfObject> {
        let Some(cell) = self.indirect_objects.get(&object_id) else {
            return Err(PdfError::ObjectError(format!(
                "indirect object does not exist: {:?}",
                object_id
            )));
        };

        if let Some(obj) = cell.get() {
            return Ok(obj);
        }

        {
            let mut loading = self.loading_objects.borrow_mut();
            if loading.contains(&object_id) {
                return Err(PdfError::InvalidReference(format!(
                    "cycle loading object {object_id:?}"
                )));
            }
            if loading.len() >= self.limits.max_depth {
                return Err(PdfError::ResourceLimit(format!(
                    "object loading depth at {object_id:?}"
                )));
            }
            loading.push(object_id);
        }
        let _guard = LoadingGuard(&self.loading_objects);

        let objinfo = self
            .cross_ref_table
            .lookup_id(object_id)
            .ok_or(PdfError::ObjectError(format!(
                "xref entry does not exist: {:?}",
                object_id
            )))?;

        let object = match objinfo.state() {
            ObjectState::Normal => self.parse_indirect_object(objinfo.offset(), object_id),
            ObjectState::Compressed => {
                let stream_number = u32::try_from(objinfo.offset()).map_err(|_| {
                    PdfError::InvalidReference("object stream number exceeds u32".into())
                })?;
                let stream_ref = ObjectId::new(stream_number, 0);
                self.load_object_stream(stream_ref)?;
                self.indirect_objects
                    .get(&object_id)
                    .and_then(OnceCell::get)
                    .cloned()
                    .ok_or(PdfError::ObjectError(format!(
                        "compressed object was not populated from object stream: {:?}",
                        object_id
                    )))
            }
            ObjectState::Free => Err(PdfError::ObjectError(format!(
                "attempted to resolve free object: {:?}",
                object_id
            ))),
        }?;

        let _ = cell.set(object);
        cell.get().ok_or(PdfError::ObjectError(format!(
            "failed to cache resolved object: {:?}",
            object_id
        )))
    }

    fn parse_indirect_object(&self, offset: u64, expected: ObjectId) -> PdfResult<PdfObject> {
        let obj = self
            .parser
            .as_ref()
            .ok_or_else(|| PdfError::InvalidReference("document has no source objects".into()))?
            .borrow_mut()
            .parse_indirect_obj_checked(offset, expected)?;
        let bytes = obj
            .as_stream()
            .map(|stream| stream.raw_data().len())
            .unwrap_or(0);
        crate::filter::check_output_size(
            self.cached_stream_bytes.get(),
            bytes,
            self.limits.max_cached_stream_bytes,
            "cached raw streams",
        )?;
        self.populate_object_stream_entries(&obj, expected)?;
        self.cached_stream_bytes
            .set(self.cached_stream_bytes.get() + bytes);
        Ok(obj)
    }

    fn populate_object_stream_entries(&self, obj: &PdfObject, source: ObjectId) -> PdfResult<()> {
        let Some(stream) = obj.as_stream() else {
            return Ok(());
        };
        let Some(st) = stream.dict().get("Type") else {
            return Ok(());
        };
        let Some(name) = st.as_name() else {
            return Ok(());
        };
        if name.name() != "ObjStm" {
            return Ok(());
        }

        // Expanded source objects must never depend on tentative or committed
        // edits to decoding dependencies, even on the first lazy load.
        let data = self.with_source_resolution(|| stream.decode_data(self))?;
        let options = crate::parser::options::ParseOptions {
            mode: self.mode,
            limits: self.limits,
        };
        let mut object_streams = ObjectStreams::try_new_with_options(stream.dict(), data, options)?;
        let objects = object_streams.parse_objects()?;
        let mut pending = Vec::new();
        for (index, (objnum, obj)) in objects.into_iter().enumerate() {
            let object_id = ObjectId::new(objnum, 0);
            let Some(info) = self.cross_ref_table.lookup_id(object_id) else {
                continue;
            };
            // Historical entries must not populate current normal/free objects or
            // compressed objects belonging to a different object stream.
            if !matches!(info.state(), ObjectState::Compressed)
                || info.offset() != u64::from(source.obj_num())
                || source.gen_num() != 0
            {
                continue;
            }
            if info.gennum() as usize != index {
                return Err(PdfError::InvalidReference(format!(
                    "compressed object {object_id:?} index {} does not match {index}",
                    info.gennum()
                )));
            }
            pending.push((object_id, obj));
        }
        // Validate all selected entries before committing any cache cells.
        for (object_id, obj) in pending {
            if let Some(cell) = self.indirect_objects.get(&object_id) {
                let _ = cell.set(obj);
            }
        }
        Ok(())
    }

    fn load_object_stream(&self, stream_ref: ObjectId) -> PdfResult<()> {
        let object_stream = self.load_indirect_object(stream_ref)?;
        let stream = object_stream
            .as_stream()
            .ok_or(PdfError::ObjectError(format!(
                "object stream reference does not point to a stream: {:?}",
                stream_ref
            )))?;
        self.populate_object_stream_entries(&PdfObject::PdfStream(stream.clone()), stream_ref)
    }

    fn with_source_resolution<T>(&self, decode: impl FnOnce() -> PdfResult<T>) -> PdfResult<T> {
        if self.source_resolution_depth.get() >= self.limits.max_depth {
            return Err(PdfError::ResourceLimit(
                "source decoding dependency depth".into(),
            ));
        }
        self.source_resolution_depth
            .set(self.source_resolution_depth.get() + 1);
        let _guard = DecodeGuard(&self.source_resolution_depth);
        decode()
    }
}
