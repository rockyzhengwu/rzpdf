use crate::{
    error::{PdfError, PdfResult},
    parser::options::Limits,
};

#[derive(Debug, Clone)]
pub(crate) struct Vertex {
    pub flag: u8,
    pub point: [f64; 2],
    pub components: Vec<f32>,
}

fn invalid(message: &str) -> PdfError {
    PdfError::ColorError(format!("shading mesh {message}"))
}

/// Build indexed triangles without cloning shared vertex/component payloads.
pub(crate) fn assemble_triangles(
    vertices: &Vec<Vertex>,
    limits: &Limits,
) -> PdfResult<Vec<[usize; 3]>> {
    if vertices.is_empty() {
        return Err(invalid("empty triangle mesh"));
    }
    let mut position = 0usize;
    let mut count = 0usize;
    while position < vertices.len() {
        match vertices[position].flag {
            0 => {
                if vertices.len() - position < 3 {
                    return Err(invalid("incomplete new triangle"));
                }
                position += 3;
            }
            1 | 2 if count != 0 => position += 1,
            1 | 2 => return Err(invalid("shared edge without a previous triangle")),
            _ => return Err(invalid("reserved triangle edge flag")),
        }
        count += 1;
    }
    if vertices
        .len()
        .checked_add(count)
        .is_none_or(|n| n > limits.max_objects)
    {
        return Err(PdfError::ResourceLimit(
            "mesh vertices and triangles".into(),
        ));
    }
    let mut bytes = vertices
        .capacity()
        .checked_mul(std::mem::size_of::<Vertex>())
        .ok_or_else(|| PdfError::ResourceLimit("mesh vertex storage overflow".into()))?;
    for vertex in vertices {
        bytes = vertex
            .components
            .capacity()
            .checked_mul(std::mem::size_of::<f32>())
            .and_then(|n| bytes.checked_add(n))
            .ok_or_else(|| PdfError::ResourceLimit("mesh component storage overflow".into()))?;
    }
    if count
        .checked_mul(std::mem::size_of::<[usize; 3]>())
        .and_then(|n| bytes.checked_add(n))
        .is_none_or(|n| n > limits.max_decoded_bytes)
    {
        return Err(PdfError::ResourceLimit(
            "mesh vertices and triangle storage".into(),
        ));
    }
    let mut triangles = Vec::new();
    triangles
        .try_reserve_exact(count)
        .map_err(|_| PdfError::ResourceLimit("mesh triangle allocation".into()))?;
    let mut previous = [0usize; 3];
    let mut position = 0usize;
    while position < vertices.len() {
        previous = match vertices[position].flag {
            0 => {
                let triangle = [position, position + 1, position + 2];
                position += 3;
                triangle
            }
            1 => {
                let triangle = [previous[1], previous[2], position];
                position += 1;
                triangle
            }
            2 => {
                let triangle = [previous[0], previous[2], position];
                position += 1;
                triangle
            }
            _ => return Err(invalid("reserved triangle edge flag")),
        };
        triangles.push(previous);
    }
    Ok(triangles)
}

