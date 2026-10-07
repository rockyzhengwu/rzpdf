use crate::error::{PdfError, PdfResult};

pub struct Rc4 {
    s: [u8; 256],
    i: u8,
    j: u8,
}

impl Rc4 {
    // Initialize RC4 with a key
    pub fn new(key: &[u8]) -> PdfResult<Self> {
        if key.is_empty() || key.len() > 256 {
            return Err(PdfError::ParserError("Invalid RC4 key length".into()));
        }
        let mut s = [0u8; 256];
        for i in 0..256 {
            s[i] = i as u8;
        }

        let mut j = 0u8;
        for i in 0..256 {
            j = j.wrapping_add(s[i]).wrapping_add(key[i % key.len()]);
            s.swap(i, j as usize);
        }

        Ok(Rc4 { s, i: 0, j: 0 })
    }

    pub fn apply_keystream(&mut self, data: &mut [u8]) {
        for byte in data.iter_mut() {
            self.i = self.i.wrapping_add(1);
            self.j = self.j.wrapping_add(self.s[self.i as usize]);

            self.s.swap(self.i as usize, self.j as usize);

            let t = self.s[self.i as usize].wrapping_add(self.s[self.j as usize]);
            *byte ^= self.s[t as usize];
        }
    }
}

pub fn rc4_decrypt(key: &[u8], data: &[u8]) -> PdfResult<Vec<u8>> {
    let mut cipher = Rc4::new(key)?;
    let mut data = data.to_vec();
    cipher.apply_keystream(&mut data);
    Ok(data)
}

type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

use aes::cipher::block_padding::ZeroPadding;
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};

pub fn aes128_decrypt(key: &[u8], data: &[u8]) -> PdfResult<Vec<u8>> {
    let (iv, payload) = split_encrypted_data(data)?;
    let mut data = payload.to_vec();
    let cipher = Aes128CbcDec::new_from_slices(key, iv)
        .map_err(|_| PdfError::ParserError("Invalid AES-128 key or IV".into()))?;
    let plaintext = cipher
        .decrypt_padded_mut::<Pkcs7>(&mut data)
        .map_err(|_| PdfError::ParserError("Invalid AES padding or ciphertext".into()))?;
    Ok(plaintext.to_vec())
}

pub fn aes256_decrypt(key: &[u8], data: &[u8]) -> PdfResult<Vec<u8>> {
    let (iv, payload) = split_encrypted_data(data)?;
    let mut data = payload.to_vec();
    let cipher = Aes256CbcDec::new_from_slices(key, iv)
        .map_err(|_| PdfError::ParserError("Invalid AES-256 key or IV".into()))?;
    let plaintext = cipher
        .decrypt_padded_mut::<Pkcs7>(&mut data)
        .map_err(|_| PdfError::ParserError("Invalid AES padding or ciphertext".into()))?;
    Ok(plaintext.to_vec())
}

fn split_encrypted_data(data: &[u8]) -> PdfResult<(&[u8], &[u8])> {
    if data.len() < 32 || (data.len() - 16) % 16 != 0 {
        return Err(PdfError::ParserError(
            "Invalid IV-prefixed AES ciphertext length".into(),
        ));
    }
    Ok(data.split_at(16))
}

