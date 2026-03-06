pub fn real_from_buffer(buf: &[u8]) -> f32 {
    if buf.is_empty() {
        return 0_f32;
    }
    let mut i = 0;
    let flag: f32 = match buf[0] {
        43 => {
            i += 1;
            1_f32
        }
        45 => {
            i += 1;
            -1_f32
        }
        _ => 1_f32,
    };

    let mut ipart = 0_f32;
    while i < buf.len() && buf[i].is_ascii_digit() {
        ipart = ipart * 10_f32 + (buf[i] - b'0') as f32;
        i += 1
    }
    if i < buf.len() && buf[i] != b'.' {
        return flag * ipart;
    } else if i < buf.len() && buf[i] == b'.' {
        i += 1;
        let mut dpart = 0_f32;
        let mut n = 1_f32;
        while i < buf.len() && buf[i].is_ascii_digit() {
            n *= 10_f32;
            dpart = dpart * 10_f32 + (buf[i] - b'0') as f32;
            i += 1
        }
        return flag * (ipart + dpart / n);
    }

    flag * ipart
}

pub fn hex_to_u8(c: u8) -> u8 {
    let uc = c.to_ascii_uppercase();
    if uc > b'9' {
        return uc - b'A' + 10;
    } else {
        return uc - b'0';
    }
}