/// Decode byte-aligned Type4 vertex records. Triangle topology is a separate
/// step: flags on the second/third vertices of a new triangle are ignored.
pub(crate) fn decode_vertices(
    data: &[u8],
    flag_bits: usize,
    coordinate_bits: usize,
    component_bits: usize,
    components: usize,
    decode: &[f32],
    limits: &Limits,
) -> PdfResult<Vec<Vertex>> {
    if !matches!(flag_bits, 2 | 4 | 8)
        || !matches!(coordinate_bits, 1 | 2 | 4 | 8 | 12 | 16 | 24 | 32)
        || !matches!(component_bits, 1 | 2 | 4 | 8 | 12 | 16)
    {
        return Err(invalid("invalid field bit widths"));
    }
    let pairs = components
        .checked_add(2)
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(|| PdfError::ResourceLimit("mesh component count overflow".into()))?;
    if components == 0 || decode.len() != pairs || decode.iter().any(|v| !v.is_finite()) {
        return Err(invalid("Decode component count or finite values"));
    }
    let bits = components
        .checked_mul(component_bits)
        .and_then(|n| n.checked_add(2 * coordinate_bits + flag_bits))
        .ok_or_else(|| PdfError::ResourceLimit("mesh record size overflow".into()))?;
    let stride = bits
        .checked_add(7)
        .map(|n| n / 8)
        .ok_or_else(|| PdfError::ResourceLimit("mesh record size overflow".into()))?;
    if data.len() % stride != 0 {
        return Err(invalid("truncated vertex record"));
    }
    let count = data.len() / stride;
    let component_bytes = components
        .checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| PdfError::ResourceLimit("mesh component storage overflow".into()))?;
    let bytes = count
        .checked_mul(std::mem::size_of::<Vertex>())
        .and_then(|n| {
            count
                .checked_mul(component_bytes)
                .and_then(|c| n.checked_add(c))
        })
        .and_then(|n| n.checked_add(data.len()));
    if count > limits.max_objects || bytes.is_none_or(|n| n > limits.max_decoded_bytes) {
        return Err(PdfError::ResourceLimit(
            "mesh retained vertices and input".into(),
        ));
    }
    let mut vertices = Vec::new();
    vertices
        .try_reserve_exact(count)
        .map_err(|_| PdfError::ResourceLimit("mesh vertex allocation".into()))?;
    for record in data.chunks_exact(stride) {
        let mut bit = 0usize;
        let mut read = |width: usize| {
            let mut value = 0u32;
            for _ in 0..width {
                value = (value << 1) | u32::from((record[bit / 8] >> (7 - bit % 8)) & 1);
                bit += 1;
            }
            value
        };
        let flag = (read(flag_bits) & 3) as u8;
        let interpolate = |value: u32, width: usize, pair: &[f32]| {
            let max = (1u64 << width) - 1;
            f64::from(pair[0])
                + f64::from(value) / max as f64 * (f64::from(pair[1]) - f64::from(pair[0]))
        };
        let x = interpolate(read(coordinate_bits), coordinate_bits, &decode[0..2]);
        let y = interpolate(read(coordinate_bits), coordinate_bits, &decode[2..4]);
        let mut values = Vec::new();
        values
            .try_reserve_exact(components)
            .map_err(|_| PdfError::ResourceLimit("mesh component allocation".into()))?;
        for pair in decode[4..].chunks_exact(2) {
            values.push(interpolate(read(component_bits), component_bits, pair) as f32);
        }
        vertices.push(Vertex {
            flag,
            point: [x, y],
            components: values,
        });
    }
    Ok(vertices)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn triangle_flags_share_edges_and_reject_incomplete_sequences() {
        let make = |flags: &[u8]| {
            flags
                .iter()
                .map(|&flag| Vertex {
                    flag,
                    point: [0.0, 0.0],
                    components: vec![0.0],
                })
                .collect::<Vec<_>>()
        };
        let vertices = make(&[0, 3, 3, 1, 2, 0, 2, 1]);
        let limits = Limits::default();
        assert_eq!(
            assemble_triangles(&vertices, &limits).unwrap(),
            [[0, 1, 2], [1, 2, 3], [1, 3, 4], [5, 6, 7]]
        );
        for flags in [
            vec![],
            vec![1],
            vec![2],
            vec![3],
            vec![0, 0],
            vec![0, 0, 0, 0],
        ] {
            assert!(assemble_triangles(&make(&flags), &limits).is_err());
        }
        let tight = Limits {
            max_objects: 11,
            ..limits.clone()
        };
        assert!(matches!(
            assemble_triangles(&vertices, &tight),
            Err(PdfError::ResourceLimit(_))
        ));
        let exact = Limits {
            max_objects: 12,
            ..limits
        };
        assert_eq!(assemble_triangles(&vertices, &exact).unwrap().len(), 4);
    }
    #[test]
    fn vertices_reset_padding_decode_and_handle_32_bit_coordinates() {
        let limits = Limits::default();
        let vertices = decode_vertices(
            &[0x3c, 0x03, 0x40, 0x3f],
            2,
            4,
            4,
            1,
            &[0.0, 10.0, 0.0, 20.0, 1.0, 0.0],
            &limits,
        )
        .unwrap();
        assert_eq!(vertices.len(), 2);
        assert_eq!(vertices[0].point, [10.0, 0.0]);
        assert_eq!(vertices[0].components, [1.0]);
        assert_eq!(vertices[1].flag, 1);
        assert_eq!(vertices[1].components, [0.0]);
        let data = [0xf1, 255, 255, 255, 255, 0, 0, 0, 0, 128];
        let result = decode_vertices(
            &data,
            8,
            32,
            8,
            1,
            &[-10.0, 10.0, 5.0, 15.0, 0.0, 1.0],
            &limits,
        )
        .unwrap();
        assert_eq!(result[0].flag, 1);
        assert_eq!(result[0].point, [10.0, 5.0]);
        assert!((result[0].components[0] - 128.0 / 255.0).abs() < 1e-6);
        assert!(
            decode_vertices(
                &data[..9],
                8,
                32,
                8,
                1,
                &[-10.0, 10.0, 5.0, 15.0, 0.0, 1.0],
                &limits
            )
            .is_err()
        );
        let tight = Limits {
            max_objects: 0,
            ..limits.clone()
        };
        assert!(matches!(
            decode_vertices(
                &data,
                8,
                32,
                8,
                1,
                &[-10.0, 10.0, 5.0, 15.0, 0.0, 1.0],
                &tight
            ),
            Err(PdfError::ResourceLimit(_))
        ));
    }
}
