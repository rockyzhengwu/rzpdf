use crate::objects::PdfObject;

#[derive(Debug, PartialEq, Clone, Default)]
pub struct PdfArray {
    items: Vec<PdfObject>,
}

impl PdfArray {
    pub fn add_obj(&mut self, obj: PdfObject) {
        self.items.push(obj)
    }

    pub fn insert(&mut self, index: usize, obj: PdfObject) {
        self.items.insert(index, obj);
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
