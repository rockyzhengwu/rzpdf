use crate::{
    error::{PdfError, PdfResult},
    objects::pdf_stream::PdfStream,
    parser::options::Limits,
    pdf_context::PDFContext,
};

#[derive(Debug, Clone)]
pub(crate) struct Vertex {
    pub flag: u8,
    pub point: [f64; 2],
    pub components: Vec<f32>,
}

/// Owns shared vertex payloads once; triangle indices cannot be changed after
/// validation. Sampling returns encoded components, not display RGB.
#[derive(Debug, Clone)]
pub(crate) struct TriangleMesh {
    vertices: Vec<Vertex>,
    triangles: Vec<[usize; 3]>,
    bounds: [f64; 4],
}

pub(crate) struct MeshEncoding<'a> {
    pub flag_bits: usize,
    pub coordinate_bits: usize,
    pub component_bits: usize,
    pub components: usize,
    pub decode: &'a [f32],
}

impl TriangleMesh {
    /// The caller derives encoded component count from ColorSpace/Function.
    #[cfg(test)]
    pub(crate) fn from_stream<R: std::io::Read + std::io::Seek>(
        stream: &PdfStream,
        components: usize,
        ctx: &PDFContext<R>,
    ) -> PdfResult<Self> {
        Self::from_stream_with_retained(stream, components, ctx, 0)
    }

