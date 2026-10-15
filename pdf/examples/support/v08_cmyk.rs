//! Project-authored synthetic ICC v2 CMYK -> D50 XYZ LUT. Not a print profile.
pub fn profile() -> Vec<u8> {
    fn fixed(v: f64) -> [u8; 4] {
        ((v * 65536.).round() as i32).to_be_bytes()
    }
    let mut lut = b"mft2\0\0\0\0".to_vec();
    lut.extend([4, 3, 2, 0]);
    for v in [1., 0., 0., 0., 1., 0., 0., 0., 1.] {
        lut.extend(fixed(v));
    }
    // lut16Type defines PCS XYZ as u1.15. lut8Type + XYZ would be ambiguous.
    lut.extend(2u16.to_be_bytes());
    lut.extend(2u16.to_be_bytes());
    for _ in 0..4 {
        lut.extend(0u16.to_be_bytes());
        lut.extend(u16::MAX.to_be_bytes());
    }
    for vertex in 0..16 {
        let c = ((vertex >> 3) & 1) as f64;
        let m = ((vertex >> 2) & 1) as f64;
        let y = ((vertex >> 1) & 1) as f64;
        let k = (vertex & 1) as f64;
        // Affine synthetic response avoids differences between legitimate
        // multidimensional LUT interpolation policies in different CMS engines.
        let rgb = [
            1. - 0.75 * c - 0.25 * k,
            1. - 0.75 * m - 0.25 * k,
            1. - 0.75 * y - 0.25 * k,
        ];
        for row in [
            [0.4360747, 0.3850649, 0.1430804],
            [0.2225045, 0.7168786, 0.0606169],
            [0.0139322, 0.0971045, 0.7141733],
        ] {
            let xyz: f64 = row.into_iter().zip(rgb).map(|(a, b)| a * b).sum();
            lut.extend(((xyz * 32768.).round().clamp(0., 65535.) as u16).to_be_bytes());
        }
    }
    for _ in 0..3 {
        lut.extend(0u16.to_be_bytes());
        lut.extend(u16::MAX.to_be_bytes());
    }
    let mut white = b"XYZ \0\0\0\0".to_vec();
    for v in [0.9642, 1., 0.8249] {
        white.extend(fixed(v));
    }
    let mut description = b"desc\0\0\0\0".to_vec();
    let name = b"rzpdf synthetic CMYK test only\0";
    description.extend((name.len() as u32).to_be_bytes());
    description.extend(name);
    description.extend([0; 78]);
    let mut copyright = b"text\0\0\0\0".to_vec();
    copyright.extend(b"Project-authored regression data; AGPL-3.0-only\0");
    let tags = [
        (*b"A2B0", lut.clone()),
        (*b"A2B1", lut.clone()),
        (*b"A2B2", lut),
        (*b"wtpt", white),
        (*b"desc", description),
        (*b"cprt", copyright),
    ];
    let mut bytes = vec![0; 132 + tags.len() * 12];
    bytes[8..12].copy_from_slice(&[2, 0x10, 0, 0]);
    bytes[12..16].copy_from_slice(b"prtr");
    bytes[16..20].copy_from_slice(b"CMYK");
    bytes[20..24].copy_from_slice(b"XYZ ");
    for (offset, value) in [(24, 2026u16), (26, 10), (28, 15)] {
        bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
    }
    bytes[36..40].copy_from_slice(b"acsp");
    bytes[40..44].copy_from_slice(b"APPL");
    for (offset, value) in [(68, 0.9642), (72, 1.), (76, 0.8249)] {
        bytes[offset..offset + 4].copy_from_slice(&fixed(value));
    }
    bytes[128..132].copy_from_slice(&(tags.len() as u32).to_be_bytes());
    for (index, (name, payload)) in tags.into_iter().enumerate() {
        while bytes.len() % 4 != 0 {
            bytes.push(0);
        }
        let offset = 132 + index * 12;
        let start = bytes.len() as u32;
        bytes[offset..offset + 4].copy_from_slice(&name);
        bytes[offset + 4..offset + 8].copy_from_slice(&start.to_be_bytes());
        bytes[offset + 8..offset + 12].copy_from_slice(&(payload.len() as u32).to_be_bytes());
        bytes.extend(payload);
    }
    let size = bytes.len() as u32;
    bytes[..4].copy_from_slice(&size.to_be_bytes());
    bytes
}
