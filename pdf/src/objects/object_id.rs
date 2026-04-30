#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId {
    obj_num: u32,
    gen_num: u32,
}

impl ObjectId {
    pub fn new(obj_num: u32, gen_num: u32) -> Self {
        Self { obj_num, gen_num }
    }

    pub fn obj_num(&self) -> u32 {
        self.obj_num
    }

    pub fn gen_num(&self) -> u32 {
        self.gen_num
    }
}