pub fn aes_cbc_encrypt(key: &[u8], iv: &[u8], data: &[u8]) -> PdfResult<Vec<u8>> {
    let cipher = Aes128CbcEnc::new_from_slices(key, iv)
        .map_err(|_| PdfError::ParserError("Invalid AES-128 key or IV".into()))?;
    let capacity = data
        .len()
        .checked_add(15)
        .map(|n| n / 16 * 16)
        .ok_or_else(|| PdfError::ResourceLimit("AES buffer size overflow".into()))?;
    let mut data = data.to_vec();
    let length = data.len();
    data.try_reserve_exact(capacity - length)
        .map_err(|_| PdfError::ResourceLimit("AES buffer allocation".into()))?;
    data.resize(capacity, 0);
    let ciphertext = cipher
        .encrypt_padded_mut::<ZeroPadding>(&mut data, length)
        .map_err(|_| PdfError::ParserError("Invalid AES encryption buffer".into()))?;
    Ok(ciphertext.to_vec())
}
pub fn aes_cbc_decrypt(key: &[u8], iv: &[u8], data: &[u8]) -> PdfResult<Vec<u8>> {
    let cipher = Aes256CbcDec::new_from_slices(key, iv)
        .map_err(|_| PdfError::ParserError("Invalid AES-256 key or IV".into()))?;
    let mut data = data.to_vec();
    let plaintext = cipher
        .decrypt_padded_mut::<ZeroPadding>(&mut data)
        .map_err(|_| PdfError::ParserError("Invalid AES ciphertext".into()))?;
    Ok(plaintext.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rc4_known_vector_and_invalid_keys() {
        assert_eq!(
            rc4_decrypt(b"Key", b"Plaintext").unwrap(),
            [0xbb, 0xf3, 0x16, 0xe8, 0xd9, 0x40, 0xaf, 0x0a, 0xd3]
        );
        assert!(rc4_decrypt(&[], b"text").is_err());
        assert!(Rc4::new(&[0; 257]).is_err());
    }

    #[test]
    fn aes_prefixed_ciphertext_returns_exact_unpadded_plaintext() {
        for plaintext in [b"".as_slice(), b"hello", &[7; 16], &[9; 31]] {
            let iv = [3; 16];
            let mut buffer = vec![0; plaintext.len() + 16];
            buffer[..plaintext.len()].copy_from_slice(plaintext);
            let encrypted = Aes128CbcEnc::new_from_slices(&[1; 16], &iv)
                .unwrap()
                .encrypt_padded_mut::<Pkcs7>(&mut buffer, plaintext.len())
                .unwrap();
            let mut prefixed = iv.to_vec();
            prefixed.extend_from_slice(encrypted);
            assert_eq!(aes128_decrypt(&[1; 16], &prefixed).unwrap(), plaintext);
            let mut buffer = vec![0; plaintext.len() + 16];
            buffer[..plaintext.len()].copy_from_slice(plaintext);
            let encrypted = cbc::Encryptor::<aes::Aes256>::new_from_slices(&[1; 32], &iv)
                .unwrap()
                .encrypt_padded_mut::<Pkcs7>(&mut buffer, plaintext.len())
                .unwrap();
            let mut prefixed = iv.to_vec();
            prefixed.extend_from_slice(encrypted);
            assert_eq!(aes256_decrypt(&[1; 32], &prefixed).unwrap(), plaintext);
        }
    }

    #[test]
    fn aes_rejects_bad_lengths_keys_ivs_and_padding() {
        for length in [0, 1, 15, 16, 17, 31, 33] {
            assert!(aes128_decrypt(&[1; 16], &vec![0; length]).is_err());
            assert!(aes256_decrypt(&[1; 32], &vec![0; length]).is_err());
        }
        assert!(aes128_decrypt(&[], &[0; 32]).is_err());
        assert!(aes256_decrypt(&[1; 16], &[0; 32]).is_err());
        // Force a zero final padding byte in an otherwise valid block.
        let mut buffer = [16_u8; 16];
        buffer[15] = 0;
        let encrypted = Aes128CbcEnc::new_from_slices(&[1; 16], &[0; 16])
            .unwrap()
            .encrypt_padded_mut::<aes::cipher::block_padding::NoPadding>(&mut buffer, 16)
            .unwrap();
        let mut prefixed = vec![0; 16];
        prefixed.extend_from_slice(encrypted);
        assert!(aes128_decrypt(&[1; 16], &prefixed).is_err());
        assert!(aes_cbc_encrypt(&[], &[0; 16], b"hello").is_err());
        assert!(aes_cbc_encrypt(&[0; 16], &[], b"hello").is_err());
        assert!(aes_cbc_decrypt(&[0; 32], &[0; 16], &[0; 17]).is_err());
        assert!(aes_cbc_decrypt(&[0; 32], &[], &[0; 16]).is_err());
        assert_eq!(
            aes_cbc_encrypt(&[0; 16], &[0; 16], b"hello").unwrap().len(),
            16
        );
    }
}
