#[derive(Debug, PartialEq, Clone)]
pub struct PdfNumber {
    value: f32,
}
impl PdfNumber {
    pub fn new(value: f32) -> Self {
        Self { value }
    }

    pub fn get_u64(&self) -> u64 {
        self.value.round() as u64
    }

    pub fn get_u32(&self) -> u32 {
        self.value.round() as u32
    }
    pub fn get_u16(&self) -> u16 {
        self.value.round() as u16
    }

    pub fn get_i8(&self) -> i8 {
        self.value as i8
    }

    pub fn get_i32(&self) -> i32 {
        self.value as i32
    }

    pub fn get_u8(&self) -> u8 {
        self.value as u8
    }

    pub fn value(&self) -> f32 {
        self.value
    }
}
