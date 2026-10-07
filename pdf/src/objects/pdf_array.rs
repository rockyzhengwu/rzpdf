use crate::error::{PdfError, PdfResult};
use crate::objects::PdfObject;

#[derive(Debug, PartialEq, Clone, Default)]
pub struct PdfArray {
    items: Vec<PdfObject>,
}

impl PdfArray {
    pub(crate) fn from_items(items: Vec<PdfObject>) -> Self {
        Self { items }
    }

    pub fn add_obj(&mut self, obj: PdfObject) {
        self.items.push(obj)
    }

    /// Fallible append for parsing and transactional editing.
    pub fn try_add_obj(&mut self, obj: PdfObject) -> PdfResult<()> {
        self.items
            .try_reserve(1)
            .map_err(|_| PdfError::ResourceLimit("array append allocation".into()))?;
        self.items.push(obj);
        Ok(())
    }

    pub fn insert(&mut self, index: usize, obj: PdfObject) -> PdfResult<()> {
        if index > self.items.len() {
            return Err(PdfError::ObjectError(
                "array insertion index out of bounds".into(),
            ));
        }
        self.items
            .try_reserve(1)
            .map_err(|_| PdfError::ResourceLimit("array insertion allocation".into()))?;
        self.items.insert(index, obj);
        Ok(())
    }

    pub fn get(&self, index: usize) -> Option<&PdfObject> {
        self.items.get(index)
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn remove(&mut self, index: usize) -> Option<PdfObject> {
        if index < self.items.len() {
            return Some(self.items.remove(index));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insertion_checks_bounds_and_preserves_array_on_error() {
        let mut array = PdfArray::default();
        assert!(array.insert(1, PdfObject::PdfNull).is_err());
        assert_eq!(array.len(), 0);
        array.insert(0, PdfObject::PdfNull).unwrap();
        array.insert(1, PdfObject::PdfNull).unwrap();
        array.insert(0, PdfObject::PdfNull).unwrap();
        let before = array.clone();
        assert!(array.insert(usize::MAX, PdfObject::PdfNull).is_err());
        assert_eq!(array, before);
        array.try_add_obj(PdfObject::PdfNull).unwrap();
        assert_eq!(array.len(), before.len() + 1);
    }
}

impl<'a> IntoIterator for &'a PdfArray {
    type Item = &'a PdfObject;
    type IntoIter = PdfArrayIterator<'a>;
    fn into_iter(self) -> Self::IntoIter {
        PdfArrayIterator {
            current_index: 0,
            array: self,
        }
    }
}

pub struct PdfArrayIterator<'a> {
    current_index: usize,
    array: &'a PdfArray,
}

impl<'a> Iterator for PdfArrayIterator<'a> {
    type Item = &'a PdfObject;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index < self.array.items.len() {
            let item = &self.array.items[self.current_index];
            self.current_index += 1;
            Some(item)
        } else {
            None
        }
    }
}