    pub(crate) fn from_stream_with_retained<R: std::io::Read + std::io::Seek>(
        stream: &PdfStream,
        components: usize,
        ctx: &PDFContext<R>,
        already_retained: usize,
    ) -> PdfResult<Self> {
        let required = |key: &str| {
            stream
                .dict()
                .get(key)
                .ok_or_else(|| invalid(&format!("missing {key}")))
        };
        let integer = |key: &str| -> PdfResult<usize> {
            let object = ctx.resolve(required(key)?)?;
            let number = object
                .as_number()
                .ok_or_else(|| invalid("bit widths require integers"))?;
            usize::try_from(number.as_u64_checked()?)
                .map_err(|_| invalid("integer outside platform range"))
        };
        let kind = integer("ShadingType")?;
        let per_row = match kind {
            4 => None,
            5 => Some(integer("VerticesPerRow")?),
            _ => return Err(invalid("triangle stream requires ShadingType 4 or 5")),
        };
        if per_row.is_some_and(|width| width < 2) {
            return Err(invalid("VerticesPerRow must be at least two"));
        }
        if per_row.is_some_and(|width| {
            width
                .checked_mul(4)
                .and_then(|n| n.checked_sub(2))
                .is_none_or(|minimum| minimum > ctx.limits().max_objects)
        }) {
            // Two complete rows require 2*w vertices and 2*(w-1) triangles.
            return Err(PdfError::ResourceLimit(
                "lattice minimum rows and triangles".into(),
            ));
        }
        let flag_bits = if kind == 4 {
            integer("BitsPerFlag")?
        } else {
            0
        };
        let coordinate_bits = integer("BitsPerCoordinate")?;
        let component_bits = integer("BitsPerComponent")?;
        if (kind == 4 && !matches!(flag_bits, 2 | 4 | 8))
            || !matches!(coordinate_bits, 1 | 2 | 4 | 8 | 12 | 16 | 24 | 32)
            || !matches!(component_bits, 1 | 2 | 4 | 8 | 12 | 16)
        {
            return Err(invalid("invalid field bit widths"));
        }
        let count = components
            .checked_add(2)
            .and_then(|n| n.checked_mul(2))
            .ok_or_else(|| PdfError::ResourceLimit("mesh Decode size".into()))?;
        let object = ctx.resolve(required("Decode")?)?;
        let array = object
            .as_array()
            .filter(|a| components != 0 && a.len() == count)
            .ok_or_else(|| invalid("Decode component count"))?;
        let mut limits = ctx.limits();
        limits.max_decoded_bytes = limits
            .max_decoded_bytes
            .checked_sub(already_retained)
            .ok_or_else(|| PdfError::ResourceLimit("mesh and existing shading payload".into()))?;
        if count > limits.max_objects
            || count
                .checked_mul(4)
                .is_none_or(|n| n > limits.max_decoded_bytes)
        {
            return Err(PdfError::ResourceLimit("mesh Decode values".into()));
        }
        let mut decode = Vec::new();
        decode
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("mesh Decode allocation".into()))?;
        for entry in array {
            decode.push(
                ctx.resolve(entry)?
                    .as_f32()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| invalid("Decode must contain finite numbers"))?,
            );
        }
        limits.max_decoded_bytes = limits
            .max_decoded_bytes
            .checked_sub(
                decode
                    .capacity()
                    .checked_mul(std::mem::size_of::<f32>())
                    .ok_or_else(|| {
                        PdfError::ResourceLimit("mesh Decode storage overflow".into())
                    })?,
            )
            .ok_or_else(|| PdfError::ResourceLimit("mesh Decode storage".into()))?;
        let data = stream.decode_data(ctx)?;
        if let Some(width) = per_row {
            return Self::decode_owned_lattice_input(
                &data,
                MeshEncoding {
                    flag_bits: 0,
                    coordinate_bits,
                    component_bits,
                    components,
                    decode: &decode,
                },
                width,
                &limits,
            );
        }
        Self::decode_owned_input(
            &data,
            MeshEncoding {
                flag_bits,
                coordinate_bits,
                component_bits,
                components,
                decode: &decode,
            },
            &limits,
        )
    }

    fn decode_owned_input(
        data: &Vec<u8>,
        encoding: MeshEncoding<'_>,
        limits: &Limits,
    ) -> PdfResult<Self> {
        let mut capacity_limits = *limits;
        capacity_limits.max_decoded_bytes = limits
            .max_decoded_bytes
            .checked_sub(data.capacity() - data.len())
            .ok_or_else(|| PdfError::ResourceLimit("mesh decoded input capacity".into()))?;
        Self::decode(data, encoding, &capacity_limits)
    }

    fn decode_owned_lattice_input(
        data: &Vec<u8>,
        encoding: MeshEncoding<'_>,
        per_row: usize,
        limits: &Limits,
    ) -> PdfResult<Self> {
        let mut capacity_limits = *limits;
        capacity_limits.max_decoded_bytes = limits
            .max_decoded_bytes
            .checked_sub(data.capacity() - data.len())
            .ok_or_else(|| PdfError::ResourceLimit("lattice decoded input capacity".into()))?;
        let vertices = decode_lattice_vertices(
            data,
            encoding.coordinate_bits,
            encoding.component_bits,
            encoding.components,
            encoding.decode,
            &capacity_limits,
        )?;
        capacity_limits.max_decoded_bytes = capacity_limits
            .max_decoded_bytes
            .checked_sub(data.len())
            .ok_or_else(|| PdfError::ResourceLimit("lattice input and topology".into()))?;
        Self::try_new_lattice(vertices, per_row, &capacity_limits)
    }

    pub(crate) fn decode(
        data: &[u8],
        encoding: MeshEncoding<'_>,
        limits: &Limits,
    ) -> PdfResult<Self> {
        let vertices = decode_vertices(
            data,
            encoding.flag_bits,
            encoding.coordinate_bits,
            encoding.component_bits,
            encoding.components,
            encoding.decode,
            limits,
        )?;
        // The borrowed decoded stream is still alive while allocating topology.
        // Reserve it from the aggregate allowance instead of checking each
        // retained allocation independently against the full limit.
        let mut retained_limits = *limits;
        retained_limits.max_decoded_bytes = limits
            .max_decoded_bytes
            .checked_sub(data.len())
            .ok_or_else(|| PdfError::ResourceLimit("mesh input and topology".into()))?;
        Self::try_new(vertices, &retained_limits)
    }

    pub(crate) fn retained_payload_bytes(&self) -> PdfResult<usize> {
        let mut bytes = self
            .vertices
            .capacity()
            .checked_mul(std::mem::size_of::<Vertex>())
            .and_then(|n| {
                self.triangles
                    .capacity()
                    .checked_mul(std::mem::size_of::<[usize; 3]>())
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or_else(|| PdfError::ResourceLimit("mesh retained storage overflow".into()))?;
        for vertex in &self.vertices {
            bytes = vertex
                .components
                .capacity()
                .checked_mul(std::mem::size_of::<f32>())
                .and_then(|n| bytes.checked_add(n))
                .ok_or_else(|| {
                    PdfError::ResourceLimit("mesh retained components overflow".into())
                })?;
        }
        Ok(bytes)
    }

    pub(crate) fn try_new(vertices: Vec<Vertex>, limits: &Limits) -> PdfResult<Self> {
        Self::from_vertices(vertices, None, limits)
    }

    pub(crate) fn try_new_lattice(
        vertices: Vec<Vertex>,
        per_row: usize,
        limits: &Limits,
    ) -> PdfResult<Self> {
        Self::from_vertices(vertices, Some(per_row), limits)
    }

    fn from_vertices(
        vertices: Vec<Vertex>,
        per_row: Option<usize>,
        limits: &Limits,
    ) -> PdfResult<Self> {
        let components = vertices.first().map_or(0, |v| v.components.len());
        if components == 0
            || vertices.iter().any(|v| {
                v.components.len() != components
                    || v.point.iter().any(|x| !x.is_finite())
                    || v.components.iter().any(|x| !x.is_finite())
            })
        {
            return Err(invalid("vertex dimensions or finite values"));
        }
        let triangles = match per_row {
            Some(width) => assemble_lattice(&vertices, width, limits)?,
            None => assemble_triangles(&vertices, limits)?,
        };
        let [x, y] = vertices[0].point;
        let mut bounds = [x, y, x, y];
        for vertex in &vertices {
            bounds[0] = bounds[0].min(vertex.point[0]);
            bounds[1] = bounds[1].min(vertex.point[1]);
            bounds[2] = bounds[2].max(vertex.point[0]);
            bounds[3] = bounds[3].max(vertex.point[1]);
        }
        Ok(Self {
            vertices,
            triangles,
            bounds,
        })
    }

    /// Conservative rejection only; points inside still need triangle tests.
    pub(crate) fn bounds_contains(&self, point: [f64; 2]) -> bool {
        point.iter().all(|value| value.is_finite())
            && point[0] >= self.bounds[0]
            && point[0] <= self.bounds[2]
            && point[1] >= self.bounds[1]
            && point[1] <= self.bounds[3]
    }

    pub(crate) fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    pub(crate) fn component_count(&self) -> usize {
        self.vertices[0].components.len()
    }

    pub(crate) fn sample_triangle(
        &self,
        index: usize,
        point: [f64; 2],
        output: &mut [f32],
    ) -> PdfResult<bool> {
        let indices = self
            .triangles
            .get(index)
            .ok_or_else(|| invalid("triangle index out of bounds"))?;
        interpolate_triangle(indices.map(|i| &self.vertices[i]), point, output)
    }
}

fn invalid(message: &str) -> PdfError {
    PdfError::ColorError(format!("shading mesh {message}"))
}

/// Weights in vertex order, before applying any colour-space conversion or
/// optional shading function. Either winding is accepted; collapsed triangles
/// have no area and therefore paint nothing.
pub(crate) fn barycentric(points: [[f64; 2]; 3], point: [f64; 2]) -> Option<[f64; 3]> {
    if points
        .iter()
        .flatten()
        .chain(point.iter())
        .any(|v| !v.is_finite())
    {
        return None;
    }
    let [a, b, c] = points;
    let cross = |u: [f64; 2], v: [f64; 2]| u[0] * v[1] - u[1] * v[0];
    let sub = |u: [f64; 2], v: [f64; 2]| [u[0] - v[0], u[1] - v[1]];
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(point, a);
    let determinant = cross(ab, ac);
    if determinant == 0.0 || !determinant.is_finite() {
        return None;
    }
    let wb = cross(ap, ac) / determinant;
    let wc = cross(ab, ap) / determinant;
    let weights = [1.0 - wb - wc, wb, wc];
    weights
        .iter()
        .all(|v| v.is_finite() && *v >= 0.0 && *v <= 1.0)
        .then_some(weights)
}

/// Interpolate encoded colour components (or a single function parameter),
/// without allocating a temporary buffer or converting vertex colours to RGB.
pub(crate) fn interpolate_triangle(
    vertices: [&Vertex; 3],
    point: [f64; 2],
    output: &mut [f32],
) -> PdfResult<bool> {
    if output.is_empty()
        || vertices.iter().any(|v| {
            v.components.len() != output.len() || v.components.iter().any(|c| !c.is_finite())
        })
    {
        return Err(invalid("triangle component dimensions or finite values"));
    }
    let Some(weights) = barycentric(vertices.map(|v| v.point), point) else {
        return Ok(false);
    };
    for (index, component) in output.iter_mut().enumerate() {
        *component = vertices
            .iter()
            .zip(weights)
            .map(|(vertex, weight)| f64::from(vertex.components[index]) * weight)
            .sum::<f64>() as f32;
    }
    Ok(true)
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
    check_triangle_storage(vertices, count, limits)?;
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

fn check_triangle_storage(vertices: &Vec<Vertex>, count: usize, limits: &Limits) -> PdfResult<()> {
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
    Ok(())
}

pub(crate) fn assemble_lattice(
    vertices: &Vec<Vertex>,
    per_row: usize,
    limits: &Limits,
) -> PdfResult<Vec<[usize; 3]>> {
    if per_row < 2 || !vertices.len().is_multiple_of(per_row) || vertices.len() / per_row < 2 {
        return Err(invalid("lattice requires at least two complete rows"));
    }
    let rows = vertices.len() / per_row;
    let count = (rows - 1)
        .checked_mul(per_row - 1)
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(|| PdfError::ResourceLimit("lattice triangle count overflow".into()))?;
    check_triangle_storage(vertices, count, limits)?;
    let mut triangles = Vec::new();
    triangles
        .try_reserve_exact(count)
        .map_err(|_| PdfError::ResourceLimit("lattice triangle allocation".into()))?;
    for row in 0..rows - 1 {
        for column in 0..per_row - 1 {
            let a = row * per_row + column;
            triangles.push([a, a + 1, a + per_row]);
            triangles.push([a + 1, a + per_row, a + per_row + 1]);
        }
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
    if !matches!(flag_bits, 2 | 4 | 8) {
        return Err(invalid("invalid Type4 flag bit width"));
    }
    decode_vertex_records(
        data,
        flag_bits,
        coordinate_bits,
        component_bits,
        components,
        decode,
        limits,
    )
}

pub(crate) fn decode_lattice_vertices(
    data: &[u8],
    coordinate_bits: usize,
    component_bits: usize,
    components: usize,
    decode: &[f32],
    limits: &Limits,
) -> PdfResult<Vec<Vertex>> {
    decode_vertex_records(
        data,
        0,
        coordinate_bits,
        component_bits,
        components,
        decode,
        limits,
    )
}

fn decode_vertex_records(
    data: &[u8],
    flag_bits: usize,
    coordinate_bits: usize,
    component_bits: usize,
    components: usize,
    decode: &[f32],
    limits: &Limits,
) -> PdfResult<Vec<Vertex>> {
    if !matches!(flag_bits, 0 | 2 | 4 | 8)
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
    if !data.len().is_multiple_of(stride) {
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
        for pair in decode[4..].as_chunks::<2>().0 {
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
    fn lattice_row_width_budget_is_rejected_before_payload_decode() {
        use crate::{
            document::PdfDocument,
            io::stream_reader::StreamReader,
            parser::{options::ParseOptions, syntax::SyntaxParser},
        };
        let doc = PdfDocument::new_with_options(ParseOptions {
            limits: Limits {
                max_objects: 9,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        // Missing remaining fields proves row-budget rejection occurs before
        // Decode allocation or filter/payload processing.
        let source = b"<< /ShadingType 5 /VerticesPerRow 3 >>";
        let dict = SyntaxParser::new(
            StreamReader::try_new(std::io::Cursor::new(source.to_vec())).unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap();
        let stream = PdfStream::new(dict, Vec::new());
        assert!(matches!(TriangleMesh::from_stream(&stream, 1, doc.ctx()),
            Err(PdfError::ResourceLimit(message)) if message == "lattice minimum rows and triangles"));
    }
    #[test]
    fn lattice_owned_input_capacity_and_topology_share_allowance() {
        let mut data = Vec::with_capacity(128);
        data.extend([0, 0, 0, 255, 0, 255, 0, 255, 0, 255, 255, 255]);
        let decode = [0.0, 4.0, 0.0, 4.0, 0.0, 1.0];
        let encoding = || MeshEncoding {
            flag_bits: 0,
            coordinate_bits: 8,
            component_bits: 8,
            components: 1,
            decode: &decode,
        };
        let limits = Limits::default();
        let mesh = TriangleMesh::decode_owned_lattice_input(&data, encoding(), 2, &limits).unwrap();
        let total = mesh.retained_payload_bytes().unwrap() + data.capacity();
        let exact = Limits {
            max_decoded_bytes: total,
            ..limits
        };
        assert!(TriangleMesh::decode_owned_lattice_input(&data, encoding(), 2, &exact).is_ok());
        let tight = Limits {
            max_decoded_bytes: total - 1,
            ..limits
        };
        assert!(matches!(
            TriangleMesh::decode_owned_lattice_input(&data, encoding(), 2, &tight),
            Err(PdfError::ResourceLimit(_))
        ));
        assert!(TriangleMesh::decode_owned_lattice_input(&data, encoding(), 2, &exact).is_ok());
    }
    #[test]
    fn lattice_records_have_no_flags_and_reset_byte_padding() {
        let limits = Limits::default();
        let decode = [0.0, 10.0, 0.0, 20.0, 0.0, 1.0];
        let vertices =
            decode_lattice_vertices(&[0xf0, 0x0f, 0x0f, 0xff], 4, 4, 1, &decode, &limits).unwrap();
        assert_eq!(vertices.len(), 2);
        assert_eq!(vertices[0].flag, 0);
        assert_eq!(vertices[0].point, [10.0, 0.0]);
        assert_eq!(vertices[0].components, [0.0]);
        assert_eq!(vertices[1].point, [0.0, 20.0]);
        assert_eq!(vertices[1].components, [1.0]);
        assert!(decode_lattice_vertices(&[0xf0], 4, 4, 1, &decode, &limits).is_err());
        assert!(decode_vertices(&[0xf0, 0x0f], 0, 4, 4, 1, &decode, &limits).is_err());
        let exact = Limits {
            max_decoded_bytes: 2 * (std::mem::size_of::<Vertex>() + 4) + 4,
            ..limits
        };
        assert!(
            decode_lattice_vertices(&[0xf0, 0x0f, 0x0f, 0xff], 4, 4, 1, &decode, &exact).is_ok()
        );
        let tight = Limits {
            max_decoded_bytes: exact.max_decoded_bytes - 1,
            ..exact
        };
        assert!(matches!(
            decode_lattice_vertices(&[0xf0, 0x0f, 0x0f, 0xff], 4, 4, 1, &decode, &tight),
            Err(PdfError::ResourceLimit(_))
        ));
    }
    #[test]
    fn lattice_connects_complete_rows_and_checks_combined_budget() {
        let vertices: Vec<_> = (0..6)
            .map(|index| Vertex {
                flag: 3,
                point: [(index % 3) as f64, (index / 3) as f64],
                components: vec![0.0],
            })
            .collect();
        let limits = Limits::default();
        assert_eq!(
            assemble_lattice(&vertices, 3, &limits).unwrap(),
            [[0, 1, 3], [1, 3, 4], [1, 2, 4], [2, 4, 5]]
        );
        for width in [0, 1, 4, 6, usize::MAX] {
            assert!(assemble_lattice(&vertices, width, &limits).is_err());
        }
        let tight = Limits {
            max_objects: 9,
            ..limits
        };
        assert!(matches!(
            assemble_lattice(&vertices, 3, &tight),
            Err(PdfError::ResourceLimit(_))
        ));
        let exact = Limits {
            max_objects: 10,
            ..limits
        };
        assert_eq!(assemble_lattice(&vertices, 3, &exact).unwrap().len(), 4);
        let mesh = TriangleMesh::try_new_lattice(vertices, 3, &exact).unwrap();
        assert_eq!(mesh.triangle_count(), 4);
        let mut value = [-1.0];
        assert!(mesh.sample_triangle(3, [1.75, 0.75], &mut value).unwrap());
        assert_eq!(value, [0.0]);
    }
    #[test]
    fn decoded_input_spare_capacity_is_charged() {
        let mut data = Vec::with_capacity(128);
        data.extend([0, 0, 0, 0, 0, 255, 0, 255, 0, 0, 255, 0]);
        let decode = [0.0, 4.0, 0.0, 4.0, 0.0, 1.0];
        let encoding = || MeshEncoding {
            flag_bits: 8,
            coordinate_bits: 8,
            component_bits: 8,
            components: 1,
            decode: &decode,
        };
        let limits = Limits::default();
        let mesh = TriangleMesh::decode_owned_input(&data, encoding(), &limits).unwrap();
        let exact = Limits {
            max_decoded_bytes: mesh.retained_payload_bytes().unwrap() + data.capacity(),
            ..limits
        };
        assert!(TriangleMesh::decode_owned_input(&data, encoding(), &exact).is_ok());
        let tight = Limits {
            max_decoded_bytes: exact.max_decoded_bytes - 1,
            ..exact
        };
        assert!(matches!(
            TriangleMesh::decode_owned_input(&data, encoding(), &tight),
            Err(PdfError::ResourceLimit(_))
        ));
        assert!(TriangleMesh::decode(&data, encoding(), &tight).is_ok());
    }
    #[test]
    fn stream_budget_reserves_existing_shading_payload() {
        use crate::{
            document::PdfDocument,
            io::stream_reader::StreamReader,
            parser::{options::ParseOptions, syntax::SyntaxParser},
        };
        let source = b"<< /ShadingType 4 /BitsPerFlag 8 /BitsPerCoordinate 8 /BitsPerComponent 8 /Decode [0 4 0 4 0 1] >>";
        let dict = SyntaxParser::new(
            StreamReader::try_new(std::io::Cursor::new(source.to_vec())).unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap();
        let stream = PdfStream::new(dict, vec![0, 0, 0, 0, 0, 255, 0, 255, 0, 0, 255, 0]);
        let doc = PdfDocument::new().unwrap();
        let mesh = TriangleMesh::from_stream(&stream, 1, doc.ctx()).unwrap();
        let existing = 100;
        let total = mesh.retained_payload_bytes().unwrap() + 12 + 24 + existing;
        for (limit, accepted) in [(total, true), (total - 1, false)] {
            let constrained = PdfDocument::new_with_options(ParseOptions {
                limits: Limits {
                    max_decoded_bytes: limit,
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();
            let result =
                TriangleMesh::from_stream_with_retained(&stream, 1, constrained.ctx(), existing);
            if accepted {
                assert!(result.is_ok());
            } else {
                assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
                assert!(
                    TriangleMesh::from_stream_with_retained(&stream, 1, constrained.ctx(), 0)
                        .is_ok()
                );
            }
        }
    }
    #[test]
    fn stream_fields_validate_exact_integers_dimensions_and_truncation() {
        use crate::{
            document::PdfDocument, io::stream_reader::StreamReader, parser::syntax::SyntaxParser,
        };
        let doc = PdfDocument::new().unwrap();
        let make = |source: &str, data: Vec<u8>| {
            let object = SyntaxParser::new(
                StreamReader::try_new(std::io::Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap();
            PdfStream::new(object.into_dict().unwrap(), data)
        };
        let source = "<< /ShadingType 4 /BitsPerFlag 8 /BitsPerCoordinate 8 /BitsPerComponent 8 /Decode [0 4 0 4 0 1] >>";
        let data = vec![0, 0, 0, 0, 0, 255, 0, 255, 0, 0, 255, 0];
        let mesh = TriangleMesh::from_stream(&make(source, data.clone()), 1, doc.ctx()).unwrap();
        let mut output = [-1.0];
        assert!(mesh.sample_triangle(0, [1.0, 1.0], &mut output).unwrap());
        assert_eq!(output, [0.25]);
        let width = doc
            .try_insert_object(
                make(source, data.clone())
                    .dict()
                    .get("BitsPerCoordinate")
                    .unwrap()
                    .clone(),
            )
            .unwrap();
        let indirect_source = source.replace(
            "/BitsPerCoordinate 8",
            &format!(
                "/BitsPerCoordinate {} {} R",
                width.obj_num(),
                width.gen_num()
            ),
        );
        assert!(
            TriangleMesh::from_stream(&make(&indirect_source, data.clone()), 1, doc.ctx()).is_ok()
        );
        for (old, new) in [
            ("/ShadingType 4", "/ShadingType 5"),
            ("/BitsPerFlag 8", "/BitsPerFlag 8.0"),
            ("/BitsPerFlag 8", "/BitsPerFlag -8"),
            ("/BitsPerCoordinate 8", "/BitsPerCoordinate 3"),
            ("/BitsPerComponent 8", "/BitsPerComponent 32"),
            ("/BitsPerFlag 8", ""),
            ("[0 4 0 4 0 1]", "[0 4 0 4]"),
            ("[0 4 0 4 0 1]", "[0 4 0 4 0 /NaN]"),
        ] {
            assert!(
                TriangleMesh::from_stream(
                    &make(&source.replace(old, new), data.clone()),
                    1,
                    doc.ctx()
                )
                .is_err(),
                "{new}"
            );
        }
        assert!(
            TriangleMesh::from_stream(&make(source, data[..11].to_vec()), 1, doc.ctx()).is_err()
        );
        assert!(TriangleMesh::from_stream(&make(source, data.clone()), 2, doc.ctx()).is_err());
        assert!(TriangleMesh::from_stream(&make(source, data), 0, doc.ctx()).is_err());
    }
    #[test]
    fn decoded_mesh_charges_input_and_topology_together() {
        let data = [0, 0, 0, 0, 0, 255, 0, 255, 0, 0, 255, 0];
        let decode = [0.0, 4.0, 0.0, 4.0, 0.0, 1.0];
        let encoding = || MeshEncoding {
            flag_bits: 8,
            coordinate_bits: 8,
            component_bits: 8,
            components: 1,
            decode: &decode,
        };
        let limits = Limits::default();
        let mesh = TriangleMesh::decode(&data, encoding(), &limits).unwrap();
        let retained = mesh.retained_payload_bytes().unwrap();
        let exact = Limits {
            max_decoded_bytes: retained + data.len(),
            ..limits
        };
        assert!(TriangleMesh::decode(&data, encoding(), &exact).is_ok());
        let tight = Limits {
            max_decoded_bytes: exact.max_decoded_bytes - 1,
            ..limits
        };
        assert!(matches!(
            TriangleMesh::decode(&data, encoding(), &tight),
            Err(PdfError::ResourceLimit(_))
        ));
        let mut output = [-1.0];
        assert!(mesh.sample_triangle(0, [1.0, 1.0], &mut output).unwrap());
        assert_eq!(output, [0.25]);
        assert!(TriangleMesh::decode(&data, encoding(), &exact).is_ok());
    }
    #[test]
    fn owned_mesh_samples_shared_edges_and_rejects_invalid_vertices() {
        let vertices = vec![
            Vertex {
                flag: 0,
                point: [0.0, 0.0],
                components: vec![0.0],
            },
            Vertex {
                flag: 0,
                point: [4.0, 0.0],
                components: vec![1.0],
            },
            Vertex {
                flag: 0,
                point: [0.0, 4.0],
                components: vec![0.0],
            },
            Vertex {
                flag: 1,
                point: [4.0, 4.0],
                components: vec![1.0],
            },
        ];
        let limits = Limits::default();
        let mesh = TriangleMesh::try_new(vertices.clone(), &limits).unwrap();
        assert_eq!(mesh.triangle_count(), 2);
        assert!(mesh.bounds_contains([0.0, 0.0]));
        assert!(mesh.bounds_contains([4.0, 4.0]));
        assert!(!mesh.bounds_contains([-0.1, 2.0]));
        assert!(!mesh.bounds_contains([2.0, 4.1]));
        assert!(!mesh.bounds_contains([f64::NAN, 2.0]));
        let mut output = [-1.0];
        assert!(mesh.sample_triangle(0, [2.0, 2.0], &mut output).unwrap());
        assert_eq!(output, [0.5]);
        assert!(mesh.sample_triangle(1, [2.0, 2.0], &mut output).unwrap());
        assert_eq!(output, [0.5]);
        assert!(mesh.sample_triangle(1, [3.0, 3.0], &mut output).unwrap());
        assert_eq!(output, [0.75]);
        assert!(mesh.sample_triangle(2, [3.0, 3.0], &mut output).is_err());
        assert_eq!(output, [0.75]);
        let mut bad = vertices.clone();
        bad[3].components.push(0.0);
        assert!(TriangleMesh::try_new(bad, &limits).is_err());
        let mut bad = vertices;
        bad[0].point[0] = f64::INFINITY;
        assert!(TriangleMesh::try_new(bad, &limits).is_err());
    }
    #[test]
    fn triangle_interpolates_components_before_function_and_preserves_misses() {
        let vertices = [
            Vertex {
                flag: 0,
                point: [0.0, 0.0],
                components: vec![0.0, 10.0],
            },
            Vertex {
                flag: 0,
                point: [4.0, 0.0],
                components: vec![1.0, 20.0],
            },
            Vertex {
                flag: 0,
                point: [0.0, 4.0],
                components: vec![0.0, 30.0],
            },
        ];
        let references = [&vertices[0], &vertices[1], &vertices[2]];
        let mut output = [-1.0; 2];
        assert!(interpolate_triangle(references, [1.0, 1.0], &mut output).unwrap());
        assert_eq!(output, [0.25, 17.5]);
        // A nonlinear function must evaluate the interpolated parameter:
        // t^2 = 0.0625, not interpolated vertex function values = 0.25.
        assert_eq!(output[0] * output[0], 0.0625);
        assert!(!interpolate_triangle(references, [4.0, 4.0], &mut output).unwrap());
        assert_eq!(output, [0.25, 17.5]);
        let mut wrong = [7.0];
        assert!(interpolate_triangle(references, [1.0, 1.0], &mut wrong).is_err());
        assert_eq!(wrong, [7.0]);
    }
    #[test]
    fn triangle_weights_preserve_vertex_order_and_reject_zero_area() {
        let points = [[10.0, 20.0], [18.0, 20.0], [10.0, 24.0]];
        assert_eq!(barycentric(points, [12.0, 21.0]), Some([0.5, 0.25, 0.25]));
        assert_eq!(barycentric(points, points[0]), Some([1.0, 0.0, 0.0]));
        assert_eq!(barycentric(points, [14.0, 22.0]), Some([0.0, 0.5, 0.5]));
        assert_eq!(
            barycentric([points[0], points[2], points[1]], [12.0, 21.0]),
            Some([0.5, 0.25, 0.25])
        );
        assert_eq!(barycentric(points, [18.0, 24.0]), None);
        assert_eq!(
            barycentric([[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]], [1.0, 1.0]),
            None
        );
        assert_eq!(barycentric(points, [f64::NAN, 21.0]), None);
        // Coordinates decoded from finite f32 endpoints remain safe in f64,
        // including triangles much larger or smaller than the raster surface.
        for scale in [1e-30, 1e30] {
            let scaled = [[0.0, 0.0], [scale, 0.0], [0.0, scale]];
            assert_eq!(
                barycentric(scaled, [scale * 0.25, scale * 0.25]),
                Some([0.5, 0.25, 0.25])
            );
        }
    }
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
            ..limits
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
            ..limits
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
