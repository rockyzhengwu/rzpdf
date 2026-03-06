use crate::error::{PdfError, PdfResult};
use crate::objects::{pdf_array::PdfArray, pdf_dict::PdfDict};
use crate::parser::crypto::{
    aes_cbc_decrypt, aes_cbc_encrypt, aes128_decrypt, aes256_decrypt, rc4_decrypt,
};
use md5::{Digest, Md5};

const PASSWORD_PAD: [u8; 32] = [
    0x28, 0xbf, 0x4e, 0x5e, 0x4e, 0x75, 0x8a, 0x41, 0x64, 0x00, 0x4e, 0x56, 0xff, 0xfa, 0x01, 0x08,
    0x2e, 0x2e, 0x00, 0xb6, 0xd0, 0x68, 0x3e, 0x80, 0x2f, 0x0c, 0xa9, 0xfe, 0x64, 0x53, 0x69, 0x7a,
];

fn padd_password(password: &[u8]) -> [u8; 32] {
    let mut res = [0; 32];
    let mut i = 0;
    while i < password.len() && i < 32 {
        res[i] = password[i];
        i += 1;
    }
    let pad_length = 32 - i;
    for j in 0..pad_length {
        res[i + j] = PASSWORD_PAD[j];
    }
    res
}

#[derive(Debug, Default)]
pub struct CryptFilter {
    cfm: Option<String>,
    length: u32,
}

impl CryptFilter {
    pub fn new(cfm: String, length: u32) -> Self {
        Self {
            cfm: Some(cfm),
            length,
        }
    }
    pub fn try_new(cf: &PdfDict) -> PdfResult<Self> {
        let mut filter = CryptFilter::default();
        if let Some(cfm) = cf.get("CFM") {
            let cfm = cfm.as_name().unwrap().name();
            filter.cfm = Some(cfm.to_string());
        }
        if let Some(length) = cf.get("Length") {
            filter.length = length.as_number().unwrap().get_u32();
        }
        Ok(filter)
    }
}

#[derive(Default, Debug)]
pub struct SecurityHandler {
    version: i32,
    rvision: i32,
    o: Vec<u8>,
    u: Vec<u8>,
    p: i32,
    oe: Vec<u8>,
    ue: Vec<u8>,
    stream_filter: CryptFilter,
    string_filter: CryptFilter,
    length: u32,
    id1: Vec<u8>,
    encrypt_metadata: bool,
    key: Option<Vec<u8>>,
}

impl SecurityHandler {
    pub fn try_new(encrypt: &PdfDict, ids: PdfArray, password: Option<&[u8]>) -> PdfResult<Self> {
        let mut security_handler = SecurityHandler::default();
        unimplemented!()
    }
}
