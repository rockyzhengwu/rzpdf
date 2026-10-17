//! Shared bounded name/number tree traversal and balanced serialization.
use super::PdfDocument;
use crate::{
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, object_id::ObjectId, pdf_array::PdfArray, pdf_dict::PdfDict,
        pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_string::PdfString,
    },
};
use std::{
    collections::HashSet,
    io::{Read, Seek},
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TreeKey {
    Name(Vec<u8>),
    Number(u32),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        io::stream_reader::StreamReader,
        parser::{
            options::{Limits, ParseOptions},
            syntax::SyntaxParser,
        },
    };
    use std::io::Cursor;

    fn parse(text: &str) -> PdfObject {
        SyntaxParser::new(StreamReader::try_new(Cursor::new(text.as_bytes().to_vec())).unwrap())
            .get_object()
            .unwrap()
    }

    #[test]
    fn nested_name_tree_checks_limits_order_and_shared_nodes() {
        let doc = PdfDocument::new().unwrap();
        let first = doc
            .try_insert_object(parse("<< /Limits [(a) (b)] /Names [(a) 1 (b) 2] >>"))
            .unwrap();
        let second = doc
            .try_insert_object(parse("<< /Limits [(c) (d)] /Names [(c) 3 (d) 4] >>"))
            .unwrap();
        let root = |a: ObjectId, b: ObjectId| {
            let mut kids = PdfArray::default();
            for id in [a, b] {
                kids.try_add_obj(PdfObject::PdfReference(PdfReference::from_id(id)))
                    .unwrap();
            }
            let mut dict = PdfDict::default();
            dict.try_insert("Kids".into(), PdfObject::PdfArray(kids))
                .unwrap();
            PdfObject::PdfDict(dict)
        };
        assert_eq!(
            doc.read_tree(&root(first, second), TreeKind::Name)
                .unwrap()
                .len(),
            4
        );
        assert!(doc.read_tree(&root(second, first), TreeKind::Name).is_err());
        assert!(doc.read_tree(&root(first, first), TreeKind::Name).is_err());
        doc.update_object(
            second,
            parse("<< /Limits [(c) (z)] /Names [(c) 3 (d) 4] >>"),
        )
        .unwrap();
        assert!(doc.read_tree(&root(first, second), TreeKind::Name).is_err());
        doc.update_object(second, parse("<< /Names [(c) 3 (d) 4] >>"))
            .unwrap();
        assert!(doc.read_tree(&root(first, second), TreeKind::Name).is_err());
    }

    #[test]
    fn number_tree_balances_more_than_two_levels_and_rejects_malformed_keys() {
        let doc = PdfDocument::new().unwrap();
        let entries: Vec<_> = (0..1100)
            .map(|n| (TreeKey::Number(n), PdfObject::PdfNull))
            .collect();
        let root = doc
            .edit_transaction(|doc| doc.build_tree(TreeKind::Number, &entries, PdfDict::default()))
            .unwrap();
        assert_eq!(
            doc.read_tree(&PdfObject::PdfDict(root), TreeKind::Number)
                .unwrap(),
            entries
        );
        for text in [
            "<< /Nums [1 null 0 null] >>",
            "<< /Nums [0 null 0 null] >>",
            "<< /Nums [-1 null] >>",
            "<< /Nums [0.5 null] >>",
            "<< /Nums [0] >>",
            "<< /Nums [] /Kids [] >>",
            "<< /Names [] >>",
        ] {
            assert!(
                doc.read_tree(&parse(text), TreeKind::Number).is_err(),
                "{text}"
            );
        }
    }

    #[test]
    fn cycles_and_depth_limits_fail_without_partial_writes() {
        let doc = PdfDocument::new().unwrap();
        let id = doc.try_insert_object(PdfObject::PdfNull).unwrap();
        let mut kids = PdfArray::default();
        kids.try_add_obj(PdfObject::PdfReference(PdfReference::from_id(id)))
            .unwrap();
        let mut dict = PdfDict::default();
        dict.try_insert("Kids".into(), PdfObject::PdfArray(kids))
            .unwrap();
        doc.update_object(id, PdfObject::PdfDict(dict)).unwrap();
        assert!(
            doc.read_tree(
                &PdfObject::PdfReference(PdfReference::from_id(id)),
                TreeKind::Name
            )
            .is_err()
        );
        let doc = PdfDocument::new_with_options(ParseOptions {
            limits: Limits {
                max_depth: 1,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        let entries: Vec<_> = (0..40)
            .map(|n| (TreeKey::Number(n), PdfObject::PdfNull))
            .collect();
        let revision = doc.commit_revision();
        assert!(
            doc.edit_transaction(|doc| doc.build_tree(
                TreeKind::Number,
                &entries,
                PdfDict::default()
            ))
            .is_err()
        );
        assert_eq!(doc.commit_revision(), revision);
        assert!(
            doc.edit_transaction(|doc| doc.build_tree(
                TreeKind::Number,
                &entries[..2],
                PdfDict::default()
            ))
            .is_ok()
        );
    }
}

#[derive(Clone, Copy)]
pub(crate) enum TreeKind {
    Name,
    Number,
}

impl TreeKind {
    fn entries(self) -> &'static str {
        match self {
            Self::Name => "Names",
            Self::Number => "Nums",
        }
    }
    fn parse(self, value: &PdfObject) -> PdfResult<TreeKey> {
        match self {
            Self::Name => value
                .as_string()
                .map(|s| TreeKey::Name(s.bytes().to_vec()))
                .ok_or_else(|| PdfError::DocumentError("name tree key must be a string".into())),
            Self::Number => Ok(TreeKey::Number(
                value
                    .as_number()
                    .ok_or_else(|| {
                        PdfError::DocumentError("number tree key must be an integer".into())
                    })?
                    .as_u32_checked()?,
            )),
        }
    }
}

impl TreeKey {
    fn object(&self) -> PdfObject {
        match self {
            Self::Name(bytes) => PdfObject::PdfString(PdfString::new(bytes.clone(), false)),
            Self::Number(n) => PdfObject::PdfNumber(
                PdfNumber::new_from_lexeme(n.to_string().as_bytes()).expect("u32 is a PDF integer"),
            ),
        }
    }
}

enum Visit {
    Enter(PdfObject, usize, bool),
    Exit(PdfDict, usize, bool),
}

impl<R: Read + Seek> PdfDocument<R> {
    pub(crate) fn read_tree(
        &self,
        root: &PdfObject,
        kind: TreeKind,
    ) -> PdfResult<Vec<(TreeKey, PdfObject)>> {
        let limits = self.ctx().limits();
        let mut stack = vec![Visit::Enter(root.clone(), 0, true)];
        let mut seen = HashSet::<ObjectId>::new();
        let mut nodes = 0usize;
        let mut result: Vec<(TreeKey, PdfObject)> = Vec::new();
        while let Some(visit) = stack.pop() {
            match visit {
                Visit::Enter(value, depth, is_root) => {
                    nodes = nodes
                        .checked_add(1)
                        .ok_or_else(|| PdfError::ResourceLimit("tree nodes".into()))?;
                    if depth >= limits.max_depth || nodes > limits.max_objects {
                        return Err(PdfError::ResourceLimit("tree depth or node count".into()));
                    }
                    if let Some(id) = value.as_reference().map(PdfReference::id) {
                        if !seen.insert(id) {
                            return Err(PdfError::DocumentError(
                                "tree cycle or shared child".into(),
                            ));
                        }
                    }
                    let dict = self.ctx().resolve(&value)?.into_dict().ok_or_else(|| {
                        PdfError::DocumentError("tree node must be a dictionary".into())
                    })?;
                    let wrong_entries = match kind {
                        TreeKind::Name => "Nums",
                        TreeKind::Number => "Names",
                    };
                    if dict.get(wrong_entries).is_some() {
                        return Err(PdfError::DocumentError(
                            "mixed name and number tree entries".into(),
                        ));
                    }
                    if !is_root
                        && (&dict).into_iter().any(|(k, _)| {
                            !matches!(k.as_str(), "Names" | "Nums" | "Kids" | "Limits")
                        })
                    {
                        return Err(PdfError::UnsupportedFeature(
                            "tree child with unknown keys cannot be safely rebalanced".into(),
                        ));
                    }
                    let start = result.len();
                    if let Some(kids) = dict.get("Kids") {
                        if dict.get(kind.entries()).is_some() {
                            return Err(PdfError::DocumentError(
                                "tree node has both Kids and entries".into(),
                            ));
                        }
                        let kids = self.ctx().resolve(kids)?.into_array().ok_or_else(|| {
                            PdfError::DocumentError("tree Kids must be an array".into())
                        })?;
                        if kids.len() == 0
                            || kids.len() > limits.max_objects.saturating_sub(nodes)
                            || stack.len().saturating_add(kids.len()) > limits.max_objects
                        {
                            return Err(PdfError::ResourceLimit("tree child count".into()));
                        }
                        stack.push(Visit::Exit(dict, start, is_root));
                        for i in (0..kids.len()).rev() {
                            let child = kids.get(i).unwrap();
                            if child.as_reference().is_none() {
                                return Err(PdfError::DocumentError(
                                    "tree child must be indirect".into(),
                                ));
                            }
                            stack.push(Visit::Enter(child.clone(), depth + 1, false));
                        }
                    } else {
                        if let Some(entries) = dict.get(kind.entries()) {
                            let entries =
                                self.ctx().resolve(entries)?.into_array().ok_or_else(|| {
                                    PdfError::DocumentError("tree entries must be an array".into())
                                })?;
                            if entries.len() % 2 != 0 {
                                return Err(PdfError::DocumentError("odd tree entries".into()));
                            }
                            if entries.len() / 2
                                > (limits.max_objects / 2).saturating_sub(result.len())
                            {
                                return Err(PdfError::ResourceLimit("tree entry count".into()));
                            }
                            for i in (0..entries.len()).step_by(2) {
                                let key =
                                    kind.parse(&self.ctx().resolve(entries.get(i).unwrap())?)?;
                                if result.last().is_some_and(|(old, _)| old >= &key) {
                                    return Err(PdfError::DocumentError(
                                        "tree keys must be unique and ordered".into(),
                                    ));
                                }
                                result.push((key, entries.get(i + 1).unwrap().clone()));
                            }
                        } else if !is_root {
                            return Err(PdfError::DocumentError("empty tree child".into()));
                        }
                        self.check_tree_limits(&dict, &result[start..], kind, is_root)?;
                    }
                }
                Visit::Exit(dict, start, is_root) => {
                    self.check_tree_limits(&dict, &result[start..], kind, is_root)?
                }
            }
        }
        Ok(result)
    }

    fn check_tree_limits(
        &self,
        dict: &PdfDict,
        entries: &[(TreeKey, PdfObject)],
        kind: TreeKind,
        root: bool,
    ) -> PdfResult<()> {
        let Some(value) = dict.get("Limits") else {
            return if root {
                Ok(())
            } else {
                Err(PdfError::DocumentError("tree child Limits missing".into()))
            };
        };
        let array = self
            .ctx()
            .resolve(value)?
            .into_array()
            .ok_or_else(|| PdfError::DocumentError("tree Limits must be an array".into()))?;
        if array.len() != 2 || entries.is_empty() {
            return Err(PdfError::DocumentError("invalid tree Limits".into()));
        }
        let first = kind.parse(&self.ctx().resolve(array.get(0).unwrap())?)?;
        let last = kind.parse(&self.ctx().resolve(array.get(1).unwrap())?)?;
        if entries.first().map(|e| &e.0) != Some(&first)
            || entries.last().map(|e| &e.0) != Some(&last)
        {
            return Err(PdfError::DocumentError(
                "tree Limits do not match entries".into(),
            ));
        }
        Ok(())
    }

    /// Call inside an edit transaction: allocation failure rolls back all nodes.
    pub(crate) fn build_tree(
        &self,
        kind: TreeKind,
        entries: &[(TreeKey, PdfObject)],
        mut root: PdfDict,
    ) -> PdfResult<PdfDict> {
        if entries.len() > self.ctx().limits().max_objects / 2 {
            return Err(PdfError::ResourceLimit("tree entries".into()));
        }
        for (i, (key, _)) in entries.iter().enumerate() {
            kind.parse(&key.object())?;
            if i > 0 && entries[i - 1].0 >= *key {
                return Err(PdfError::DocumentError("unordered tree entries".into()));
            }
        }
        root.remove("Names");
        root.remove("Nums");
        root.remove("Kids");
        root.remove("Limits");
        const FANOUT: usize = 32;
        if entries.len() <= FANOUT {
            root.try_insert(
                kind.entries().into(),
                PdfObject::PdfArray(Self::tree_pairs(entries)?),
            )?;
            return Ok(root);
        }
        let mut level = Vec::new();
        for chunk in entries.chunks(FANOUT) {
            let mut leaf = PdfDict::default();
            leaf.try_insert(
                kind.entries().into(),
                PdfObject::PdfArray(Self::tree_pairs(chunk)?),
            )?;
            Self::tree_bounds(&mut leaf, &chunk[0].0, &chunk[chunk.len() - 1].0)?;
            let id = self.try_insert_object(PdfObject::PdfDict(leaf))?;
            level.push((chunk[0].0.clone(), chunk[chunk.len() - 1].0.clone(), id));
        }
        let mut depth = 1;
        while level.len() > FANOUT {
            depth += 1;
            if depth >= self.ctx().limits().max_depth {
                return Err(PdfError::ResourceLimit("tree depth".into()));
            }
            let mut next = Vec::new();
            for chunk in level.chunks(FANOUT) {
                let mut branch = PdfDict::default();
                branch.try_insert("Kids".into(), PdfObject::PdfArray(Self::tree_kids(chunk)?))?;
                Self::tree_bounds(&mut branch, &chunk[0].0, &chunk[chunk.len() - 1].1)?;
                let id = self.try_insert_object(PdfObject::PdfDict(branch))?;
                next.push((chunk[0].0.clone(), chunk[chunk.len() - 1].1.clone(), id));
            }
            level = next;
        }
        if depth >= self.ctx().limits().max_depth {
            return Err(PdfError::ResourceLimit("tree depth".into()));
        }
        root.try_insert("Kids".into(), PdfObject::PdfArray(Self::tree_kids(&level)?))?;
        Ok(root)
    }

    fn tree_pairs(entries: &[(TreeKey, PdfObject)]) -> PdfResult<PdfArray> {
        let mut array = PdfArray::default();
        for (key, value) in entries {
            array.try_add_obj(key.object())?;
            array.try_add_obj(value.clone())?;
        }
        Ok(array)
    }
    fn tree_bounds(dict: &mut PdfDict, first: &TreeKey, last: &TreeKey) -> PdfResult<()> {
        let mut array = PdfArray::default();
        array.try_add_obj(first.object())?;
        array.try_add_obj(last.object())?;
        dict.try_insert("Limits".into(), PdfObject::PdfArray(array))?;
        Ok(())
    }
    fn tree_kids(entries: &[(TreeKey, TreeKey, ObjectId)]) -> PdfResult<PdfArray> {
        let mut array = PdfArray::default();
        for (_, _, id) in entries {
            array.try_add_obj(PdfObject::PdfReference(PdfReference::from_id(*id)))?;
        }
        Ok(array)
    }
}
