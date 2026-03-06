use crate::objects::PdfObject;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone, Default)]
pub struct PdfDict {
    map: HashMap<String, PdfObject>,
}

impl PdfDict {
    pub fn new(map: HashMap<String, PdfObject>) -> Self {
        Self { map }
    }
    pub fn insert(&mut self, key: String, value: PdfObject) {
        self.map.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<&PdfObject> {
        self.map.get(key)
    }
}
impl<'a> IntoIterator for &'a PdfDict {
    type Item = (&'a String, &'a PdfObject);
    type IntoIter = std::collections::hash_map::Iter<'a, String, PdfObject>;

    fn into_iter(self) -> Self::IntoIter {
        self.map.iter()
    }
}
