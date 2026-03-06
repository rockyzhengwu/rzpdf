#[derive(Debug)]
pub struct ContentSyntax {
    buffer: Vec<u8>,
    pos: u64,
}

impl ContentSyntax {
    pub fn new(buffer: Vec<u8>) -> Self {
        Self { buffer, pos: 0 }
    }

    pub fn is_end(&self) -> bool {
        self.pos > self.buffer.len() as u64
    }
}
