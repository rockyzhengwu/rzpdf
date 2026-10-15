//! Certified parameter-space geometry for PDF Coons and tensor-product patches.
#[cfg(test)]
#[path = "shading_tensor_tests.rs"]
mod tensor_tests;

#[derive(Debug, Clone)]
pub(crate) struct CoonsPatch {
    // PDF stream order: corners are points 0, 3, 6, 9.
    points: [[f64; 2]; 12],
    // Exact-real decoded coordinates may differ from their rounded values.
    point_enclosures: Option<[[Interval; 2]; 12]>,
    encoded: Option<EncodedGeometry>,
    // Type7 supplies these controls independently of its boundary. PDF order
    // is (u,v)=(1/3,1/3),(1/3,2/3),(2/3,2/3),(2/3,1/3).
    tensor_interior: Option<([[f64; 2]; 4], [[Interval; 2]; 4])>,
}

#[derive(Debug, Clone, Copy)]
struct EncodedGeometry {
    raw: [[u32; 2]; 12],
    width: u8,
    decode: [f64; 4],
}

fn unit(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn cubic(points: [[f64; 2]; 4], t: f64) -> [f64; 2] {
    // de Casteljau interpolation also preserves endpoints exactly.
    let mix = |a: [f64; 2], b: [f64; 2]| [a[0] * (1.0 - t) + b[0] * t, a[1] * (1.0 - t) + b[1] * t];
    let a = mix(points[0], points[1]);
    let b = mix(points[1], points[2]);
    let c = mix(points[2], points[3]);
    mix(mix(a, b), mix(b, c))
}

type TensorControls = [[[f64; 2]; 4]; 4];

/// Parameter-space corner interpolation with uncertain roots/components.
/// RGB conversion/function mapping remains the next, separate operation.
pub(crate) fn interpolate_corner_bounds(
    corners: [&[Interval]; 4],
    parameters: [Interval; 2],
    output: &mut [Interval],
) -> Option<()> {
    if output.is_empty()
        || parameters
            .iter()
            .any(|v| !unit(v.lower) || !unit(v.upper) || v.lower > v.upper)
        || corners.iter().any(|c| {
            c.len() != output.len()
                || c.iter()
                    .any(|v| v.lower.is_nan() || v.upper.is_nan() || v.lower > v.upper)
        })
    {
        return None;
    }
    let [u, v] = parameters;
    let nu = Interval::exact(1.0).sub(u);
    let nv = Interval::exact(1.0).sub(v);
    let weights = [nu.mul(nv), nu.mul(v), u.mul(v), u.mul(nv)];
    for (index, value) in output.iter_mut().enumerate() {
        *value = corners
            .iter()
            .zip(weights)
            .fold(Interval::exact(0.0), |sum, (corner, weight)| {
                sum.add(corner[index].mul(weight))
            });
    }
    Some(())
}

#[derive(Debug, Clone)]
pub(crate) struct ColoredPatch {
    pub(crate) geometry: CoonsPatch,
    pub(crate) values: Vec<f32>,
    pub(crate) bounds: Vec<Interval>,
}

impl ColoredPatch {
    /// Sample encoded components/parameters, not RGB. Unresolved geometry or
    /// ordering is an error. Output remains untouched until root selection.
    pub(crate) fn sample(
        &self,
        point: [f64; 2],
        depth: u8,
        remaining: &mut usize,
        values: &mut [f32],
        bounds: &mut [Interval],
    ) -> Result<bool, SearchError> {
        let count = values.len();
        if depth > 20
            || !point.iter().all(|v| v.is_finite())
            || count == 0
            || bounds.len() != count
            || self.values.len() != count.checked_mul(4).ok_or(SearchError::InvalidInput)?
            || self.bounds.len() != self.values.len()
            || self.values.iter().any(|v| !v.is_finite())
            || self
                .bounds
                .iter()
                .any(|v| v.lower.is_nan() || v.upper.is_nan() || v.lower > v.upper)
        {
            return Err(SearchError::InvalidInput);
        }
        *remaining = remaining.checked_sub(1).ok_or(SearchError::WorkLimit)?;
        let parameters = match self.geometry.constant_coverage(point)? {
            ConstantCoverage::NoCoverage => return Ok(false),
            ConstantCoverage::Parameter(uv) => uv.map(Interval::exact),
            ConstantCoverage::NotConstant => {
                let controls = self.geometry.tensor_control_enclosures();
                if let Some(corner) = certify_unique_corner(controls, point, remaining)? {
                    corner
                } else if let Some(edge) = certify_unique_straight_edge(controls, point, remaining)?
                {
                    edge
                } else {
                    // Bounded root storage. Reaching the cap is a work-limit
                    // error, not a truncated set used to choose a winner.
                    let mut roots = [[Interval::exact(0.0); 2]; 64];
                    let mut used = 0;
                    visit_certified_roots(controls, point, depth, remaining, &mut |root| {
                        if used == roots.len() {
                            return Err(SearchError::WorkLimit);
                        }
                        roots[used] = root;
                        used += 1;
                        Ok(())
                    })?;
                    let Some(root) = self.geometry.preferred_root(&roots[..used], remaining)?
                    else {
                        return Ok(false);
                    };
                    root
                }
            }
        };
        if parameters
            .iter()
            .any(|v| !unit(v.lower) || !unit(v.upper) || v.lower > v.upper)
        {
            return Err(SearchError::Unresolved);
        }
        let colors = std::array::from_fn::<_, 4, _>(|i| &self.values[i * count..(i + 1) * count]);
        let intervals =
            std::array::from_fn::<_, 4, _>(|i| &self.bounds[i * count..(i + 1) * count]);
        let midpoint = parameters.map(|v| v.lower * 0.5 + v.upper * 0.5);
        // All dimensions, components and root intervals were preflighted.
        interpolate_corner_bounds(intervals, parameters, bounds)
            .ok_or(SearchError::InvalidInput)?;
        interpolate_corners(colors, midpoint[0], midpoint[1], values)
            .ok_or(SearchError::InvalidInput)?;
        Ok(true)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PatchMesh {
    pub(crate) patches: Vec<ColoredPatch>,
    pub(crate) components: usize,
}

impl PatchMesh {
    /// Retained visible heap storage, including unused Vec capacity. Inline
    /// geometry is already included in the patch allocation. This excludes
    /// temporary decoder storage, allocator metadata and operation leases.
    pub(crate) fn retained_payload_bytes(&self) -> Option<usize> {
        self.patches.iter().try_fold(
            self.patches
                .capacity()
                .checked_mul(std::mem::size_of::<ColoredPatch>())?,
            |total, patch| {
                total
                    .checked_add(
                        patch
                            .values
                            .capacity()
                            .checked_mul(std::mem::size_of::<f32>())?,
                    )?
                    .checked_add(
                        patch
                            .bounds
                            .capacity()
                            .checked_mul(std::mem::size_of::<Interval>())?,
                    )
            },
        )
    }

    /// Later patches paint over earlier patches. An unresolved later patch
    /// must not be skipped as if it were a proven miss.
    pub(crate) fn sample(
        &self,
        point: [f64; 2],
        depth: u8,
        remaining: &mut usize,
        values: &mut [f32],
        bounds: &mut [Interval],
    ) -> Result<bool, SearchError> {
        if self.components == 0
            || self.patches.is_empty()
            || values.len() != self.components
            || bounds.len() != self.components
            || depth > 20
            || !point.iter().all(|v| v.is_finite())
        {
            return Err(SearchError::InvalidInput);
        }
        for patch in self.patches.iter().rev() {
            if patch.sample(point, depth, remaining, values, bounds)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Decode a preflighted stream into owned geometry and corner data. The
/// byte allowance counts borrowed input length and visible Vec capacities,
/// not allocator metadata, stack frames or a full operation-wide lease.
pub(crate) fn decode_patch_stream(
    bytes: &[u8],
    encoding: &PatchEncoding<'_>,
    max_patches: usize,
    max_bytes: usize,
) -> Result<PatchMesh, PatchStreamError> {
    decode_patch_stream_kind(bytes, encoding, max_patches, max_bytes, false)
}

pub(crate) fn decode_patch_stream_kind(
    bytes: &[u8],
    encoding: &PatchEncoding<'_>,
    max_patches: usize,
    max_bytes: usize,
    tensor: bool,
) -> Result<PatchMesh, PatchStreamError> {
    let components = encoding.components.len();
    let plan = scan_patch_stream_kind(
        bytes,
        encoding.flag_bits,
        encoding.coordinate_bits,
        encoding.component_bits,
        components,
        max_patches,
        tensor,
    )?;
    if !encoding.coordinates.iter().all(|v| v.is_finite())
        || encoding.components.iter().flatten().any(|v| !v.is_finite())
    {
        return Err(PatchStreamError::InvalidDecode);
    }
    let values = components
        .checked_mul(4)
        .ok_or(PatchStreamError::MemoryLimit)?;
    let stored = values
        .checked_mul(std::mem::size_of::<f32>() + std::mem::size_of::<Interval>())
        .ok_or(PatchStreamError::MemoryLimit)?;
    let required = stored
        .checked_add(std::mem::size_of::<ColoredPatch>())
        .and_then(|v| v.checked_mul(plan.patches))
        .and_then(|v| {
            values
                .checked_mul(std::mem::size_of::<u32>())
                .and_then(|s| v.checked_add(s))
        })
        .and_then(|v| v.checked_add(bytes.len()))
        .ok_or(PatchStreamError::MemoryLimit)?;
    if required > max_bytes {
        return Err(PatchStreamError::MemoryLimit);
    }
    fn reserve<T>(
        count: usize,
        used: &mut usize,
        limit: usize,
    ) -> Result<Vec<T>, PatchStreamError> {
        let requested = count
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(PatchStreamError::MemoryLimit)?;
        if used.checked_add(requested).is_none_or(|n| n > limit) {
            return Err(PatchStreamError::MemoryLimit);
        }
        let mut buffer = Vec::new();
        buffer
            .try_reserve_exact(count)
            .map_err(|_| PatchStreamError::Allocation)?;
        let actual = buffer
            .capacity()
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(PatchStreamError::MemoryLimit)?;
        *used = used
            .checked_add(actual)
            .filter(|n| *n <= limit)
            .ok_or(PatchStreamError::MemoryLimit)?;
        Ok(buffer)
    }
    let mut used = bytes.len();
    let mut patches: Vec<ColoredPatch> = reserve(plan.patches, &mut used, max_bytes)?;
    let mut scratch: Vec<u32> = reserve(values, &mut used, max_bytes)?;
    scratch.resize(values, 0);
    let mut stream = PatchBits::new(bytes);
    for _ in 0..plan.patches {
        let flag = stream
            .peek_flag(encoding.flag_bits)
            .ok_or(PatchStreamError::Truncated)?;
        let mut values_buffer: Vec<f32> = reserve(values, &mut used, max_bytes)?;
        let mut bounds_buffer: Vec<Interval> = reserve(values, &mut used, max_bytes)?;
        values_buffer.resize(values, 0.0);
        bounds_buffer.resize(values, Interval::exact(0.0));
        let offset = if flag == 0 { 0 } else { components * 2 };
        let count = values - offset;
        let (_, geometry) = stream
            .decoded_record_kind(
                encoding,
                patches.last().map(|p| &p.geometry),
                &mut scratch[..count],
                &mut values_buffer[offset..],
                &mut bounds_buffer[offset..],
                tensor,
            )
            .ok_or(PatchStreamError::InvalidDecode)?;
        if flag != 0 {
            let previous = patches
                .last()
                .ok_or(PatchStreamError::MissingPreviousPatch)?;
            for corner in 0..2 {
                let old = (usize::from(flag) + corner) % 4;
                let old_range = old * components..(old + 1) * components;
                let new_range = corner * components..(corner + 1) * components;
                values_buffer[new_range.clone()]
                    .copy_from_slice(&previous.values[old_range.clone()]);
                bounds_buffer[new_range].copy_from_slice(&previous.bounds[old_range]);
            }
        }
        patches.push(ColoredPatch {
            geometry,
            values: values_buffer,
            bounds: bounds_buffer,
        });
    }
    if stream.position != plan.payload_bits {
        return Err(PatchStreamError::Truncated);
    }
    Ok(PatchMesh {
        patches,
        components,
    })
}

pub(crate) struct PatchEncoding<'a> {
    pub(crate) flag_bits: u8,
    pub(crate) coordinate_bits: u8,
    pub(crate) component_bits: u8,
    pub(crate) coordinates: [f64; 4],
    pub(crate) components: &'a [[f64; 2]],
}

#[derive(Debug, PartialEq)]
pub(crate) enum PatchStreamError {
    InvalidEncoding,
    MissingPreviousPatch,
    Truncated,
    Empty,
    PatchLimit,
    MemoryLimit,
    Allocation,
    InvalidDecode,
}

#[derive(Debug, PartialEq)]
pub(crate) struct PatchStreamPlan {
    pub(crate) patches: usize,
    pub(crate) payload_bits: usize,
    pub(crate) tail_bits: u8,
}

/// Structural preflight without allocations. Only the final partial byte
/// is padding; complete trailing bytes must form complete patch records.
pub(crate) fn scan_patch_stream(
    bytes: &[u8],
    flag_bits: u8,
    coordinate_bits: u8,
    component_bits: u8,
    components: usize,
    max_patches: usize,
) -> Result<PatchStreamPlan, PatchStreamError> {
    scan_patch_stream_kind(
        bytes,
        flag_bits,
        coordinate_bits,
        component_bits,
        components,
        max_patches,
        false,
    )
}

pub(crate) fn scan_patch_stream_kind(
    bytes: &[u8],
    flag_bits: u8,
    coordinate_bits: u8,
    component_bits: u8,
    components: usize,
    max_patches: usize,
    tensor: bool,
) -> Result<PatchStreamPlan, PatchStreamError> {
    patch_record_layout_kind(
        0,
        flag_bits,
        coordinate_bits,
        component_bits,
        components,
        tensor,
    )
    .ok_or(PatchStreamError::InvalidEncoding)?;
    let mut cursor = PatchBits::new(bytes);
    let mut patches = 0_usize;
    let mut tail_bits = 0;
    loop {
        let remaining = cursor
            .remaining()
            .ok_or(PatchStreamError::InvalidEncoding)?;
        if remaining == 0 {
            break;
        }
        if remaining < 8 && patches > 0 {
            tail_bits = remaining as u8;
            break;
        }
        let start = cursor.position;
        let flag = cursor.flag(flag_bits).ok_or(PatchStreamError::Truncated)?;
        if patches == 0 && flag != 0 {
            return Err(PatchStreamError::MissingPreviousPatch);
        }
        let layout = patch_record_layout_kind(
            flag,
            flag_bits,
            coordinate_bits,
            component_bits,
            components,
            tensor,
        )
        .ok_or(PatchStreamError::InvalidEncoding)?;
        if remaining < layout.bits {
            return Err(PatchStreamError::Truncated);
        }
        patches = patches.checked_add(1).ok_or(PatchStreamError::PatchLimit)?;
        if patches > max_patches {
            return Err(PatchStreamError::PatchLimit);
        }
        cursor.position = start
            .checked_add(layout.bits)
            .ok_or(PatchStreamError::InvalidEncoding)?;
    }
    if patches == 0 {
        return Err(PatchStreamError::Empty);
    }
    Ok(PatchStreamPlan {
        patches,
        payload_bits: cursor.position,
        tail_bits,
    })
}

/// Decode the newly encoded corner components, with caller-owned buffers.
/// Preflight all values before writing; no hidden allocation or RGB mapping.
pub(crate) fn decode_patch_components(
    raw: &[u32],
    width: u8,
    decode: &[[f64; 2]],
    corners: usize,
    values: &mut [f32],
    bounds: &mut [Interval],
) -> Option<()> {
    let components = decode.len();
    let count = corners.checked_mul(components)?;
    if components == 0
        || ![2, 4].contains(&corners)
        || raw.len() != count
        || values.len() != count
        || bounds.len() != count
    {
        return None;
    }
    for (index, &raw) in raw.iter().enumerate() {
        let (value, _) = decode_patch_scalar(raw, width, decode[index % components])?;
        if !(value as f32).is_finite() {
            return None;
        }
    }
    for (index, &raw) in raw.iter().enumerate() {
        let (value, mut enclosure) = decode_patch_scalar(raw, width, decode[index % components])?;
        let rounded = value as f32;
        // Retain both exact decoded-real and the engine's stored-f32 value.
        enclosure.lower = enclosure.lower.min(f64::from(rounded));
        enclosure.upper = enclosure.upper.max(f64::from(rounded));
        values[index] = rounded;
        bounds[index] = enclosure;
    }
    Some(())
}

/// Decode one integer with a rounded value and an exact-real enclosure.
/// Weighted endpoint interpolation avoids overflowing (max-min).
pub(crate) fn decode_patch_scalar(
    raw: u32,
    width: u8,
    decode: [f64; 2],
) -> Option<(f64, Interval)> {
    if width == 0 || width > 32 || !decode.iter().all(|v| v.is_finite()) {
        return None;
    }
    let maximum = (1_u64 << width) - 1;
    if u64::from(raw) > maximum {
        return None;
    }
    if raw == 0 {
        return Some((decode[0], Interval::exact(decode[0])));
    }
    if u64::from(raw) == maximum {
        return Some((decode[1], Interval::exact(decode[1])));
    }
    // These affine maps simplify exactly over integers <= 2^32-1.
    // Avoid introducing division-rounding uncertainty for an identity
    // (or reversed identity) before constructing boundary witnesses.
    let integer = if decode == [0.0, maximum as f64] {
        Some(u64::from(raw))
    } else if decode == [maximum as f64, 0.0] {
        Some(maximum - u64::from(raw))
    } else {
        None
    };
    if let Some(integer) = integer {
        let value = integer as f64;
        return Some((value, Interval::exact(value)));
    }
    let t = f64::from(raw) / maximum as f64;
    let value = decode[0] * (1.0 - t) + decode[1] * t;
    if !value.is_finite() {
        return None;
    }
    let fraction = Interval::enclosing(t, t);
    let enclosure = Interval::exact(decode[0])
        .mul(Interval::exact(1.0).sub(fraction))
        .add(Interval::exact(decode[1]).mul(fraction));
    Some((value, enclosure))
}

/// Continuous MSB-first patch bitstream; no implicit per-record alignment.
#[derive(Clone, Copy)]
pub(crate) struct PatchBits<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> PatchBits<'a> {
    /// Assemble one record without hidden heap storage. `scratch` may be
    /// overwritten on failure; decoded outputs and stream position are atomic.
    pub(crate) fn decoded_record(
        &mut self,
        encoding: &PatchEncoding<'_>,
        previous: Option<&CoonsPatch>,
        scratch: &mut [u32],
        values: &mut [f32],
        bounds: &mut [Interval],
    ) -> Option<(u8, CoonsPatch)> {
        self.decoded_record_kind(encoding, previous, scratch, values, bounds, false)
    }

    fn decoded_record_kind(
        &mut self,
        encoding: &PatchEncoding<'_>,
        previous: Option<&CoonsPatch>,
        scratch: &mut [u32],
        values: &mut [f32],
        bounds: &mut [Interval],
        tensor: bool,
    ) -> Option<(u8, CoonsPatch)> {
        let flag = self.peek_flag(encoding.flag_bits)?;
        let layout = patch_record_layout_kind(
            flag,
            encoding.flag_bits,
            encoding.coordinate_bits,
            encoding.component_bits,
            encoding.components.len(),
            tensor,
        )?;
        if flag != 0 && previous.is_none() {
            return None;
        }
        if !encoding.coordinates.iter().all(|v| v.is_finite()) {
            return None;
        }
        let mut cursor = *self;
        let mut raw_points = [[0; 2]; 16];
        cursor.record_kind(
            encoding.flag_bits,
            encoding.coordinate_bits,
            encoding.component_bits,
            encoding.components.len(),
            &mut raw_points[..layout.points],
            scratch,
            tensor,
        )?;
        let patch = if tensor {
            CoonsPatch::from_tensor_record(
                flag,
                &raw_points[..layout.points],
                previous,
                encoding.coordinate_bits,
                encoding.coordinates,
            )?
        } else if flag == 0 {
            CoonsPatch::from_decoded_points(
                std::array::from_fn(|i| raw_points[i]),
                encoding.coordinate_bits,
                encoding.coordinates,
            )?
        } else {
            previous?.continue_decoded(
                flag,
                std::array::from_fn(|i| raw_points[i]),
                encoding.coordinate_bits,
                encoding.coordinates,
            )?
        };
        decode_patch_components(
            scratch,
            encoding.component_bits,
            encoding.components,
            layout.corners,
            values,
            bounds,
        )?;
        self.position = cursor.position;
        Some((flag, patch))
    }

    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }
    pub(crate) fn remaining(&self) -> Option<usize> {
        self.bytes.len().checked_mul(8)?.checked_sub(self.position)
    }
    /// Failed reads do not consume input.
    pub(crate) fn read(&mut self, width: u8) -> Option<u32> {
        if width == 0 || width > 32 {
            return None;
        }
        let end = self.position.checked_add(usize::from(width))?;
        if end > self.bytes.len().checked_mul(8)? {
            return None;
        }
        let mut value = 0_u32;
        for bit in self.position..end {
            value = (value << 1) | u32::from((self.bytes[bit / 8] >> (7 - bit % 8)) & 1);
        }
        self.position = end;
        Some(value)
    }
    pub(crate) fn flag(&mut self, width: u8) -> Option<u8> {
        if ![2, 4, 8].contains(&width) {
            return None;
        }
        Some((self.read(width)? & 3) as u8)
    }

    pub(crate) fn peek_flag(&self, width: u8) -> Option<u8> {
        let mut cursor = *self;
        cursor.flag(width)
    }

    /// Read raw integers into caller storage, geometry first then colors.
    /// Whole-record preflight precedes any buffer write or cursor change.
    pub(crate) fn record(
        &mut self,
        flag_bits: u8,
        coordinate_bits: u8,
        component_bits: u8,
        components: usize,
        points: &mut [[u32; 2]],
        colors: &mut [u32],
    ) -> Option<u8> {
        self.record_kind(
            flag_bits,
            coordinate_bits,
            component_bits,
            components,
            points,
            colors,
            false,
        )
    }

    fn record_kind(
        &mut self,
        flag_bits: u8,
        coordinate_bits: u8,
        component_bits: u8,
        components: usize,
        points: &mut [[u32; 2]],
        colors: &mut [u32],
        tensor: bool,
    ) -> Option<u8> {
        let mut cursor = *self;
        let flag = cursor.flag(flag_bits)?;
        let layout = patch_record_layout_kind(
            flag,
            flag_bits,
            coordinate_bits,
            component_bits,
            components,
            tensor,
        )?;
        if points.len() != layout.points
            || colors.len() != layout.corners.checked_mul(components)?
            || self.remaining()? < layout.bits
        {
            return None;
        }
        for point in points {
            point[0] = cursor.read(coordinate_bits)?;
            point[1] = cursor.read(coordinate_bits)?;
        }
        for color in colors {
            *color = cursor.read(component_bits)?;
        }
        self.position = cursor.position;
        Some(flag)
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct PatchRecordLayout {
    pub(crate) points: usize,
    pub(crate) corners: usize,
    pub(crate) bits: usize,
}

/// Bit payload only; deliberately does not assume a byte-padding policy.
/// The supplied flag is already decoded, not an unmasked raw flag field.
pub(crate) fn patch_record_layout(
    flag: u8,
    flag_bits: u8,
    coordinate_bits: u8,
    component_bits: u8,
    components: usize,
) -> Option<PatchRecordLayout> {
    patch_record_layout_kind(
        flag,
        flag_bits,
        coordinate_bits,
        component_bits,
        components,
        false,
    )
}

pub(crate) fn patch_record_layout_kind(
    flag: u8,
    flag_bits: u8,
    coordinate_bits: u8,
    component_bits: u8,
    components: usize,
    tensor: bool,
) -> Option<PatchRecordLayout> {
    if flag > 3
        || ![2, 4, 8].contains(&flag_bits)
        || ![1, 2, 4, 8, 12, 16, 24, 32].contains(&coordinate_bits)
        || ![1, 2, 4, 8, 12, 16].contains(&component_bits)
        || components == 0
    {
        return None;
    }
    let points: usize = if flag == 0 {
        if tensor { 16 } else { 12 }
    } else {
        if tensor { 12 } else { 8 }
    };
    let corners: usize = if flag == 0 { 4 } else { 2 };
    let coordinates = points
        .checked_mul(2)?
        .checked_mul(usize::from(coordinate_bits))?;
    let colors = corners
        .checked_mul(components)?
        .checked_mul(usize::from(component_bits))?;
    let bits = usize::from(flag_bits)
        .checked_add(coordinates)?
        .checked_add(colors)?;
    Some(PatchRecordLayout {
        points,
        corners,
        bits,
    })
}
type IntervalControls = [[[Interval; 2]; 4]; 4];

/// Contract a uniquely certified local root while retaining its proof.
/// Lack of further contraction preserves the earlier enclosure; exhausted
/// work allowance is an error, never a silently returned partial refinement.
pub(crate) fn tighten_unique_cell(
    controls: &IntervalControls,
    target: [f64; 2],
    rounds: u8,
    remaining: &mut usize,
) -> Result<CellCertificate, SearchError> {
    if rounds > 32 {
        return Err(SearchError::InvalidInput);
    }
    *remaining = remaining.checked_sub(1).ok_or(SearchError::WorkLimit)?;
    let mut root = match certify_interval_cell(controls, target)? {
        CellCertificate::Unique(root) => root,
        other => return Ok(other),
    };
    for _ in 0..rounds {
        *remaining = remaining.checked_sub(1).ok_or(SearchError::WorkLimit)?;
        if root.iter().any(|v| v.lower >= v.upper) {
            break;
        }
        let rect = [root[0].lower, root[1].lower, root[0].upper, root[1].upper];
        let restricted =
            restrict_interval_tensor(*controls, rect).ok_or(SearchError::InvalidInput)?;
        // Every actual root in the current rectangle is in its Krawczyk
        // image, even when that image fails a fresh existence certificate.
        let Some((local, _)) = krawczyk_image(&restricted, target)? else {
            break;
        };
        let next: [Interval; 2] = std::array::from_fn(|axis| {
            let mapped = Interval::exact(rect[axis]).add(
                Interval::exact(rect[axis + 2])
                    .sub(Interval::exact(rect[axis]))
                    .mul(local[axis]),
            );
            Interval {
                lower: root[axis].lower.max(mapped.lower),
                upper: root[axis].upper.min(mapped.upper),
            }
        });
        if next.iter().any(|v| v.lower > v.upper) {
            return Err(SearchError::Unresolved);
        }
        let unchanged = (0..2).all(|axis| {
            root[axis].lower == next[axis].lower && root[axis].upper == next[axis].upper
        });
        root = next;
        if unchanged {
            break;
        }
    }
    Ok(CellCertificate::Unique(root))
}

/// Bernstein restriction via the cubic blossom: control i evaluates at
/// (3-i) copies of the lower endpoint and i copies of the upper endpoint.
/// Enclosures retain conversion and restriction-rounding uncertainty.
pub(crate) fn restrict_interval_tensor(
    controls: IntervalControls,
    rect: [f64; 4],
) -> Option<IntervalControls> {
    if !rect.iter().all(|v| unit(*v)) || rect[0] > rect[2] || rect[1] > rect[3] {
        return None;
    }
    reparameterize_interval_tensor(controls, rect)
}

/// Polynomial continuation to a finite parameter rectangle, including
/// outside the original square. This is geometry only: roots in the
/// continued surface are not evidence of coverage in the PDF domain.
fn reparameterize_interval_tensor(
    controls: IntervalControls,
    rect: [f64; 4],
) -> Option<IntervalControls> {
    if !rect.iter().all(|v| v.is_finite()) || rect[0] > rect[2] || rect[1] > rect[3] {
        return None;
    }
    interval_tensor_bounds(&controls)?;
    fn restrict(p: [[Interval; 2]; 4], a: f64, b: f64) -> [[Interval; 2]; 4] {
        std::array::from_fn(|index| {
            let mut points = p;
            for level in 0..3 {
                let t = if level < 3 - index { a } else { b };
                if t == 0.0 {
                    continue;
                }
                if t == 1.0 {
                    for i in 0..3 - level {
                        points[i] = points[i + 1];
                    }
                    continue;
                }
                let t = Interval::exact(t);
                let complement = Interval::exact(1.0).sub(t);
                for i in 0..3 - level {
                    points[i] = std::array::from_fn(|axis| {
                        points[i][axis]
                            .mul(complement)
                            .add(points[i + 1][axis].mul(t))
                    });
                }
            }
            points[0]
        })
    }
    let rows = controls.map(|row| restrict(row, rect[0], rect[2]));
    let mut result = controls;
    for i in 0..4 {
        let column = std::array::from_fn(|j| rows[j][i]);
        let restricted = restrict(column, rect[1], rect[3]);
        for j in 0..4 {
            result[j][i] = restricted[j];
        }
    }
    interval_tensor_bounds(&result)?;
    Some(result)
}

/// An exact interval evaluation supplies an existence witness, not a
/// uniqueness or complete-coverage certificate. Callers must still prove
/// that no other root changes PDF precedence.
fn exact_parameter_witness(
    controls: &IntervalControls,
    target: [f64; 2],
    parameters: [f64; 2],
) -> Result<bool, SearchError> {
    if !target.iter().all(|v| v.is_finite()) || !parameters.iter().all(|v| unit(*v)) {
        return Err(SearchError::InvalidInput);
    }
    let value = interval_tensor_box(controls, parameters.map(Interval::exact))
        .ok_or(SearchError::InvalidInput)?;
    Ok((0..2).all(|axis| value[axis].lower == target[axis] && value[axis].upper == target[axis]))
}

/// Certify a corner only when a continuation covering the entire original
/// domain has exactly one root. Failure to certify is not a coverage miss.
fn certify_unique_corner(
    controls: IntervalControls,
    target: [f64; 2],
    remaining: &mut usize,
) -> Result<Option<[Interval; 2]>, SearchError> {
    *remaining = remaining.checked_sub(1).ok_or(SearchError::WorkLimit)?;
    let mut witness = None;
    for uv in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] {
        *remaining = remaining.checked_sub(1).ok_or(SearchError::WorkLimit)?;
        if exact_parameter_witness(&controls, target, uv)? {
            witness = Some(uv);
            break;
        }
    }
    let Some(uv) = witness else { return Ok(None) };
    *remaining = remaining.checked_sub(1).ok_or(SearchError::WorkLimit)?;
    // This rectangle strictly contains the whole PDF parameter square.
    // A unique root here excludes every competing original-domain root.
    let continued = reparameterize_interval_tensor(controls, [-0.25, -0.25, 1.25, 1.25])
        .ok_or(SearchError::InvalidInput)?;
    if matches!(
        certify_interval_cell(&continued, target)?,
        CellCertificate::Unique(_)
    ) {
        Ok(Some(uv.map(Interval::exact)))
    } else {
        Ok(None)
    }
}

/// Constant-coordinate edges have a one-dimensional existence proof:
/// the other coordinate brackets the target at its endpoints. A global
/// continuation certificate is still required to exclude competing roots.
fn certify_unique_straight_edge(
    controls: IntervalControls,
    target: [f64; 2],
    remaining: &mut usize,
) -> Result<Option<[Interval; 2]>, SearchError> {
    if !target.iter().all(|v| v.is_finite()) || interval_tensor_bounds(&controls).is_none() {
        return Err(SearchError::InvalidInput);
    }
    let edges = [
        (controls[0], 1, 0.0),
        (controls[3], 1, 1.0),
        (std::array::from_fn(|j| controls[j][0]), 0, 0.0),
        (std::array::from_fn(|j| controls[j][3]), 0, 1.0),
    ];
    for (edge, fixed_parameter, endpoint) in edges {
        *remaining = remaining.checked_sub(1).ok_or(SearchError::WorkLimit)?;
        for axis in 0..2 {
            if !edge
                .iter()
                .all(|p| p[axis].lower == target[axis] && p[axis].upper == target[axis])
            {
                continue;
            }
            let other = 1 - axis;
            let a = edge[0][other];
            let b = edge[3][other];
            if !((a.upper < target[other] && b.lower > target[other])
                || (b.upper < target[other] && a.lower > target[other]))
            {
                continue;
            }
            *remaining = remaining.checked_sub(1).ok_or(SearchError::WorkLimit)?;
            let continued = reparameterize_interval_tensor(controls, [-0.25, -0.25, 1.25, 1.25])
                .ok_or(SearchError::InvalidInput)?;
            let CellCertificate::Unique(local) =
                tighten_unique_cell(&continued, target, 8, remaining)?
            else {
                return Ok(None);
            };
            let mut root = local.map(|v| Interval::exact(-0.25).add(Interval::exact(1.5).mul(v)));
            // IVT proves an original-domain edge root. Global uniqueness
            // identifies it with the certified root, so intersecting with
            // the original domain and exact edge parameter preserves proof.
            let variable = 1 - fixed_parameter;
            root[variable].lower = root[variable].lower.max(0.0);
            root[variable].upper = root[variable].upper.min(1.0);
            if root[variable].lower > root[variable].upper
                || root[fixed_parameter].lower > endpoint
                || root[fixed_parameter].upper < endpoint
            {
                return Err(SearchError::Unresolved);
            }
            root[fixed_parameter] = Interval::exact(endpoint);
            return Ok(Some(root));
        }
    }
    Ok(None)
}

/// Select only when the PDF v/u precedence is proven by root enclosures.
/// Overlapping v intervals cannot be epsilon-tied or midpoint-sorted.
pub(crate) fn preferred_certified_root(
    roots: &[[Interval; 2]],
    remaining: &mut usize,
) -> Result<Option<[Interval; 2]>, SearchError> {
    preferred_certified_root_impl(roots, remaining, false)
}

fn preferred_certified_root_impl(
    roots: &[[Interval; 2]],
    remaining: &mut usize,
    common_v: bool,
) -> Result<Option<[Interval; 2]>, SearchError> {
    if roots
        .iter()
        .flatten()
        .any(|v| !unit(v.lower) || !unit(v.upper) || v.lower > v.upper)
    {
        return Err(SearchError::InvalidInput);
    }
    if roots.is_empty() {
        return Ok(None);
    }
    for (index, &candidate) in roots.iter().enumerate() {
        let mut dominates = true;
        for (other_index, &other) in roots.iter().enumerate() {
            if index == other_index {
                continue;
            }
            *remaining = remaining.checked_sub(1).ok_or(SearchError::WorkLimit)?;
            let v_greater = candidate[1].lower > other[1].upper;
            let v_equal = common_v
                || (candidate[1].lower == candidate[1].upper
                    && other[1].lower == other[1].upper
                    && candidate[1].lower == other[1].lower);
            if !v_greater && !(v_equal && candidate[0].lower >= other[0].upper) {
                dominates = false;
                break;
            }
        }
        if dominates {
            return Ok(Some(candidate));
        }
    }
    Err(SearchError::Unresolved)
}

/// Contract the entire candidate domain before subdividing it. A Krawczyk
/// image contains every root even without an existence certificate; each
/// intersection therefore preserves completeness. Only a subsequent strict
/// certificate supplies existence and uniqueness, never the contraction alone.
fn contract_candidate_cell(
    controls: IntervalControls,
    target: [f64; 2],
    remaining: &mut usize,
) -> Result<CellCertificate, SearchError> {
    let mut domain = [Interval {
        lower: 0.0,
        upper: 1.0,
    }; 2];
    contract_candidate_domain(controls, target, &mut domain, remaining)
}

// Also return the surviving all-root domain when no unique certificate is
// possible, so traversal does not discard useful contraction on a folded net.
fn contract_candidate_domain(
    controls: IntervalControls,
    target: [f64; 2],
    domain: &mut [Interval; 2],
    remaining: &mut usize,
) -> Result<CellCertificate, SearchError> {
    let mut first_survivors = None;
    for _ in 0..8 {
        *remaining = remaining.checked_sub(1).ok_or(SearchError::WorkLimit)?;
        let rect = [
            domain[0].lower,
            domain[1].lower,
            domain[0].upper,
            domain[1].upper,
        ];
        let restricted =
            restrict_interval_tensor(controls, rect).ok_or(SearchError::InvalidInput)?;
        let Some((image, _)) = krawczyk_image(&restricted, target)? else {
            if let Some(first) = first_survivors {
                *domain = first;
            }
            return Ok(CellCertificate::Unresolved);
        };
        let next: [Interval; 2] = std::array::from_fn(|axis| {
            // Keep room for strict inclusion despite control/rounding
            // uncertainty. Enlarging the image preserves all-root coverage;
            // this margin is not an existence tolerance.
            let padded = Interval {
                lower: image[axis].sub(Interval::exact(0.0625)).lower,
                upper: image[axis].add(Interval::exact(0.0625)).upper,
            };
            let mapped = Interval::exact(rect[axis]).add(
                Interval::exact(rect[axis + 2])
                    .sub(Interval::exact(rect[axis]))
                    .mul(padded),
            );
            Interval {
                lower: domain[axis].lower.max(mapped.lower),
                upper: domain[axis].upper.min(mapped.upper),
            }
        });
        if next.iter().any(|v| v.lower > v.upper) {
            return Ok(CellCertificate::NoRoot);
        }
        if (0..2).all(|axis| {
            next[axis].lower == domain[axis].lower && next[axis].upper == domain[axis].upper
        }) {
            if let Some(first) = first_survivors {
                *domain = first;
            }
            return Ok(CellCertificate::Unresolved);
        }
        first_survivors.get_or_insert(next);
        *domain = next;
        let rect = [
            domain[0].lower,
            domain[1].lower,
            domain[0].upper,
            domain[1].upper,
        ];
        let restricted =
            restrict_interval_tensor(controls, rect).ok_or(SearchError::InvalidInput)?;
        match tighten_unique_cell(&restricted, target, 8, remaining)? {
            CellCertificate::Unique(root) => {
                let mapped = std::array::from_fn(|axis| {
                    let value = Interval::exact(rect[axis]).add(
                        Interval::exact(rect[axis + 2])
                            .sub(Interval::exact(rect[axis]))
                            .mul(root[axis]),
                    );
                    Interval {
                        lower: domain[axis].lower.max(value.lower),
                        upper: domain[axis].upper.min(value.upper),
                    }
                });
                return Ok(CellCertificate::Unique(mapped));
            }
            CellCertificate::NoRoot => return Ok(CellCertificate::NoRoot),
            CellCertificate::Unresolved => {}
        }
    }
    if let Some(first) = first_survivors {
        *domain = first;
    }
    Ok(CellCertificate::Unresolved)
}

/// Certify every visited region or fail explicitly. Callback roots are
/// provisional until the complete traversal succeeds, including all siblings.
pub(crate) fn visit_certified_roots(
    controls: IntervalControls,
    target: [f64; 2],
    depth: u8,
    remaining: &mut usize,
    visit: &mut impl FnMut([Interval; 2]) -> Result<(), SearchError>,
) -> Result<(), SearchError> {
    if depth > 20
        || !target.iter().all(|x| x.is_finite())
        || interval_tensor_bounds(&controls).is_none()
    {
        return Err(SearchError::InvalidInput);
    }
    fn walk(
        controls: IntervalControls,
        rect: [f64; 4],
        mapping: [[Interval; 2]; 2],
        target: [f64; 2],
        depth: u8,
        remaining: &mut usize,
        visit: &mut impl FnMut([Interval; 2]) -> Result<(), SearchError>,
    ) -> Result<(), SearchError> {
        *remaining = remaining.checked_sub(1).ok_or(SearchError::WorkLimit)?;
        let hull = interval_tensor_bounds(&controls).ok_or(SearchError::InvalidInput)?;
        if (0..2).any(|axis| target[axis] < hull[axis].lower || target[axis] > hull[axis].upper) {
            return Ok(());
        }
        let mut surviving = [Interval {
            lower: 0.0,
            upper: 1.0,
        }; 2];
        let certificate = match tighten_unique_cell(&controls, target, 8, remaining)? {
            CellCertificate::Unresolved => {
                contract_candidate_domain(controls, target, &mut surviving, remaining)?
            }
            other => other,
        };
        match certificate {
            CellCertificate::NoRoot => return Ok(()),
            CellCertificate::Unique(root) => {
                let global = std::array::from_fn(|axis| {
                    let value = mapping[axis][0].add(mapping[axis][1].mul(root[axis]));
                    Interval {
                        lower: value.lower.max(0.0),
                        upper: value.upper.min(1.0),
                    }
                });
                return visit(global);
            }
            CellCertificate::Unresolved => {}
        }
        if depth == 0 {
            return Err(SearchError::Unresolved);
        }
        if surviving.iter().any(|v| v.lower > 0.0 || v.upper < 1.0) {
            // Split only the wider surviving parameter interval. Narrow
            // axes retain their interior roots instead of creating a new
            // boundary through every root. Both children cover all survivors.
            let axis = usize::from(
                surviving[1].upper - surviving[1].lower > surviving[0].upper - surviving[0].lower,
            );
            let fraction = if rect == [0.0, 0.0, 1.0, 1.0] {
                0.375
            } else {
                0.5
            };
            let mid =
                surviving[axis].lower + (surviving[axis].upper - surviving[axis].lower) * fraction;
            if mid <= surviving[axis].lower || mid >= surviving[axis].upper {
                return Err(SearchError::Unresolved);
            }
            for side in 0..2 {
                let mut local = surviving;
                if side == 0 {
                    local[axis].upper = mid;
                } else {
                    local[axis].lower = mid;
                }
                let domain = [
                    local[0].lower,
                    local[1].lower,
                    local[0].upper,
                    local[1].upper,
                ];
                let child =
                    restrict_interval_tensor(controls, domain).ok_or(SearchError::InvalidInput)?;
                // Keep uncertain affine coefficients, not only rounded
                // endpoints: a narrow root must retain mapping uncertainty.
                let child_mapping = std::array::from_fn(|dimension| {
                    [
                        mapping[dimension][0].add(
                            mapping[dimension][1].mul(Interval::exact(local[dimension].lower)),
                        ),
                        mapping[dimension][1].mul(
                            Interval::exact(local[dimension].upper)
                                .sub(Interval::exact(local[dimension].lower)),
                        ),
                    ]
                });
                let global: [Interval; 2] = std::array::from_fn(|dimension| {
                    let mapped = Interval::exact(rect[dimension]).add(
                        Interval::exact(rect[dimension + 2])
                            .sub(Interval::exact(rect[dimension]))
                            .mul(local[dimension]),
                    );
                    Interval {
                        lower: rect[dimension].max(mapped.lower),
                        upper: rect[dimension + 2].min(mapped.upper),
                    }
                });
                walk(
                    child,
                    [
                        global[0].lower,
                        global[1].lower,
                        global[0].upper,
                        global[1].upper,
                    ],
                    child_mapping,
                    target,
                    depth - 1,
                    remaining,
                    visit,
                )?;
            }
            return Ok(());
        }
        // Offset only the first split; subsequent midpoint splits remain
        // exactly dyadic through the depth limit. This avoids repeatedly
        // placing the common u/v=.5 roots on every subdivision boundary.
        let first = rect == [0.0, 0.0, 1.0, 1.0];
        let mid = if first {
            [0.375, 0.375]
        } else {
            [(rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5]
        };
        let children = if first {
            let domains = [
                [0.0, 0.0, 0.375, 0.375],
                [0.375, 0.0, 1.0, 0.375],
                [0.0, 0.375, 0.375, 1.0],
                [0.375, 0.375, 1.0, 1.0],
            ];
            let mut children = [controls; 4];
            for (child, domain) in children.iter_mut().zip(domains) {
                *child =
                    restrict_interval_tensor(controls, domain).ok_or(SearchError::InvalidInput)?;
            }
            children
        } else {
            subdivide_interval_tensor(controls).ok_or(SearchError::InvalidInput)?
        };
        for (index, child) in children.into_iter().enumerate() {
            let x = index % 2;
            let y = index / 2;
            let next = [
                if x == 0 { rect[0] } else { mid[0] },
                if y == 0 { rect[1] } else { mid[1] },
                if x == 0 { mid[0] } else { rect[2] },
                if y == 0 { mid[1] } else { rect[3] },
            ];
            let child_mapping = std::array::from_fn(|axis| {
                let side = if axis == 0 { x } else { y };
                let split = if first { 0.375 } else { 0.5 };
                let start = if side == 0 { 0.0 } else { split };
                let width = if side == 0 { split } else { 1.0 - split };
                [
                    mapping[axis][0].add(mapping[axis][1].mul(Interval::exact(start))),
                    mapping[axis][1].mul(Interval::exact(width)),
                ]
            });
            walk(
                child,
                next,
                child_mapping,
                target,
                depth - 1,
                remaining,
                visit,
            )?;
        }
        Ok(())
    }
    walk(
        controls,
        [0.0, 0.0, 1.0, 1.0],
        [[Interval::exact(0.0), Interval::exact(1.0)]; 2],
        target,
        depth,
        remaining,
        visit,
    )
}

#[derive(Debug)]
pub(crate) enum CellCertificate {
    NoRoot,
    Unique([Interval; 2]),
    Unresolved,
}

#[derive(Debug, PartialEq)]
pub(crate) enum ConstantCoverage {
    NotConstant,
    NoCoverage,
    Parameter([f64; 2]),
}

/// Krawczyk enclosure on the local unit square. Strict inclusion plus an
/// outward-rounded contraction bound certifies existence and uniqueness.
/// A failed certificate is not evidence of absent roots.
pub(crate) fn certify_interval_cell(
    controls: &IntervalControls,
    target: [f64; 2],
) -> Result<CellCertificate, SearchError> {
    let Some((image, contraction)) = krawczyk_image(controls, target)? else {
        return Ok(CellCertificate::Unresolved);
    };
    if image.iter().any(|v| v.upper < 0.0 || v.lower > 1.0) {
        return Ok(CellCertificate::NoRoot);
    }
    if contraction && image.iter().all(|v| v.lower > 0.0 && v.upper < 1.0) {
        Ok(CellCertificate::Unique(image))
    } else {
        Ok(CellCertificate::Unresolved)
    }
}

fn krawczyk_image(
    controls: &IntervalControls,
    target: [f64; 2],
) -> Result<Option<([Interval; 2], bool)>, SearchError> {
    if !target.iter().all(|x| x.is_finite()) {
        return Err(SearchError::InvalidInput);
    }
    let derivative = interval_tensor_derivatives(controls).ok_or(SearchError::InvalidInput)?;
    if derivative
        .iter()
        .flatten()
        .any(|v| !v.lower.is_finite() || !v.upper.is_finite())
    {
        return Ok(None);
    }
    let midpoint = derivative.map(|column| column.map(|v| v.lower * 0.5 + v.upper * 0.5));
    let det = midpoint[0][0] * midpoint[1][1] - midpoint[1][0] * midpoint[0][1];
    if !det.is_finite() || det == 0.0 {
        return Ok(None);
    }
    let inverse = [
        [midpoint[1][1] / det, -midpoint[1][0] / det],
        [-midpoint[0][1] / det, midpoint[0][0] / det],
    ];
    if inverse.iter().flatten().any(|v| !v.is_finite()) {
        return Ok(None);
    }
    let center = interval_tensor_point(controls, 0.5, 0.5).ok_or(SearchError::InvalidInput)?;
    let residual =
        std::array::from_fn::<_, 2, _>(|axis| center[axis].sub(Interval::exact(target[axis])));
    let matrix: [[Interval; 2]; 2] = std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            Interval::exact(if row == column { 1.0 } else { 0.0 }).sub(
                Interval::exact(inverse[row][0])
                    .mul(derivative[column][0])
                    .add(Interval::exact(inverse[row][1]).mul(derivative[column][1])),
            )
        })
    });
    let radius = Interval {
        lower: -0.5,
        upper: 0.5,
    };
    let image: [Interval; 2] = std::array::from_fn(|row| {
        Interval::exact(0.5)
            .sub(
                Interval::exact(inverse[row][0])
                    .mul(residual[0])
                    .add(Interval::exact(inverse[row][1]).mul(residual[1])),
            )
            .add(matrix[row][0].mul(radius))
            .add(matrix[row][1].mul(radius))
    });
    let contraction = matrix.iter().all(|row| {
        (row[0].lower.abs().max(row[0].upper.abs()) + row[1].lower.abs().max(row[1].upper.abs()))
            .next_up()
            < 1.0
    });
    Ok(Some((image, contraction)))
}

/// Enclose evaluation at an exact stored-f64 local parameter. Input control
/// uncertainty and each de Casteljau arithmetic rounding are retained.
pub(crate) fn interval_tensor_point(
    controls: &IntervalControls,
    u: f64,
    v: f64,
) -> Option<[Interval; 2]> {
    interval_tensor_box(controls, [Interval::exact(u), Interval::exact(v)])
}

/// Enclose the entire uncertain parameter rectangle, not its midpoint.
pub(crate) fn interval_tensor_box(
    controls: &IntervalControls,
    parameters: [Interval; 2],
) -> Option<[Interval; 2]> {
    if parameters
        .iter()
        .any(|t| !unit(t.lower) || !unit(t.upper) || t.lower > t.upper)
    {
        return None;
    }
    interval_tensor_bounds(controls)?;
    fn curve(mut p: [[Interval; 2]; 4], t: Interval) -> [Interval; 2] {
        // Preserve exact boundary identity, avoiding needless 0*infinity.
        if t.lower == 0.0 && t.upper == 0.0 {
            return p[0];
        }
        if t.lower == 1.0 && t.upper == 1.0 {
            return p[3];
        }
        let complement = Interval::exact(1.0).sub(t);
        for width in (1..4).rev() {
            for i in 0..width {
                p[i] = std::array::from_fn(|axis| {
                    p[i][axis].mul(complement).add(p[i + 1][axis].mul(t))
                });
            }
        }
        p[0]
    }
    Some(curve(
        controls.map(|row| curve(row, parameters[0])),
        parameters[1],
    ))
}

/// Candidate traversal using inherited interval uncertainty, without an
/// ad-hoc padding. A successful traversal still does not certify its leaves.
pub(crate) fn visit_interval_candidates(
    controls: IntervalControls,
    point: [f64; 2],
    depth: u8,
    remaining: &mut usize,
    visit: &mut impl FnMut(IntervalControls, [f64; 4]) -> Result<(), SearchError>,
) -> Result<(), SearchError> {
    if depth > 20
        || !point.iter().all(|x| x.is_finite())
        || interval_tensor_bounds(&controls).is_none()
    {
        return Err(SearchError::InvalidInput);
    }
    fn walk(
        controls: IntervalControls,
        rect: [f64; 4],
        point: [f64; 2],
        depth: u8,
        remaining: &mut usize,
        visit: &mut impl FnMut(IntervalControls, [f64; 4]) -> Result<(), SearchError>,
    ) -> Result<(), SearchError> {
        *remaining = remaining.checked_sub(1).ok_or(SearchError::WorkLimit)?;
        let bounds = interval_tensor_bounds(&controls).ok_or(SearchError::InvalidInput)?;
        if (0..2).any(|axis| point[axis] < bounds[axis].lower || point[axis] > bounds[axis].upper) {
            return Ok(());
        }
        if depth == 0 {
            return visit(controls, rect);
        }
        let children = subdivide_interval_tensor(controls).ok_or(SearchError::InvalidInput)?;
        let mid = [(rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5];
        for (index, child) in children.into_iter().enumerate() {
            let x = index % 2;
            let y = index / 2;
            let next = [
                if x == 0 { rect[0] } else { mid[0] },
                if y == 0 { rect[1] } else { mid[1] },
                if x == 0 { mid[0] } else { rect[2] },
                if y == 0 { mid[1] } else { rect[3] },
            ];
            walk(child, next, point, depth - 1, remaining, visit)?;
        }
        Ok(())
    }
    walk(
        controls,
        [0.0, 0.0, 1.0, 1.0],
        point,
        depth,
        remaining,
        visit,
    )
}

/// Convex hull of uncertain controls, retaining all inherited uncertainty.
pub(crate) fn interval_tensor_bounds(controls: &IntervalControls) -> Option<[Interval; 2]> {
    let mut bounds = [Interval {
        lower: f64::INFINITY,
        upper: f64::NEG_INFINITY,
    }; 2];
    for point in controls.iter().flatten() {
        for axis in 0..2 {
            let value = point[axis];
            if value.lower.is_nan() || value.upper.is_nan() || value.lower > value.upper {
                return None;
            }
            bounds[axis].lower = bounds[axis].lower.min(value.lower);
            bounds[axis].upper = bounds[axis].upper.max(value.upper);
        }
    }
    Some(bounds)
}

/// Local-unit-parameter derivative hulls [du/dv][x/y]. On a subdivided
/// cell these are derivatives with respect to that cell, not global u/v.
pub(crate) fn interval_tensor_derivatives(
    controls: &IntervalControls,
) -> Option<[[Interval; 2]; 2]> {
    interval_tensor_bounds(controls)?;
    let mut bounds = [[Interval {
        lower: f64::INFINITY,
        upper: f64::NEG_INFINITY,
    }; 2]; 2];
    for direction in 0..2 {
        for j in 0..4 {
            for i in 0..4 {
                if (direction == 0 && i == 3) || (direction == 1 && j == 3) {
                    continue;
                }
                let next = if direction == 0 {
                    controls[j][i + 1]
                } else {
                    controls[j + 1][i]
                };
                for axis in 0..2 {
                    let derivative = next[axis]
                        .sub(controls[j][i][axis])
                        .mul(Interval::exact(3.0));
                    bounds[direction][axis].lower =
                        bounds[direction][axis].lower.min(derivative.lower);
                    bounds[direction][axis].upper =
                        bounds[direction][axis].upper.max(derivative.upper);
                }
            }
        }
    }
    Some(bounds)
}

/// Propagate control enclosures through midpoint subdivision without
/// discarding conversion uncertainty or subsequent rounding errors.
pub(crate) fn subdivide_interval_tensor(
    controls: IntervalControls,
) -> Option<[IntervalControls; 4]> {
    if controls
        .iter()
        .flatten()
        .flatten()
        .any(|v| v.lower.is_nan() || v.upper.is_nan() || v.lower > v.upper)
    {
        return None;
    }
    fn split(p: [[Interval; 2]; 4]) -> ([[Interval; 2]; 4], [[Interval; 2]; 4]) {
        let midpoint = |a: [Interval; 2], b: [Interval; 2]| {
            std::array::from_fn(|axis| {
                a[axis]
                    .mul(Interval::exact(0.5))
                    .add(b[axis].mul(Interval::exact(0.5)))
            })
        };
        let a = midpoint(p[0], p[1]);
        let b = midpoint(p[1], p[2]);
        let c = midpoint(p[2], p[3]);
        let d = midpoint(a, b);
        let e = midpoint(b, c);
        let center = midpoint(d, e);
        ([p[0], a, d, center], [center, e, c, p[3]])
    }
    let rows = controls.map(split);
    let mut children = [[[[Interval::exact(0.0); 2]; 4]; 4]; 4];
    for side in 0..2 {
        for i in 0..4 {
            let column = std::array::from_fn(|j| {
                if side == 0 {
                    rows[j].0[i]
                } else {
                    rows[j].1[i]
                }
            });
            let (low, high) = split(column);
            for j in 0..4 {
                children[side][j][i] = low[j];
                children[2 + side][j][i] = high[j];
            }
        }
    }
    Some(children)
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Interval {
    pub(crate) lower: f64,
    pub(crate) upper: f64,
}

impl Interval {
    fn exact(value: f64) -> Self {
        Self {
            lower: value,
            upper: value,
        }
    }
    fn enclosing(lower: f64, upper: f64) -> Self {
        if lower.is_nan() || upper.is_nan() {
            Self {
                lower: f64::NEG_INFINITY,
                upper: f64::INFINITY,
            }
        } else {
            Self {
                lower: lower.next_down(),
                upper: upper.next_up(),
            }
        }
    }
    fn add(self, other: Self) -> Self {
        Self::enclosing(self.lower + other.lower, self.upper + other.upper)
    }
    fn sub(self, other: Self) -> Self {
        Self::enclosing(self.lower - other.upper, self.upper - other.lower)
    }
    fn mul(self, other: Self) -> Self {
        let products = [
            self.lower * other.lower,
            self.lower * other.upper,
            self.upper * other.lower,
            self.upper * other.upper,
        ];
        if products.iter().any(|p| p.is_nan()) {
            return Self {
                lower: f64::NEG_INFINITY,
                upper: f64::INFINITY,
            };
        }
        Self::enclosing(
            products.into_iter().fold(f64::INFINITY, f64::min),
            products.into_iter().fold(f64::NEG_INFINITY, f64::max),
        )
    }
    fn third(index: usize) -> Self {
        if index == 0 || index == 3 {
            Self::exact(index as f64 / 3.0)
        } else {
            let v = index as f64 / 3.0;
            Self::enclosing(v, v)
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum SearchError {
    InvalidInput,
    WorkLimit,
    /// A candidate could not be certified; not a coverage miss.
    Unresolved,
}

/// PDF patch precedence among supplied roots: greatest v, then greatest u.
/// Caller must establish root validity/completeness before using the result.
/// No epsilon tie merging: nearly equal v values are not equal parameters.
pub(crate) fn preferred_parameter(roots: &[[f64; 2]]) -> Result<Option<[f64; 2]>, SearchError> {
    if roots.iter().flatten().any(|x| !unit(*x)) {
        return Err(SearchError::InvalidInput);
    }
    let mut best: Option<[f64; 2]> = None;
    for &root in roots {
        if best.is_none_or(|old| root[1] > old[1] || (root[1] == old[1] && root[0] > old[0])) {
            best = Some(root);
        }
    }
    Ok(best)
}

/// Enumerate candidate parameter rectangles, not verified roots.
/// Callback results are provisional until this returns Ok: budget failure
/// must never be interpreted as a complete search or a coverage miss.
pub(crate) fn visit_candidates(
    controls: TensorControls,
    point: [f64; 2],
    padding: f64,
    depth: u8,
    remaining: &mut usize,
    visit: &mut impl FnMut([f64; 4]) -> Result<(), SearchError>,
) -> Result<(), SearchError> {
    if depth > 20
        || !padding.is_finite()
        || padding < 0.0
        || !point.iter().all(|x| x.is_finite())
        || tensor_bounds(&controls).is_none()
    {
        return Err(SearchError::InvalidInput);
    }
    fn walk(
        controls: TensorControls,
        rect: [f64; 4],
        point: [f64; 2],
        padding: f64,
        depth: u8,
        remaining: &mut usize,
        visit: &mut impl FnMut([f64; 4]) -> Result<(), SearchError>,
    ) -> Result<(), SearchError> {
        *remaining = remaining.checked_sub(1).ok_or(SearchError::WorkLimit)?;
        let bounds = tensor_bounds(&controls).ok_or(SearchError::InvalidInput)?;
        if (0..2).any(|a| point[a] < bounds[a] - padding || point[a] > bounds[a + 2] + padding) {
            return Ok(());
        }
        if depth == 0 {
            return visit(rect);
        }
        let children = subdivide_tensor(controls).ok_or(SearchError::InvalidInput)?;
        let mid = [(rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5];
        for (i, child) in children.into_iter().enumerate() {
            let x = i % 2;
            let y = i / 2;
            let next = [
                if x == 0 { rect[0] } else { mid[0] },
                if y == 0 { rect[1] } else { mid[1] },
                if x == 0 { mid[0] } else { rect[2] },
                if y == 0 { mid[1] } else { rect[3] },
            ];
            walk(child, next, point, padding, depth - 1, remaining, visit)?;
        }
        Ok(())
    }
    walk(
        controls,
        [0.0, 0.0, 1.0, 1.0],
        point,
        padding,
        depth,
        remaining,
        visit,
    )
}

/// Control-hull AABB, outward rounded by one representable value.
/// Covers the surface represented by these controls, not accumulated errors
/// from prior conversions/subdivisions; callers must budget those separately.
pub(crate) fn tensor_bounds(controls: &TensorControls) -> Option<[f64; 4]> {
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for point in controls.iter().flatten() {
        if !point.iter().all(|x| x.is_finite()) {
            return None;
        }
        for axis in 0..2 {
            bounds[axis] = bounds[axis].min(point[axis]);
            bounds[axis + 2] = bounds[axis + 2].max(point[axis]);
        }
    }
    Some([
        bounds[0].next_down(),
        bounds[1].next_down(),
        bounds[2].next_up(),
        bounds[3].next_up(),
    ])
}

/// Determinant enclosure of the Jacobian of this control net.
/// Excluding zero proves local nonsingularity, not global injectivity.
pub(crate) fn tensor_jacobian_bounds(controls: &TensorControls) -> Option<[f64; 2]> {
    let derivatives = tensor_derivative_bounds(controls)?;
    if derivatives
        .iter()
        .flatten()
        .flatten()
        .any(|x| !x.is_finite())
    {
        return Some([f64::NEG_INFINITY, f64::INFINITY]);
    }
    fn product(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
        let values = [a[0] * b[0], a[0] * b[1], a[1] * b[0], a[1] * b[1]];
        [
            values.into_iter().fold(f64::INFINITY, f64::min).next_down(),
            values
                .into_iter()
                .fold(f64::NEG_INFINITY, f64::max)
                .next_up(),
        ]
    }
    let positive = product(derivatives[0][0], derivatives[1][1]);
    let negative = product(derivatives[0][1], derivatives[1][0]);
    let bounds = [
        (positive[0] - negative[1]).next_down(),
        (positive[1] - negative[0]).next_up(),
    ];
    if bounds.iter().any(|x| x.is_nan()) {
        Some([f64::NEG_INFINITY, f64::INFINITY])
    } else {
        Some(bounds)
    }
}

fn split_cubic_half(p: [[f64; 2]; 4]) -> ([[f64; 2]; 4], [[f64; 2]; 4]) {
    // Multiplying before adding avoids overflow for large finite controls.
    let midpoint =
        |a: [f64; 2], b: [f64; 2]| std::array::from_fn(|axis| a[axis] * 0.5 + b[axis] * 0.5);
    let a = midpoint(p[0], p[1]);
    let b = midpoint(p[1], p[2]);
    let c = midpoint(p[2], p[3]);
    let d = midpoint(a, b);
    let e = midpoint(b, c);
    let center = midpoint(d, e);
    ([p[0], a, d, center], [center, e, c, p[3]])
}

/// Bernstein derivative hulls, indexed [du/dv][x/y][lower/upper].
/// Subtraction and multiplication are outward rounded separately. These
/// bounds concern this control net, not errors in its prior construction.
pub(crate) fn tensor_derivative_bounds(controls: &TensorControls) -> Option<[[[f64; 2]; 2]; 2]> {
    tensor_bounds(controls)?;
    let mut result = [[[f64::INFINITY, f64::NEG_INFINITY]; 2]; 2];
    for direction in 0..2 {
        for j in 0..4 {
            for i in 0..4 {
                if (direction == 0 && i == 3) || (direction == 1 && j == 3) {
                    continue;
                }
                let next = if direction == 0 {
                    controls[j][i + 1]
                } else {
                    controls[j + 1][i]
                };
                for axis in 0..2 {
                    let difference = next[axis] - controls[j][i][axis];
                    let lower = (difference.next_down() * 3.0).next_down();
                    let upper = (difference.next_up() * 3.0).next_up();
                    result[direction][axis][0] = result[direction][axis][0].min(lower);
                    result[direction][axis][1] = result[direction][axis][1].max(upper);
                }
            }
        }
    }
    Some(result)
}

/// Exact midpoint subdivision, ordered low-v/low-u, low-v/high-u,
/// high-v/low-u, high-v/high-u. No sampling or dynamic allocation.
pub(crate) fn subdivide_tensor(controls: TensorControls) -> Option<[TensorControls; 4]> {
    if !controls.iter().flatten().flatten().all(|x| x.is_finite()) {
        return None;
    }
    let rows = controls.map(split_cubic_half);
    let mut children = [[[[0.0; 2]; 4]; 4]; 4];
    for side in 0..2 {
        for i in 0..4 {
            let column = std::array::from_fn(|j| {
                if side == 0 {
                    rows[j].0[i]
                } else {
                    rows[j].1[i]
                }
            });
            let (low, high) = split_cubic_half(column);
            for j in 0..4 {
                children[side][j][i] = low[j];
                children[2 + side][j][i] = high[j];
            }
        }
    }
    Some(children)
}

/// Derivative of the cubic Bernstein curve.
fn cubic_derivative(points: [[f64; 2]; 4], t: f64) -> [f64; 2] {
    let differences: [[f64; 2]; 3] = std::array::from_fn(|i| {
        std::array::from_fn(|axis| 3.0 * (points[i + 1][axis] - points[i][axis]))
    });
    std::array::from_fn(|axis| {
        (1.0 - t) * (1.0 - t) * differences[0][axis]
            + 2.0 * t * (1.0 - t) * differences[1][axis]
            + t * t * differences[2][axis]
    })
}

/// Weights in PDF corner order, for components/parameters before conversion.
pub(crate) fn corner_weights(u: f64, v: f64) -> Option<[f64; 4]> {
    (unit(u) && unit(v)).then_some([(1.0 - u) * (1.0 - v), (1.0 - u) * v, u * v, u * (1.0 - v)])
}

/// PDF corner components or function parameters, before RGB/function mapping.
/// Validation precedes writes, so failure preserves the caller's buffer.
pub(crate) fn interpolate_corners(
    corners: [&[f32]; 4],
    u: f64,
    v: f64,
    output: &mut [f32],
) -> Option<()> {
    let weights = corner_weights(u, v)?;
    if output.is_empty()
        || corners
            .iter()
            .any(|c| c.len() != output.len() || c.iter().any(|v| !v.is_finite()))
    {
        return None;
    }
    for (index, value) in output.iter_mut().enumerate() {
        *value = corners
            .iter()
            .zip(weights)
            .map(|(corner, weight)| f64::from(corner[index]) * weight)
            .sum::<f64>() as f32;
    }
    Some(())
}

impl CoonsPatch {
    fn from_tensor_record(
        flag: u8,
        new: &[[u32; 2]],
        previous: Option<&Self>,
        width: u8,
        decode: [f64; 4],
    ) -> Option<Self> {
        if flag > 3 || new.len() != if flag == 0 { 16 } else { 12 } {
            return None;
        }
        let mut raw = [[0; 2]; 16];
        let start = if flag == 0 {
            0
        } else {
            let old = previous?.encoded?;
            if old.width != width || old.decode != decode {
                return None;
            }
            for i in 0..4 {
                raw[i] = old.raw[(usize::from(flag) * 3 + i) % 12];
            }
            4
        };
        raw[start..].copy_from_slice(new);
        let mut patch = Self::from_decoded_points(std::array::from_fn(|i| raw[i]), width, decode)?;
        let mut points = [[0.; 2]; 4];
        let mut bounds = [[Interval::exact(0.); 2]; 4];
        for i in 0..4 {
            for axis in 0..2 {
                let (value, interval) = decode_patch_scalar(
                    raw[12 + i][axis],
                    width,
                    [decode[2 * axis], decode[2 * axis + 1]],
                )?;
                points[i][axis] = value;
                bounds[i][axis] = interval;
            }
        }
        patch.tensor_interior = Some((points, bounds));
        Some(patch)
    }

    pub(crate) fn continue_decoded(
        &self,
        flag: u8,
        raw: [[u32; 2]; 8],
        width: u8,
        decode: [f64; 4],
    ) -> Option<Self> {
        if self
            .encoded
            .is_some_and(|old| old.width != width || old.decode != decode)
        {
            return None;
        }
        self.shared_edge(flag)?;
        let mut tail = [[0.0; 2]; 8];
        let mut tail_bounds = [[Interval::exact(0.0); 2]; 8];
        for i in 0..8 {
            for axis in 0..2 {
                let (value, bounds) = decode_patch_scalar(
                    raw[i][axis],
                    width,
                    [decode[axis * 2], decode[axis * 2 + 1]],
                )?;
                tail[i][axis] = value;
                tail_bounds[i][axis] = bounds;
            }
        }
        let mut next = self.continue_from(flag, tail)?;
        let mut bounds = next
            .point_enclosures
            .unwrap_or_else(|| next.points.map(|point| point.map(Interval::exact)));
        bounds[4..].copy_from_slice(&tail_bounds);
        next.point_enclosures = Some(bounds);
        if let Some(old) = self.encoded {
            let mut combined = [[0; 2]; 12];
            for i in 0..4 {
                combined[i] = old.raw[(usize::from(flag) * 3 + i) % 12];
            }
            combined[4..].copy_from_slice(&raw);
            next.encoded = Some(EncodedGeometry {
                raw: combined,
                width,
                decode,
            });
        }
        Some(next)
    }

    pub(crate) fn from_decoded_points(
        raw: [[u32; 2]; 12],
        width: u8,
        decode: [f64; 4],
    ) -> Option<Self> {
        let mut points = [[0.0; 2]; 12];
        let mut enclosures = [[Interval::exact(0.0); 2]; 12];
        for i in 0..12 {
            for axis in 0..2 {
                let (value, bounds) = decode_patch_scalar(
                    raw[i][axis],
                    width,
                    [decode[axis * 2], decode[axis * 2 + 1]],
                )?;
                points[i][axis] = value;
                enclosures[i][axis] = bounds;
            }
        }
        Some(Self {
            points,
            point_enclosures: Some(enclosures),
            encoded: Some(EncodedGeometry { raw, width, decode }),
            tensor_interior: None,
        })
    }

    /// Borrow inherited corner components plus the two newly encoded corners.
    /// No allocation/conversion: the stream decoder must retain or copy these
    /// under its own storage budget before releasing the previous patch.
    pub(crate) fn continued_components<'a>(
        &self,
        flag: u8,
        previous: [&'a [f32]; 4],
        new: [&'a [f32]; 2],
    ) -> Option<[&'a [f32]; 4]> {
        let (_, indices) = self.shared_edge(flag)?;
        let corners = [previous[indices[0]], previous[indices[1]], new[0], new[1]];
        let count = corners[0].len();
        if count == 0
            || corners
                .iter()
                .any(|c| c.len() != count || c.iter().any(|v| !v.is_finite()))
        {
            return None;
        }
        Some(corners)
    }

    /// Exact control identity handles the fully collapsed surface: all
    /// parameters map to one point, so PDF precedence chooses (u,v)=(1,1).
    pub(crate) fn constant_coverage(
        &self,
        target: [f64; 2],
    ) -> Result<ConstantCoverage, SearchError> {
        if !target.iter().all(|x| x.is_finite()) {
            return Err(SearchError::InvalidInput);
        }
        if self
            .point_enclosures
            .is_some_and(|points| points.iter().flatten().any(|v| v.lower != v.upper))
            || self.points.iter().any(|point| *point != self.points[0])
            || self.tensor_interior.is_some_and(|(points, bounds)| {
                points.iter().any(|p| *p != self.points[0])
                    || bounds.iter().flatten().any(|v| v.lower != v.upper)
            })
        {
            return Ok(ConstantCoverage::NotConstant);
        }
        if target == self.points[0] {
            Ok(ConstantCoverage::Parameter([1.0, 1.0]))
        } else {
            Ok(ConstantCoverage::NoCoverage)
        }
    }

    /// Exact control identities prove one coordinate is a strictly monotone
    /// cubic of v alone. Roots of the same device point therefore share v.
    fn proves_common_v(&self) -> bool {
        // A tensor patch needs an identity over its entire control net, not
        // merely the boundary. An exact, strictly monotone coordinate that
        // depends only on v proves that all roots have a common v.
        if self.tensor_interior.is_some() {
            let controls = self.tensor_control_enclosures();
            return (0..2).any(|axis| {
                let values = std::array::from_fn::<_, 4, _>(|j| controls[j][0][axis].lower);
                (0..4).all(|j| {
                    (0..4).all(|i| {
                        controls[j][i][axis].lower == values[j]
                            && controls[j][i][axis].upper == values[j]
                    })
                }) && (values.windows(2).all(|w| w[0] < w[1])
                    || values.windows(2).all(|w| w[0] > w[1]))
            });
        }
        if let Some(encoded) = self.encoded {
            let p = encoded.raw;
            return (0..2).any(|axis| {
                let left = [p[0][axis], p[1][axis], p[2][axis], p[3][axis]];
                let right = [p[9][axis], p[8][axis], p[7][axis], p[6][axis]];
                encoded.decode[axis * 2] != encoded.decode[axis * 2 + 1]
                    && left == right
                    && p[11][axis] == left[0]
                    && p[10][axis] == left[0]
                    && p[4][axis] == left[3]
                    && p[5][axis] == left[3]
                    && (left.windows(2).all(|w| w[0] < w[1])
                        || left.windows(2).all(|w| w[0] > w[1]))
            });
        }
        // Rounded decoded coordinate equality is not exact control identity.
        if self.point_enclosures.is_some() {
            return false;
        }
        let p = &self.points;
        (0..2).any(|axis| {
            let left = [p[0][axis], p[1][axis], p[2][axis], p[3][axis]];
            let right = [p[9][axis], p[8][axis], p[7][axis], p[6][axis]];
            left == right
                && p[11][axis] == left[0]
                && p[10][axis] == left[0]
                && p[4][axis] == left[3]
                && p[5][axis] == left[3]
                && (left.windows(2).all(|w| w[0] < w[1]) || left.windows(2).all(|w| w[0] > w[1]))
        })
    }

    /// Supplied roots must be certified for the same point of this patch.
    pub(crate) fn preferred_root(
        &self,
        roots: &[[Interval; 2]],
        remaining: &mut usize,
    ) -> Result<Option<[Interval; 2]>, SearchError> {
        preferred_certified_root_impl(roots, remaining, self.proves_common_v())
    }

    /// Enclose exact-real Bernstein conversion of the stored f64 points.
    /// Includes rational 1/3 coefficients and each arithmetic rounding step.
    /// Does not include error incurred before points reached this object.
    pub(crate) fn tensor_control_enclosures(&self) -> [[[Interval; 2]; 4]; 4] {
        let p = self
            .point_enclosures
            .unwrap_or_else(|| self.points.map(|point| point.map(Interval::exact)));
        if let Some((_, q)) = self.tensor_interior {
            return [
                [p[0], p[11], p[10], p[9]],
                [p[1], q[0], q[3], p[8]],
                [p[2], q[1], q[2], p[7]],
                [p[3], p[4], p[5], p[6]],
            ];
        }
        let bottom = [p[0], p[11], p[10], p[9]];
        let top = [p[3], p[4], p[5], p[6]];
        let left = [p[0], p[1], p[2], p[3]];
        let right = [p[9], p[8], p[7], p[6]];
        let one = Interval::exact(1.0);
        std::array::from_fn(|j| {
            let v = Interval::third(j);
            let nv = one.sub(v);
            std::array::from_fn(|i| {
                // Coons interpolation reproduces each boundary cubic
                // exactly. Preserve its original enclosure rather than
                // introducing cancellation uncertainty in the formula.
                if j == 0 {
                    return bottom[i];
                }
                if j == 3 {
                    return top[i];
                }
                if i == 0 {
                    return left[j];
                }
                if i == 3 {
                    return right[j];
                }
                let u = Interval::third(i);
                let nu = one.sub(u);
                std::array::from_fn(|axis| {
                    let bilinear = nu
                        .mul(nv)
                        .mul(p[0][axis])
                        .add(nu.mul(v).mul(p[3][axis]))
                        .add(u.mul(v).mul(p[6][axis]))
                        .add(u.mul(nv).mul(p[9][axis]));
                    nv.mul(bottom[i][axis])
                        .add(v.mul(top[i][axis]))
                        .add(nu.mul(left[j][axis]))
                        .add(u.mul(right[j][axis]))
                        .sub(bilinear)
                })
            })
        })
    }

    /// Local Newton refinement constrained to a candidate rectangle.
    /// None means unresolved (including singularity), never proven absence.
    /// A residual-qualified point is not a proof of uniqueness/completeness.
    pub(crate) fn refine_candidate(
        &self,
        target: [f64; 2],
        rect: [f64; 4],
        tolerance: f64,
        remaining: &mut usize,
    ) -> Result<Option<[f64; 2]>, SearchError> {
        if !target.iter().all(|x| x.is_finite())
            || !rect.iter().all(|x| unit(*x))
            || rect[0] > rect[2]
            || rect[1] > rect[3]
            || !tolerance.is_finite()
            || tolerance <= 0.0
        {
            return Err(SearchError::InvalidInput);
        }
        let mut uv = [(rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5];
        for _ in 0..32 {
            *remaining = remaining.checked_sub(1).ok_or(SearchError::WorkLimit)?;
            let Some(p) = self.point(uv[0], uv[1]) else {
                return Ok(None);
            };
            let residual = [p[0] - target[0], p[1] - target[1]];
            if residual.iter().all(|r| r.abs() <= tolerance) {
                return Ok(Some(uv));
            }
            let Some([du, dv]) = self.derivatives(uv[0], uv[1]) else {
                return Ok(None);
            };
            let det = du[0] * dv[1] - du[1] * dv[0];
            if !det.is_finite() || det == 0.0 {
                return Ok(None);
            }
            let step = [
                (residual[0] * dv[1] - residual[1] * dv[0]) / det,
                (du[0] * residual[1] - du[1] * residual[0]) / det,
            ];
            if !step.iter().all(|s| s.is_finite()) {
                return Ok(None);
            }
            // Damping prevents a step from escaping this candidate's domain.
            let mut accepted = false;
            let mut factor = 1.0;
            for _ in 0..16 {
                *remaining = remaining.checked_sub(1).ok_or(SearchError::WorkLimit)?;
                let next = [uv[0] - step[0] * factor, uv[1] - step[1] * factor];
                if next[0] >= rect[0]
                    && next[0] <= rect[2]
                    && next[1] >= rect[1]
                    && next[1] <= rect[3]
                {
                    if let Some(p) = self.point(next[0], next[1]) {
                        let error = (p[0] - target[0]).abs().max((p[1] - target[1]).abs());
                        if error < residual[0].abs().max(residual[1].abs()) {
                            uv = next;
                            accepted = true;
                            break;
                        }
                    }
                }
                factor *= 0.5;
            }
            if !accepted {
                return Ok(None);
            }
        }
        Ok(None)
    }

    /// Exact bicubic Bernstein representation, indexed [v][u].
    /// Unlike corner bounds, its convex hull contains folded interiors too.
    pub(crate) fn tensor_controls(&self) -> Option<[[[f64; 2]; 4]; 4]> {
        let p = &self.points;
        if let Some((q, _)) = self.tensor_interior {
            return Some([
                [p[0], p[11], p[10], p[9]],
                [p[1], q[0], q[3], p[8]],
                [p[2], q[1], q[2], p[7]],
                [p[3], p[4], p[5], p[6]],
            ]);
        }
        let bottom = [p[0], p[11], p[10], p[9]];
        let top = [p[3], p[4], p[5], p[6]];
        let left = [p[0], p[1], p[2], p[3]];
        let right = [p[9], p[8], p[7], p[6]];
        let controls: [[[f64; 2]; 4]; 4] = std::array::from_fn(|j| {
            let v = j as f64 / 3.0;
            std::array::from_fn(|i| {
                let u = i as f64 / 3.0;
                std::array::from_fn(|axis| {
                    let bilinear = (1.0 - u) * (1.0 - v) * p[0][axis]
                        + (1.0 - u) * v * p[3][axis]
                        + u * v * p[6][axis]
                        + u * (1.0 - v) * p[9][axis];
                    (1.0 - v) * bottom[i][axis]
                        + v * top[i][axis]
                        + (1.0 - u) * left[j][axis]
                        + u * right[j][axis]
                        - bilinear
                })
            })
        });
        controls
            .iter()
            .flatten()
            .flatten()
            .all(|x| x.is_finite())
            .then_some(controls)
    }

    /// Nonzero PDF edge flags inherit four points and two corner colours.
    pub(crate) fn shared_edge(&self, flag: u8) -> Option<([[f64; 2]; 4], [usize; 2])> {
        let start = match flag {
            1 => 3,
            2 => 6,
            3 => 9,
            _ => return None,
        };
        Some((
            std::array::from_fn(|i| self.points[(start + i) % 12]),
            [usize::from(flag), (usize::from(flag) + 1) % 4],
        ))
    }

    pub(crate) fn continue_from(&self, flag: u8, tail: [[f64; 2]; 8]) -> Option<Self> {
        let (edge, _) = self.shared_edge(flag)?;
        let mut points = [[0.0; 2]; 12];
        points[..4].copy_from_slice(&edge);
        points[4..].copy_from_slice(&tail);
        let mut next = Self::try_new(points)?;
        if let Some(previous) = self.point_enclosures {
            let start = usize::from(flag) * 3;
            let mut enclosures = points.map(|point| point.map(Interval::exact));
            for i in 0..4 {
                enclosures[i] = previous[(start + i) % 12];
            }
            next.point_enclosures = Some(enclosures);
        }
        Some(next)
    }

    pub(crate) fn try_new(points: [[f64; 2]; 12]) -> Option<Self> {
        points
            .iter()
            .flatten()
            .all(|v| v.is_finite())
            .then_some(Self {
                points,
                point_enclosures: None,
                encoded: None,
                tensor_interior: None,
            })
    }

    pub(crate) fn point(&self, u: f64, v: f64) -> Option<[f64; 2]> {
        let weights = corner_weights(u, v)?;
        if self.tensor_interior.is_some() {
            let rows = self.tensor_controls()?.map(|row| cubic(row, u));
            let result = cubic(rows, v);
            return result.iter().all(|x| x.is_finite()).then_some(result);
        }
        let p = &self.points;
        let bottom = cubic([p[0], p[11], p[10], p[9]], u);
        let top = cubic([p[3], p[4], p[5], p[6]], u);
        let left = cubic([p[0], p[1], p[2], p[3]], v);
        let right = cubic([p[9], p[8], p[7], p[6]], v);
        let corners = [p[0], p[3], p[6], p[9]];
        let result = std::array::from_fn(|axis| {
            let bilinear: f64 = corners.iter().zip(weights).map(|(p, w)| p[axis] * w).sum();
            (1.0 - v) * bottom[axis] + v * top[axis] + (1.0 - u) * left[axis] + u * right[axis]
                - bilinear
        });
        result.iter().all(|v| v.is_finite()).then_some(result)
    }

    /// Analytic derivatives in u/v order; not an inverse or fold resolver.
    pub(crate) fn derivatives(&self, u: f64, v: f64) -> Option<[[f64; 2]; 2]> {
        if !unit(u) || !unit(v) {
            return None;
        }
        if self.tensor_interior.is_some() {
            let controls = self.tensor_controls()?;
            let du = cubic(controls.map(|row| cubic_derivative(row, u)), v);
            let dv = cubic(
                std::array::from_fn(|i| {
                    cubic_derivative(std::array::from_fn(|j| controls[j][i]), v)
                }),
                u,
            );
            let result = [du, dv];
            return result
                .iter()
                .flatten()
                .all(|x| x.is_finite())
                .then_some(result);
        }
        let p = &self.points;
        let c1 = [p[0], p[11], p[10], p[9]];
        let c2 = [p[3], p[4], p[5], p[6]];
        let d1 = [p[0], p[1], p[2], p[3]];
        let d2 = [p[9], p[8], p[7], p[6]];
        let bottom = cubic(c1, u);
        let top = cubic(c2, u);
        let left = cubic(d1, v);
        let right = cubic(d2, v);
        let bottom_derivative = cubic_derivative(c1, u);
        let top_derivative = cubic_derivative(c2, u);
        let left_derivative = cubic_derivative(d1, v);
        let right_derivative = cubic_derivative(d2, v);
        let du = std::array::from_fn(|axis| {
            (1.0 - v) * bottom_derivative[axis] + v * top_derivative[axis] + right[axis]
                - left[axis]
                - ((1.0 - v) * (p[9][axis] - p[0][axis]) + v * (p[6][axis] - p[3][axis]))
        });
        let dv = std::array::from_fn(|axis| {
            top[axis] - bottom[axis]
                + (1.0 - u) * left_derivative[axis]
                + u * right_derivative[axis]
                - ((1.0 - u) * (p[3][axis] - p[0][axis]) + u * (p[6][axis] - p[9][axis]))
        });
        let result = [du, dv];
        result
            .iter()
            .flatten()
            .all(|v| v.is_finite())
            .then_some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_payload_counts_capacity_without_double_counting_geometry() {
        let mut patches = Vec::with_capacity(3);
        let mut values = Vec::with_capacity(12);
        values.extend([0.5; 4]);
        let mut bounds = Vec::with_capacity(10);
        bounds.extend([Interval::exact(0.5); 4]);
        let expected = patches.capacity() * std::mem::size_of::<ColoredPatch>()
            + values.capacity() * std::mem::size_of::<f32>()
            + bounds.capacity() * std::mem::size_of::<Interval>();
        patches.push(ColoredPatch {
            geometry: CoonsPatch::try_new(rectangle()).unwrap(),
            values,
            bounds,
        });
        let mut mesh = PatchMesh {
            patches,
            components: 1,
        };
        assert_eq!(mesh.retained_payload_bytes(), Some(expected));
        mesh.patches.clear();
        assert_eq!(
            mesh.retained_payload_bytes(),
            Some(mesh.patches.capacity() * std::mem::size_of::<ColoredPatch>())
        );
        mesh.patches.shrink_to_fit();
        assert_eq!(mesh.retained_payload_bytes(), Some(0));
    }

    #[test]
    fn mesh_sampling_respects_later_patches_and_never_skips_unresolved_geometry() {
        let make = |geometry, gray: f32| ColoredPatch {
            geometry,
            values: vec![gray; 4],
            bounds: vec![Interval::exact(f64::from(gray)); 4],
        };
        let mut mesh = PatchMesh {
            components: 1,
            patches: vec![
                make(CoonsPatch::try_new(rectangle()).unwrap(), 0.2),
                make(CoonsPatch::try_new(rectangle()).unwrap(), 0.8),
            ],
        };
        let mut value = [-99.0];
        let mut bounds = [Interval::exact(-99.0)];
        let mut budget = 10000;
        assert!(
            mesh.sample([1.5, 1.5], 12, &mut budget, &mut value, &mut bounds)
                .unwrap()
        );
        assert_eq!(value, [0.8]);
        mesh.patches[1].geometry =
            CoonsPatch::try_new(rectangle().map(|p| [p[0] + 6.0, p[1]])).unwrap();
        assert!(
            mesh.sample([1.5, 1.5], 12, &mut budget, &mut value, &mut bounds)
                .unwrap()
        );
        assert_eq!(value, [0.2]);
        assert!(
            !mesh
                .sample([99.0, 99.0], 12, &mut budget, &mut value, &mut bounds)
                .unwrap()
        );
        assert_eq!(value, [0.2]);
        let mut zero = 0;
        assert_eq!(
            mesh.sample([1.5, 1.5], 12, &mut zero, &mut value, &mut bounds),
            Err(SearchError::WorkLimit)
        );
        assert_eq!(value, [0.2]);
        mesh.patches[0] = make(CoonsPatch::try_new([[1.5, 0.0]; 12]).unwrap(), 0.2);
        mesh.patches[1] = make(
            CoonsPatch::try_new(rectangle().map(|p| [p[0], 0.0])).unwrap(),
            0.8,
        );
        assert_eq!(
            mesh.sample([1.5, 0.0], 2, &mut budget, &mut value, &mut bounds),
            Err(SearchError::Unresolved)
        );
        assert_eq!(value, [0.2]);
    }
    #[test]
    fn certified_rectangle_corners_and_edges_preserve_sample_contract() {
        let patch = ColoredPatch {
            geometry: CoonsPatch::try_new(rectangle()).unwrap(),
            values: vec![0.0, 0.0, 1.0, 1.0],
            bounds: vec![
                Interval::exact(0.0),
                Interval::exact(0.0),
                Interval::exact(1.0),
                Interval::exact(1.0),
            ],
        };
        // These straight edges have IVT existence and global uniqueness.
        for (point, expected) in [
            ([0.0, 0.0], 0.0),
            ([0.0, 3.0], 0.0),
            ([3.0, 0.0], 1.0),
            ([3.0, 3.0], 1.0),
        ] {
            let mut values = [-99.0];
            let mut bounds = [Interval::exact(-99.0)];
            assert!(
                patch
                    .sample(point, 2, &mut 10000, &mut values, &mut bounds)
                    .unwrap()
            );
            assert_eq!(values, [expected]);
            assert!(
                bounds[0].lower <= f64::from(expected) && bounds[0].upper >= f64::from(expected)
            );
        }
        for (point, expected) in [
            ([0.0, 1.5], 0.0),
            ([3.0, 1.5], 1.0),
            ([1.5, 0.0], 0.5),
            ([1.5, 3.0], 0.5),
        ] {
            let mut values = [-99.0];
            let mut bounds = [Interval::exact(-99.0)];
            let mut remaining = 10000;
            assert!(
                patch
                    .sample(point, 2, &mut remaining, &mut values, &mut bounds)
                    .unwrap()
            );
            assert_eq!(values, [expected]);
            assert!(
                bounds[0].lower <= f64::from(expected) && bounds[0].upper >= f64::from(expected)
            );
            assert!(remaining < 10000);
            remaining = 10000;
            assert!(
                patch
                    .sample([0.75, 2.25], 12, &mut remaining, &mut values, &mut bounds)
                    .unwrap()
            );
            assert!((values[0] - 0.25).abs() < 1e-6);
        }
    }

    #[test]
    fn patch_sampling_connects_certified_geometry_to_parameters_and_preserves_misses() {
        let values = vec![0.0, 0.0, 1.0, 1.0];
        let patch = ColoredPatch {
            geometry: CoonsPatch::try_new(rectangle()).unwrap(),
            bounds: values
                .iter()
                .map(|&v| Interval::exact(f64::from(v)))
                .collect(),
            values,
        };
        let mut value = [-99.0];
        let mut bounds = [Interval::exact(-99.0)];
        let mut budget = 10000;
        assert!(
            patch
                .sample([0.75, 2.25], 12, &mut budget, &mut value, &mut bounds)
                .unwrap()
        );
        assert!((value[0] - 0.25).abs() < 1e-6);
        assert!(bounds[0].lower <= 0.25 && bounds[0].upper >= 0.25);
        let old = value;
        assert!(
            !patch
                .sample([10.0, 10.0], 12, &mut budget, &mut value, &mut bounds)
                .unwrap()
        );
        assert_eq!(value, old);
        let mut zero = 0;
        assert_eq!(
            patch.sample([0.75, 2.25], 12, &mut zero, &mut value, &mut bounds),
            Err(SearchError::WorkLimit)
        );
        assert_eq!(value, old);
        let collapsed = ColoredPatch {
            geometry: CoonsPatch::from_decoded_points([[0; 2]; 12], 8, [2.0, 3.0, 3.0, 4.0])
                .unwrap(),
            values: vec![0.0, 0.25, 0.75, 1.0],
            bounds: vec![
                Interval::exact(0.0),
                Interval::exact(0.25),
                Interval::exact(0.75),
                Interval::exact(1.0),
            ],
        };
        assert!(
            collapsed
                .sample([2.0, 3.0], 12, &mut budget, &mut value, &mut bounds)
                .unwrap()
        );
        assert_eq!(value, [0.75]);
    }
    #[test]
    fn uncertain_corner_interpolation_preserves_parameter_order_and_atomic_failure() {
        let values = [
            [Interval::exact(0.0)],
            [Interval::exact(1.0)],
            [Interval::exact(0.0)],
            [Interval::exact(1.0)],
        ];
        let corners = values.each_ref().map(|v| v.as_slice());
        let mut result = [Interval::exact(-99.0)];
        interpolate_corner_bounds(
            corners,
            [Interval::exact(0.25), Interval::exact(0.75)],
            &mut result,
        )
        .unwrap();
        assert!(result[0].lower <= 0.625 && result[0].upper >= 0.625);
        let squared = result[0].mul(result[0]);
        assert!(squared.lower <= 0.390625 && squared.upper >= 0.390625);
        let parameters = [
            Interval {
                lower: 0.2,
                upper: 0.3,
            },
            Interval {
                lower: 0.7,
                upper: 0.8,
            },
        ];
        interpolate_corner_bounds(corners, parameters, &mut result).unwrap();
        for u in [0.2, 0.25, 0.3] {
            for v in [0.7, 0.75, 0.8] {
                let t = (1.0 - u) * v + u * (1.0 - v);
                assert!(result[0].lower <= t && result[0].upper >= t);
            }
        }
        let old = result[0];
        assert!(
            interpolate_corner_bounds(
                [corners[0], corners[1], &[], corners[3]],
                parameters,
                &mut result
            )
            .is_none()
        );
        assert_eq!(result[0].lower, old.lower);
        assert_eq!(result[0].upper, old.upper);
    }
    #[test]
    fn owned_stream_covers_every_shared_edge_and_independent_patch_reset() {
        let encoding = PatchEncoding {
            flag_bits: 8,
            coordinate_bits: 8,
            component_bits: 8,
            coordinates: [0.0, 3.0, 0.0, 3.0],
            components: &[[0.0, 1.0]],
        };
        let mut first = vec![0];
        for point in rectangle() {
            first.extend(point.map(|v| (v * 85.0) as u8));
        }
        first.extend([0, 85, 170, 255]);
        for flag in 1_u8..=3 {
            let mut bytes = first.clone();
            bytes.push(flag);
            for _ in 0..8 {
                bytes.extend([85, 170]);
            }
            bytes.extend([64, 192]);
            let mesh = decode_patch_stream(&bytes, &encoding, 2, 100000).unwrap();
            for corner in 0..2 {
                let old = (usize::from(flag) + corner) % 4;
                assert_eq!(mesh.patches[1].values[corner], mesh.patches[0].values[old]);
            }
            assert_eq!(mesh.patches[1].values[2], 64.0_f32 / 255.0);
            assert_eq!(mesh.patches[1].values[3], 192.0_f32 / 255.0);
            for i in 0..4 {
                assert_eq!(
                    mesh.patches[1].geometry.points[i],
                    mesh.patches[0].geometry.points[(usize::from(flag) * 3 + i) % 12]
                );
            }
        }
        let mut reset = first.clone();
        reset.extend_from_slice(&first);
        let mesh = decode_patch_stream(&reset, &encoding, 2, 100000).unwrap();
        assert_eq!(mesh.patches[0].values, mesh.patches[1].values);
        assert_eq!(
            mesh.patches[0].geometry.points,
            mesh.patches[1].geometry.points
        );
    }
    #[test]
    fn late_decode_failure_discards_partial_stream_and_allows_retry() {
        let mut bytes = vec![0; 29];
        bytes.push(1);
        bytes.extend([0; 16]);
        bytes.extend([255, 0]);
        let bad = PatchEncoding {
            flag_bits: 8,
            coordinate_bits: 8,
            component_bits: 8,
            coordinates: [0.0, 1.0, 0.0, 1.0],
            components: &[[0.0, f64::MAX]],
        };
        assert!(matches!(
            decode_patch_stream(&bytes, &bad, 2, 100000),
            Err(PatchStreamError::InvalidDecode)
        ));
        let good = PatchEncoding {
            components: &[[0.0, 1.0]],
            ..bad
        };
        let mesh = decode_patch_stream(&bytes, &good, 2, 100000).unwrap();
        assert_eq!(mesh.patches.len(), 2);
        assert_eq!(mesh.patches[0].values, [0.0; 4]);
        assert_eq!(mesh.patches[1].values, [0.0, 0.0, 1.0, 0.0]);
    }
    #[test]
    fn owned_stream_preserves_shared_corner_data_and_preflights_storage() {
        let bytes = [0, 0, 0, 0x2b, 0x12, 0x34, 0x80];
        let encoding = PatchEncoding {
            flag_bits: 2,
            coordinate_bits: 1,
            component_bits: 1,
            coordinates: [0.0, 3.0, 0.0, 3.0],
            components: &[[0.0, 1.0]],
        };
        let mesh = decode_patch_stream(&bytes, &encoding, 2, 100000).unwrap();
        assert_eq!(mesh.components, 1);
        assert_eq!(mesh.patches.len(), 2);
        assert_eq!(mesh.patches[0].values, [1.0, 0.0, 1.0, 0.0]);
        assert_eq!(mesh.patches[1].values, [0.0, 1.0, 1.0, 0.0]);
        for (new, old) in [(0, 3), (1, 0)] {
            assert_eq!(
                mesh.patches[1].bounds[new].lower,
                mesh.patches[0].bounds[old].lower
            );
            assert_eq!(
                mesh.patches[1].bounds[new].upper,
                mesh.patches[0].bounds[old].upper
            );
        }
        let required = bytes.len()
            + 2 * std::mem::size_of::<ColoredPatch>()
            + 4 * std::mem::size_of::<u32>()
            + 2 * 4 * (std::mem::size_of::<f32>() + std::mem::size_of::<Interval>());
        assert!(matches!(
            decode_patch_stream(&bytes, &encoding, 2, required - 1),
            Err(PatchStreamError::MemoryLimit)
        ));
        assert!(decode_patch_stream(&bytes, &encoding, 2, required).is_ok());
        assert!(matches!(
            decode_patch_stream(&bytes, &encoding, 1, 100000),
            Err(PatchStreamError::PatchLimit)
        ));
    }
    #[test]
    fn decoded_record_assembles_shared_geometry_and_parameters_atomically() {
        let bytes = [0, 0, 0, 0x2b, 0x12, 0x34, 0x80];
        let encoding = PatchEncoding {
            flag_bits: 2,
            coordinate_bits: 1,
            component_bits: 1,
            coordinates: [0.0, 3.0, 0.0, 3.0],
            components: &[[0.0, 1.0]],
        };
        let mut bits = PatchBits::new(&bytes);
        let mut scratch = [0; 4];
        let mut values = [-99.0; 4];
        let mut bounds = [Interval::exact(-99.0); 4];
        let (flag, first) = bits
            .decoded_record(&encoding, None, &mut scratch, &mut values, &mut bounds)
            .unwrap();
        assert_eq!(flag, 0);
        assert_eq!(values, [1.0, 0.0, 1.0, 0.0]);
        assert_eq!(bits.remaining(), Some(26));
        let mut missing = bits;
        let mut new_values = [-99.0; 2];
        let mut new_bounds = [Interval::exact(-99.0); 2];
        assert!(
            missing
                .decoded_record(
                    &encoding,
                    None,
                    &mut scratch[..2],
                    &mut new_values,
                    &mut new_bounds
                )
                .is_none()
        );
        assert_eq!(missing.remaining(), Some(26));
        assert_eq!(new_values, [-99.0; 2]);
        let (flag, next) = bits
            .decoded_record(
                &encoding,
                Some(&first),
                &mut scratch[..2],
                &mut new_values,
                &mut new_bounds,
            )
            .unwrap();
        assert_eq!(flag, 3);
        assert_eq!(new_values, [1.0, 0.0]);
        assert_eq!(next.points[4], [0.0, 0.0]);
        assert_eq!(next.points[5], [0.0, 3.0]);
        assert_eq!(bits.remaining(), Some(6));
        let invalid = PatchEncoding {
            components: &[[0.0, f64::MAX]],
            ..encoding
        };
        let mut bits = PatchBits::new(&bytes);
        values = [-99.0; 4];
        bounds = [Interval::exact(-99.0); 4];
        assert!(
            bits.decoded_record(&invalid, None, &mut scratch, &mut values, &mut bounds)
                .is_none()
        );
        assert_eq!(bits.remaining(), Some(56));
        assert_eq!(values, [-99.0; 4]);
        assert!(bounds.iter().all(|b| b.lower == -99.0 && b.upper == -99.0));
    }
    #[test]
    fn stream_scan_preflights_continuous_records_and_rejects_incomplete_structure() {
        let bytes = [0, 0, 0, 0x2b, 0x12, 0x34, 0x80];
        assert_eq!(
            scan_patch_stream(&bytes, 2, 1, 1, 1, 2),
            Ok(PatchStreamPlan {
                patches: 2,
                payload_bits: 50,
                tail_bits: 6
            })
        );
        assert_eq!(
            scan_patch_stream(&bytes, 2, 1, 1, 1, 1),
            Err(PatchStreamError::PatchLimit)
        );
        assert_eq!(
            scan_patch_stream(&bytes[..3], 2, 1, 1, 1, 2),
            Err(PatchStreamError::Truncated)
        );
        assert_eq!(
            scan_patch_stream(&[], 2, 1, 1, 1, 2),
            Err(PatchStreamError::Empty)
        );
        assert_eq!(
            scan_patch_stream(&[0xc0; 7], 2, 1, 1, 1, 2),
            Err(PatchStreamError::MissingPreviousPatch)
        );
        assert_eq!(
            scan_patch_stream(&bytes, 2, 3, 1, 1, 2),
            Err(PatchStreamError::InvalidEncoding)
        );
        // An extra full byte is not treated as an arbitrary padding suffix.
        let extended = [0, 0, 0, 0x2b, 0x12, 0x34, 0x80, 0];
        assert_eq!(
            scan_patch_stream(&extended, 2, 1, 1, 1, 3),
            Err(PatchStreamError::Truncated)
        );
    }
    #[test]
    fn corner_decode_is_atomic_and_retains_f32_rounding_uncertainty() {
        let raw = [0, 255, 128, 255, 0, 64];
        let decode = [[0.0, 1.0], [1.0, 0.0], [-1.0, 1.0]];
        let mut values = [-99.0; 6];
        let mut bounds = [Interval::exact(-99.0); 6];
        decode_patch_components(&raw, 8, &decode, 2, &mut values, &mut bounds).unwrap();
        assert_eq!(values[0], 0.0);
        assert_eq!(values[1], 0.0);
        assert_eq!(values[3], 1.0);
        assert_eq!(values[4], 1.0);
        for i in 0..6 {
            let (exact, _) = decode_patch_scalar(raw[i], 8, decode[i % 3]).unwrap();
            assert!(bounds[i].lower <= exact && bounds[i].upper >= exact);
            assert!(
                bounds[i].lower <= f64::from(values[i]) && bounds[i].upper >= f64::from(values[i])
            );
        }
        let old_values = values;
        let old_bounds = bounds;
        let mut invalid = raw;
        invalid[5] = 256;
        assert!(
            decode_patch_components(&invalid, 8, &decode, 2, &mut values, &mut bounds).is_none()
        );
        assert_eq!(values, old_values);
        for i in 0..6 {
            assert_eq!(bounds[i].lower, old_bounds[i].lower);
            assert_eq!(bounds[i].upper, old_bounds[i].upper);
        }
        assert!(decode_patch_components(&raw, 8, &decode, 4, &mut values, &mut bounds).is_none());
        assert!(
            decode_patch_components(
                &[255, 255],
                8,
                &[[0.0, f64::MAX]],
                2,
                &mut values[..2],
                &mut bounds[..2]
            )
            .is_none()
        );
        assert_eq!(values, old_values);
    }
    #[test]
    fn encoded_identity_rejects_rounding_collisions_and_encoding_changes() {
        let mut raw = rectangle().map(|point| point.map(|v| (v * 85.0) as u32));
        let decode = [0.0, 3.0, 1.0, 1.0_f64.next_up()];
        raw[4][1] = 254; // Distinct real value can round to the same f64 corner.
        let patch = CoonsPatch::from_decoded_points(raw, 8, decode).unwrap();
        assert_eq!(patch.points[4][1], patch.points[3][1]);
        assert!(!patch.proves_common_v());
        assert!(patch.continue_decoded(1, [[1; 2]; 8], 16, decode).is_none());
        assert!(
            patch
                .continue_decoded(1, [[1; 2]; 8], 8, [0.0, 4.0, 1.0, 1.0_f64.next_up()])
                .is_none()
        );
        let next = patch.continue_decoded(3, [[1; 2]; 8], 8, decode).unwrap();
        let source = next.encoded.unwrap();
        for i in 0..4 {
            assert_eq!(source.raw[i], raw[(9 + i) % 12]);
        }
        assert_eq!(&source.raw[4..], &[[1; 2]; 8]);
    }
    #[test]
    fn decoded_continuations_preserve_both_inherited_and_new_point_errors() {
        let raw = rectangle().map(|point| point.map(|x| (x * 85.0) as u32));
        let patch = CoonsPatch::from_decoded_points(raw, 8, [0.0, 3.0, 0.0, 3.0]).unwrap();
        let previous = patch.point_enclosures.unwrap();
        for flag in 1..=3 {
            let next = patch
                .continue_decoded(flag, [[85, 170]; 8], 8, [0.0, 3.0, 0.0, 3.0])
                .unwrap();
            let bounds = next.point_enclosures.unwrap();
            for i in 0..4 {
                for axis in 0..2 {
                    let old = previous[(flag as usize * 3 + i) % 12][axis];
                    assert_eq!(bounds[i][axis].lower, old.lower);
                    assert_eq!(bounds[i][axis].upper, old.upper);
                }
            }
            for i in 4..12 {
                assert_eq!(next.points[i], [1.0, 2.0]);
                assert!(bounds[i][0].lower < 1.0 && bounds[i][0].upper > 1.0);
                assert!(bounds[i][1].lower < 2.0 && bounds[i][1].upper > 2.0);
            }
        }
        assert!(
            patch
                .continue_decoded(0, [[85, 170]; 8], 8, [0.0, 3.0, 0.0, 3.0])
                .is_none()
        );
        assert!(
            patch
                .continue_decoded(1, [[256, 170]; 8], 8, [0.0, 3.0, 0.0, 3.0])
                .is_none()
        );
        assert_eq!(patch.points, rectangle());
    }
    #[test]
    fn decoded_coordinate_uncertainty_survives_conversion_and_shared_edges() {
        let raw = rectangle().map(|point| point.map(|x| (x * 85.0) as u32));
        let patch = CoonsPatch::from_decoded_points(raw, 8, [0.0, 3.0, 0.0, 3.0]).unwrap();
        let source = patch.point_enclosures.unwrap();
        assert!(source[1][1].lower <= 1.0 && source[1][1].upper >= 1.0);
        assert!(source[1][1].lower < source[1][1].upper);
        let controls = patch.tensor_control_enclosures();
        assert!(controls[1][0][1].lower <= 1.0 && controls[1][0][1].upper >= 1.0);
        assert!(patch.proves_common_v());
        let next = patch.continue_from(1, [[4.0, 4.0]; 8]).unwrap();
        for i in 0..4 {
            for axis in 0..2 {
                assert_eq!(
                    next.point_enclosures.unwrap()[i][axis].lower,
                    source[3 + i][axis].lower
                );
                assert_eq!(
                    next.point_enclosures.unwrap()[i][axis].upper,
                    source[3 + i][axis].upper
                );
            }
        }
        assert!(CoonsPatch::from_decoded_points([[256; 2]; 12], 8, [0.0, 3.0, 0.0, 3.0]).is_none());
    }
    #[test]
    fn decode_scalar_retains_32bit_coordinates_reversed_ranges_and_error_bounds() {
        let (zero, z) = decode_patch_scalar(0, 32, [1.0, 1e12]).unwrap();
        assert_eq!(zero, 1.0);
        assert_eq!(z.lower, 1.0);
        assert_eq!(z.upper, 1.0);
        let (maximum, m) = decode_patch_scalar(u32::MAX, 32, [1.0, 1e12]).unwrap();
        assert_eq!(maximum, 1e12);
        assert_eq!(m.lower, 1e12);
        assert_eq!(m.upper, 1e12);
        let (near, bounds) = decode_patch_scalar(u32::MAX - 1, 32, [0.0, 1e12]).unwrap();
        assert!(near < 1e12 && bounds.lower <= near && bounds.upper >= near);
        let (reversed, bounds) = decode_patch_scalar(64, 8, [2.0, -2.0]).unwrap();
        let expected = 2.0 - 4.0 * 64.0 / 255.0;
        assert!((reversed - expected).abs() < 1e-15);
        assert!(bounds.lower <= expected && bounds.upper >= expected);
        let (large, bounds) = decode_patch_scalar(1, 2, [-f64::MAX, f64::MAX]).unwrap();
        assert!(large.is_finite() && bounds.lower <= large && bounds.upper >= large);
        assert!(decode_patch_scalar(256, 8, [0.0, 1.0]).is_none());
        assert!(decode_patch_scalar(0, 0, [0.0, 1.0]).is_none());
        assert!(decode_patch_scalar(0, 8, [f64::NAN, 1.0]).is_none());
    }
    #[test]
    fn integer_identity_decode_preserves_exact_non_endpoint_coordinates() {
        for width in 1..=32 {
            let maximum = ((1_u64 << width) - 1) as u32;
            for raw in [0, 1, maximum / 2, maximum - 1, maximum] {
                for reversed in [false, true] {
                    let decode = if reversed {
                        [f64::from(maximum), 0.0]
                    } else {
                        [0.0, f64::from(maximum)]
                    };
                    let expected = f64::from(if reversed { maximum - raw } else { raw });
                    let (value, bounds) = decode_patch_scalar(raw, width, decode).unwrap();
                    assert_eq!(value, expected);
                    assert_eq!(bounds.lower, expected);
                    assert_eq!(bounds.upper, expected);
                }
            }
        }
        let (_, bounds) = decode_patch_scalar(1, 8, [0.0, 254.0]).unwrap();
        assert!(bounds.lower < bounds.upper);
        let (_, bounds) = decode_patch_scalar(1, 8, [254.0, 0.0]).unwrap();
        assert!(bounds.lower < bounds.upper);
    }
    #[test]
    fn raw_record_preflight_preserves_input_and_output_on_failure() {
        let bytes = [0, 0, 0, 0x2b, 0x12, 0x34, 0x80];
        let mut bits = PatchBits::new(&bytes);
        let mut points = [[99; 2]; 12];
        let mut colors = [99; 4];
        assert_eq!(bits.peek_flag(2), Some(0));
        assert_eq!(bits.record(2, 1, 1, 1, &mut points[..8], &mut colors), None);
        assert_eq!(points, [[99; 2]; 12]);
        assert_eq!(colors, [99; 4]);
        assert_eq!(bits.remaining(), Some(56));
        assert_eq!(bits.record(2, 1, 1, 1, &mut points, &mut colors), Some(0));
        assert_eq!(points, [[0; 2]; 12]);
        assert_eq!(colors, [1, 0, 1, 0]);
        let mut continued = [[99; 2]; 8];
        let mut new_colors = [99; 2];
        assert_eq!(bits.peek_flag(2), Some(3));
        assert_eq!(
            bits.record(2, 1, 1, 1, &mut continued, &mut new_colors),
            Some(3)
        );
        assert_eq!(
            continued,
            [
                [0, 0],
                [0, 1],
                [0, 0],
                [1, 0],
                [0, 0],
                [1, 1],
                [0, 1],
                [0, 0]
            ]
        );
        assert_eq!(new_colors, [1, 0]);
        assert_eq!(bits.remaining(), Some(6));
        let mut truncated = PatchBits::new(&bytes[..3]);
        points = [[99; 2]; 12];
        colors = [99; 4];
        assert_eq!(truncated.record(2, 1, 1, 1, &mut points, &mut colors), None);
        assert_eq!(points, [[99; 2]; 12]);
        assert_eq!(colors, [99; 4]);
        assert_eq!(truncated.remaining(), Some(24));
    }
    #[test]
    fn patch_bits_read_continuous_records_and_preserve_position_on_failure() {
        // 30-bit new patch followed immediately by a 20-bit continuation.
        let mut bits = PatchBits::new(&[0, 0, 0, 0x2b, 0x12, 0x34, 0x80]);
        assert_eq!(bits.flag(2), Some(0));
        assert_eq!(bits.read(24), Some(0));
        assert_eq!(bits.read(4), Some(0b1010));
        assert_eq!(bits.flag(2), Some(3));
        assert_eq!(bits.read(16), Some(0x1234));
        assert_eq!(bits.read(2), Some(0b10));
        assert_eq!(bits.remaining(), Some(6));
        assert_eq!(bits.read(8), None);
        assert_eq!(bits.read(0), None);
        assert_eq!(bits.read(33), None);
        assert_eq!(bits.remaining(), Some(6));
        assert_eq!(bits.read(6), Some(0));
        assert_eq!(bits.remaining(), Some(0));
        assert_eq!(PatchBits::new(&[255; 4]).read(32), Some(u32::MAX));
        assert_eq!(PatchBits::new(&[255]).flag(8), Some(3));
    }
    #[test]
    fn patch_record_layout_counts_geometry_before_all_corner_components() {
        assert_eq!(
            patch_record_layout(0, 8, 8, 8, 1),
            Some(PatchRecordLayout {
                points: 12,
                corners: 4,
                bits: 232
            })
        );
        for flag in 1..=3 {
            assert_eq!(
                patch_record_layout(flag, 8, 8, 8, 1),
                Some(PatchRecordLayout {
                    points: 8,
                    corners: 2,
                    bits: 152
                })
            );
        }
        assert_eq!(patch_record_layout(0, 8, 8, 8, 3).unwrap().bits, 296);
        assert_eq!(patch_record_layout(0, 2, 1, 1, 1).unwrap().bits, 30);
        assert_eq!(patch_record_layout(3, 2, 1, 1, 1).unwrap().bits, 20);
        assert!(patch_record_layout(0, 8, 32, 16, usize::MAX).is_none());
        assert!(patch_record_layout(4, 8, 8, 8, 1).is_none());
        assert!(patch_record_layout(0, 3, 8, 8, 1).is_none());
        assert!(patch_record_layout(0, 8, 3, 8, 1).is_none());
        assert!(patch_record_layout(0, 8, 8, 3, 1).is_none());
        assert!(patch_record_layout(0, 8, 8, 8, 0).is_none());
    }
    #[test]
    fn continuation_inherits_encoded_components_in_pdf_corner_order() {
        let patch = CoonsPatch::try_new(rectangle()).unwrap();
        let values = [[0.0, 1.0], [2.0, 3.0], [4.0, 5.0], [6.0, 7.0]];
        let previous = values.each_ref().map(|c| c.as_slice());
        let new = [[8.0, 9.0], [10.0, 11.0]];
        let new_refs = new.each_ref().map(|c| c.as_slice());
        for flag in 1..=3 {
            let corners = patch
                .continued_components(flag, previous, new_refs)
                .unwrap();
            assert_eq!(corners[0], previous[flag as usize]);
            assert_eq!(corners[1], previous[(flag as usize + 1) % 4]);
            assert_eq!(corners[2], new_refs[0]);
            assert_eq!(corners[3], new_refs[1]);
            assert!(std::ptr::eq(
                corners[0].as_ptr(),
                previous[flag as usize].as_ptr()
            ));
        }
        assert!(patch.continued_components(0, previous, new_refs).is_none());
        assert!(patch.continued_components(4, previous, new_refs).is_none());
        assert!(
            patch
                .continued_components(1, previous, [&[], new_refs[1]])
                .is_none()
        );
        assert!(
            patch
                .continued_components(1, previous, [&[f32::NAN, 0.0], new_refs[1]])
                .is_none()
        );
    }
    #[test]
    fn constant_surface_selects_maximum_v_and_u_for_continuous_roots() {
        let patch = CoonsPatch::try_new([[2.0, 3.0]; 12]).unwrap();
        assert_eq!(
            patch.constant_coverage([2.0, 3.0]),
            Ok(ConstantCoverage::Parameter([1.0, 1.0]))
        );
        assert_eq!(
            patch.constant_coverage([2.0_f64.next_up(), 3.0]),
            Ok(ConstantCoverage::NoCoverage)
        );
        assert_eq!(
            patch.constant_coverage([f64::NAN, 3.0]),
            Err(SearchError::InvalidInput)
        );
        let corners = [[0.0], [0.25], [0.75], [1.0]];
        let mut output = [-1.0];
        interpolate_corners(
            corners.each_ref().map(|c| c.as_slice()),
            1.0,
            1.0,
            &mut output,
        )
        .unwrap();
        assert_eq!(output, [0.75]); // PDF corner 2, not the last encoded corner.
        let mut not_constant = patch;
        not_constant.points[5][0] = 2.0_f64.next_up();
        assert_eq!(
            not_constant.constant_coverage([2.0, 3.0]),
            Ok(ConstantCoverage::NotConstant)
        );
    }
    #[test]
    fn common_v_control_identity_selects_greatest_u_without_epsilon_ties() {
        let mut patch = CoonsPatch::try_new(rectangle()).unwrap();
        patch.points[11][0] = 4.0;
        patch.points[10][0] = -1.0;
        patch.points[4][0] = 4.0;
        patch.points[5][0] = -1.0;
        assert!(patch.proves_common_v());
        let mut budget = 10000;
        let mut roots = Vec::new();
        visit_certified_roots(
            patch.tensor_control_enclosures(),
            [1.5, 1.5],
            12,
            &mut budget,
            &mut |r| {
                roots.push(r);
                Ok(())
            },
        )
        .unwrap();
        let mut budget = 100;
        let winner = patch.preferred_root(&roots, &mut budget).unwrap().unwrap();
        let expected = 0.5 + 3.0_f64.sqrt() / 6.0;
        assert!(winner[0].lower <= expected && winner[0].upper >= expected);
        patch.points[4][1] = 6.0;
        assert!(!patch.proves_common_v());
    }
    #[test]
    fn offset_first_split_certifies_midpoint_fold_roots() {
        let mut patch = CoonsPatch::try_new(rectangle()).unwrap();
        patch.points[11][0] = 4.0;
        patch.points[10][0] = -1.0;
        patch.points[4][0] = 4.0;
        patch.points[5][0] = -1.0;
        for v in [0.5, 0.6875] {
            let controls = patch.tensor_control_enclosures();
            let target = [1.5, 3.0 * v];
            // Global candidate contraction cannot discard competing roots
            // and turn this three-root domain into a unique certificate.
            assert!(matches!(
                contract_candidate_cell(controls, target, &mut 10000).unwrap(),
                CellCertificate::Unresolved
            ));
            let mut roots = Vec::new();
            let mut budget = 10000;
            visit_certified_roots(controls, target, 12, &mut budget, &mut |r| {
                roots.push(r);
                Ok(())
            })
            .unwrap();
            assert_eq!(roots.len(), 3);
            for u in [0.5 - 3.0_f64.sqrt() / 6.0, 0.5, 0.5 + 3.0_f64.sqrt() / 6.0] {
                assert!(roots.iter().any(|r| r[0].lower <= u
                    && r[0].upper >= u
                    && r[1].lower <= v
                    && r[1].upper >= v));
            }
        }
    }
    #[test]
    fn traversal_and_refinement_share_work_allowance_before_emitting_roots() {
        let controls = CoonsPatch::try_new(rectangle())
            .unwrap()
            .tensor_control_enclosures();
        let mut budget = 1;
        let mut calls = 0;
        assert_eq!(
            visit_certified_roots(controls, [0.6, 2.4], 8, &mut budget, &mut |_| {
                calls += 1;
                Ok(())
            }),
            Err(SearchError::WorkLimit)
        );
        assert_eq!(calls, 0);
        assert_eq!(budget, 0);
    }
    #[test]
    fn certified_root_tightening_preserves_inclusion_and_work_limit() {
        let mut patch = CoonsPatch::try_new(rectangle()).unwrap();
        patch.points[4][1] = 6.0;
        patch.points[5][1] = 6.0;
        let controls =
            restrict_interval_tensor(patch.tensor_control_enclosures(), [0.2, 0.3, 0.4, 0.5])
                .unwrap();
        let target = patch.point(0.3, 0.4).unwrap();
        let CellCertificate::Unique(original) = certify_interval_cell(&controls, target).unwrap()
        else {
            panic!("restricted root must certify")
        };
        let mut budget = 100;
        let CellCertificate::Unique(tight) =
            tighten_unique_cell(&controls, target, 8, &mut budget).unwrap()
        else {
            panic!()
        };
        for axis in 0..2 {
            assert!(
                tight[axis].lower >= original[axis].lower
                    && tight[axis].upper <= original[axis].upper
            );
            assert!(tight[axis].lower <= 0.5 && tight[axis].upper >= 0.5);
        }
        assert!(tight[1].upper - tight[1].lower < original[1].upper - original[1].lower);
        let mut zero = 0;
        assert!(matches!(
            tighten_unique_cell(&controls, target, 8, &mut zero),
            Err(SearchError::WorkLimit)
        ));
    }
    #[test]
    fn arbitrary_restriction_preserves_curved_surface_and_parameter_scale() {
        let mut patch = CoonsPatch::try_new(rectangle()).unwrap();
        patch.points[4][1] = 6.0;
        patch.points[5][1] = 6.0;
        let controls = patch.tensor_control_enclosures();
        let rect = [0.2, 0.3, 0.7, 0.8];
        let restricted = restrict_interval_tensor(controls, rect).unwrap();
        for u in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for v in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let expected = patch
                    .point(
                        rect[0] + u * (rect[2] - rect[0]),
                        rect[1] + v * (rect[3] - rect[1]),
                    )
                    .unwrap();
                let actual = interval_tensor_point(&restricted, u, v).unwrap();
                for axis in 0..2 {
                    assert!(
                        actual[axis].lower <= expected[axis]
                            && actual[axis].upper >= expected[axis]
                    );
                }
            }
        }
        let identity = restrict_interval_tensor(controls, [0.0, 0.0, 1.0, 1.0]).unwrap();
        for j in 0..4 {
            for i in 0..4 {
                for axis in 0..2 {
                    assert_eq!(identity[j][i][axis].lower, controls[j][i][axis].lower);
                    assert_eq!(identity[j][i][axis].upper, controls[j][i][axis].upper);
                }
            }
        }
        assert!(restrict_interval_tensor(controls, [0.7, 0.3, 0.2, 0.8]).is_none());
    }
    #[test]
    fn uncertain_parameter_evaluation_covers_entire_box_not_just_center() {
        let mut patch = CoonsPatch::try_new(rectangle()).unwrap();
        patch.points[4][1] = 6.0;
        patch.points[5][1] = 6.0;
        let controls = patch.tensor_control_enclosures();
        let parameters = [
            Interval {
                lower: 0.2,
                upper: 0.4,
            },
            Interval {
                lower: 0.6,
                upper: 0.8,
            },
        ];
        let bounds = interval_tensor_box(&controls, parameters).unwrap();
        for u in [0.2, 0.3, 0.4] {
            for v in [0.6, 0.7, 0.8] {
                let p = patch.point(u, v).unwrap();
                for axis in 0..2 {
                    assert!(bounds[axis].lower <= p[axis] && bounds[axis].upper >= p[axis]);
                }
            }
        }
        let invalid = [
            Interval {
                lower: 0.4,
                upper: 0.2,
            },
            parameters[1],
        ];
        assert!(interval_tensor_box(&controls, invalid).is_none());
        let CellCertificate::Unique(root) = certify_interval_cell(
            &CoonsPatch::try_new(rectangle())
                .unwrap()
                .tensor_control_enclosures(),
            [0.6, 2.4],
        )
        .unwrap() else {
            panic!()
        };
        let rectangle = CoonsPatch::try_new(rectangle())
            .unwrap()
            .tensor_control_enclosures();
        let mapped = interval_tensor_box(&rectangle, root).unwrap();
        assert!(mapped[0].lower <= 0.6 && mapped[0].upper >= 0.6);
        assert!(mapped[1].lower <= 2.4 && mapped[1].upper >= 2.4);
    }
    #[test]
    fn certified_precedence_requires_proven_order_and_accounts_comparisons() {
        let interval = |lower, upper| Interval { lower, upper };
        let roots = [
            [interval(0.8, 0.9), interval(0.2, 0.3)],
            [interval(0.1, 0.2), interval(0.7, 0.8)],
        ];
        let mut budget = 100;
        let winner = preferred_certified_root(&roots, &mut budget)
            .unwrap()
            .unwrap();
        assert_eq!(winner[1].lower, 0.7);
        let tied = [
            [interval(0.1, 0.2), Interval::exact(0.5)],
            [interval(0.7, 0.8), Interval::exact(0.5)],
        ];
        let winner = preferred_certified_root(&tied, &mut budget)
            .unwrap()
            .unwrap();
        assert_eq!(winner[0].lower, 0.7);
        let overlapping = [
            [interval(0.1, 0.2), interval(0.4, 0.6)],
            [interval(0.7, 0.8), interval(0.5, 0.7)],
        ];
        assert!(matches!(
            preferred_certified_root(&overlapping, &mut budget),
            Err(SearchError::Unresolved)
        ));
        let mut zero = 0;
        assert!(matches!(
            preferred_certified_root(&roots, &mut zero),
            Err(SearchError::WorkLimit)
        ));
        assert!(preferred_certified_root(&[], &mut zero).unwrap().is_none());
    }
    #[test]
    fn adaptive_certification_finds_fold_roots_and_never_hides_unresolved_cells() {
        let mut patch = CoonsPatch::try_new(rectangle()).unwrap();
        patch.points[11][0] = 4.0;
        patch.points[10][0] = -1.0;
        patch.points[4][0] = 4.0;
        patch.points[5][0] = -1.0;
        let controls = patch.tensor_control_enclosures();
        let mut roots = Vec::new();
        let mut budget = 10000;
        visit_certified_roots(controls, [1.4, 1.2], 12, &mut budget, &mut |root| {
            roots.push(root);
            Ok(())
        })
        .unwrap();
        assert_eq!(roots.len(), 3);
        for root in &roots {
            assert!(root[1].lower <= 0.4 && root[1].upper >= 0.4);
            assert!(root[0].upper - root[0].lower < 1e-6);
            assert!(root[1].upper - root[1].lower < 1e-6);
        }
        roots.sort_by(|a, b| a[0].lower.total_cmp(&b[0].lower));
        assert!(roots[0][0].upper < roots[1][0].lower && roots[1][0].upper < roots[2][0].lower);
        let collapsed = CoonsPatch::try_new([[0.0; 2]; 12])
            .unwrap()
            .tensor_control_enclosures();
        let mut budget = 100;
        assert_eq!(
            visit_certified_roots(collapsed, [0.0, 0.0], 2, &mut budget, &mut |_| panic!()),
            Err(SearchError::Unresolved)
        );
        let mut budget = 0;
        assert_eq!(
            visit_certified_roots(controls, [1.4, 1.2], 12, &mut budget, &mut |_| panic!()),
            Err(SearchError::WorkLimit)
        );
    }
    #[test]
    fn root_certificate_distinguishes_unique_absent_and_unresolved() {
        let controls = CoonsPatch::try_new(rectangle())
            .unwrap()
            .tensor_control_enclosures();
        let CellCertificate::Unique(root) = certify_interval_cell(&controls, [0.6, 2.4]).unwrap()
        else {
            panic!("interior affine root must certify")
        };
        assert!(root[0].lower <= 0.2 && root[0].upper >= 0.2);
        assert!(root[1].lower <= 0.8 && root[1].upper >= 0.8);
        assert!(matches!(
            certify_interval_cell(&controls, [10.0, 10.0]).unwrap(),
            CellCertificate::NoRoot
        ));
        assert!(matches!(
            certify_interval_cell(&controls, [0.0, 0.0]).unwrap(),
            CellCertificate::Unresolved
        ));
        let collapsed = CoonsPatch::try_new([[0.0; 2]; 12])
            .unwrap()
            .tensor_control_enclosures();
        assert!(matches!(
            certify_interval_cell(&collapsed, [0.0, 0.0]).unwrap(),
            CellCertificate::Unresolved
        ));
    }
    #[test]
    fn polynomial_continuation_moves_corner_root_inside_without_claiming_coverage() {
        let controls = CoonsPatch::try_new(rectangle())
            .unwrap()
            .tensor_control_enclosures();
        let domain = [-0.25, -0.25, 1.25, 1.25];
        assert!(restrict_interval_tensor(controls, domain).is_none());
        let continued = reparameterize_interval_tensor(controls, domain).unwrap();
        let CellCertificate::Unique(root) = certify_interval_cell(&continued, [0.0, 0.0]).unwrap()
        else {
            panic!("continued affine corner should be an interior root")
        };
        for axis in 0..2 {
            let mapped = Interval::exact(domain[axis])
                .add(Interval::exact(domain[axis + 2] - domain[axis]).mul(root[axis]));
            assert!(mapped.lower <= 0.0 && mapped.upper >= 0.0);
        }
        // A root outside the PDF domain also certifies in the continuation:
        // domain membership therefore requires a separate proof.
        assert!(matches!(
            certify_interval_cell(&continued, [-0.3, 1.5]).unwrap(),
            CellCertificate::Unique(_)
        ));
        for invalid in [
            [f64::NAN, 0.0, 1.0, 1.0],
            [0.0, 0.0, f64::INFINITY, 1.0],
            [1.0, 0.0, 0.0, 1.0],
        ] {
            assert!(reparameterize_interval_tensor(controls, invalid).is_none());
        }
    }
    #[test]
    fn polynomial_continuation_encloses_curved_surface_on_original_domain() {
        let mut points = rectangle();
        points[1] = [-0.5, 0.75];
        points[2] = [0.5, 2.25];
        points[4] = [0.75, 4.0];
        points[5] = [2.25, 2.0];
        points[7] = [4.0, 2.25];
        points[8] = [2.0, 0.75];
        let patch = CoonsPatch::try_new(points).unwrap();
        // Width two maps original dyadic samples to exact dyadic samples
        // in the continuation, so this check needs no epsilon allowance.
        let extended = reparameterize_interval_tensor(
            patch.tensor_control_enclosures(),
            [-0.5, -0.5, 1.5, 1.5],
        )
        .unwrap();
        for i in 0..=8 {
            for j in 0..=8 {
                let u = f64::from(i) / 8.0;
                let v = f64::from(j) / 8.0;
                let expected = patch.point(u, v).unwrap();
                let actual = interval_tensor_box(
                    &extended,
                    [
                        Interval::exact((u + 0.5) / 2.0),
                        Interval::exact((v + 0.5) / 2.0),
                    ],
                )
                .unwrap();
                for axis in 0..2 {
                    assert!(
                        actual[axis].lower <= expected[axis]
                            && actual[axis].upper >= expected[axis],
                        "u={u}, v={v}, axis={axis}"
                    );
                }
            }
        }
    }
    #[test]
    fn exact_corner_witness_rejects_nearby_and_uncertain_coordinates() {
        let controls = CoonsPatch::try_new(rectangle())
            .unwrap()
            .tensor_control_enclosures();
        for (uv, point) in [
            ([0.0, 0.0], [0.0, 0.0]),
            ([1.0, 0.0], [3.0, 0.0]),
            ([1.0, 1.0], [3.0, 3.0]),
            ([0.0, 1.0], [0.0, 3.0]),
        ] {
            assert!(exact_parameter_witness(&controls, point, uv).unwrap());
        }
        assert!(!exact_parameter_witness(&controls, [f64::MIN_POSITIVE, 0.0], [0.0, 0.0]).unwrap());
        let mut uncertain = controls;
        uncertain[0][0][0] = Interval {
            lower: -0.001,
            upper: 0.001,
        };
        assert!(!exact_parameter_witness(&uncertain, [0.0, 0.0], [0.0, 0.0]).unwrap());
        assert_eq!(
            exact_parameter_witness(&controls, [0.0, 0.0], [-0.1, 0.0]),
            Err(SearchError::InvalidInput)
        );
        assert_eq!(
            exact_parameter_witness(&controls, [f64::NAN, 0.0], [0.0, 0.0]),
            Err(SearchError::InvalidInput)
        );
    }
    #[test]
    fn unique_corner_requires_global_certificate_and_work_allowance() {
        let controls = CoonsPatch::try_new(rectangle())
            .unwrap()
            .tensor_control_enclosures();
        for (point, uv) in [
            ([0.0, 0.0], [0.0, 0.0]),
            ([3.0, 0.0], [1.0, 0.0]),
            ([3.0, 3.0], [1.0, 1.0]),
            ([0.0, 3.0], [0.0, 1.0]),
        ] {
            let mut remaining = 100;
            let root = certify_unique_corner(controls, point, &mut remaining)
                .unwrap()
                .unwrap();
            for axis in 0..2 {
                assert_eq!(root[axis].lower, uv[axis]);
                assert_eq!(root[axis].upper, uv[axis]);
            }
        }
        // Every parameter maps to this point: a corner witness alone must
        // not suppress other roots or claim global uniqueness.
        let collapsed = CoonsPatch::try_new([[0.0; 2]; 12])
            .unwrap()
            .tensor_control_enclosures();
        assert!(
            certify_unique_corner(collapsed, [0.0, 0.0], &mut 100)
                .unwrap()
                .is_none()
        );
        assert!(
            certify_unique_corner(controls, [1.5, 1.5], &mut 100)
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            certify_unique_corner(controls, [0.0, 0.0], &mut 0),
            Err(SearchError::WorkLimit)
        ));
    }
    #[test]
    fn straight_edge_certificate_proves_non_corner_membership() {
        let controls = CoonsPatch::try_new(rectangle())
            .unwrap()
            .tensor_control_enclosures();
        for (point, uv) in [
            ([0.0, 1.5], [0.0, 0.5]),
            ([3.0, 1.5], [1.0, 0.5]),
            ([0.75, 0.0], [0.25, 0.0]),
            ([0.75, 3.0], [0.25, 1.0]),
        ] {
            let root = certify_unique_straight_edge(controls, point, &mut 100)
                .unwrap()
                .unwrap();
            for axis in 0..2 {
                assert!(root[axis].lower <= uv[axis] && root[axis].upper >= uv[axis]);
                assert!(unit(root[axis].lower) && unit(root[axis].upper));
            }
        }
        assert!(
            certify_unique_straight_edge(controls, [-0.001, 1.5], &mut 100)
                .unwrap()
                .is_none()
        );
        assert!(
            certify_unique_straight_edge(controls, [0.0, 4.0], &mut 100)
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            certify_unique_straight_edge(controls, [0.0, 1.5], &mut 0),
            Err(SearchError::WorkLimit)
        ));
    }
    #[test]
    fn straight_edge_certificate_handles_reflections_and_rejects_uncertainty() {
        for reflected_axis in 0..2 {
            let mut points = rectangle();
            for point in &mut points {
                point[reflected_axis] = 3.0 - point[reflected_axis];
            }
            let controls = CoonsPatch::try_new(points)
                .unwrap()
                .tensor_control_enclosures();
            for (mut target, uv) in [
                ([0.0, 0.75], [0.0, 0.25]),
                ([3.0, 2.25], [1.0, 0.75]),
                ([0.75, 0.0], [0.25, 0.0]),
                ([2.25, 3.0], [0.75, 1.0]),
            ] {
                target[reflected_axis] = 3.0 - target[reflected_axis];
                let root = certify_unique_straight_edge(controls, target, &mut 100)
                    .unwrap()
                    .unwrap();
                for axis in 0..2 {
                    assert!(root[axis].lower <= uv[axis] && root[axis].upper >= uv[axis]);
                }
            }
        }
        let mut controls = CoonsPatch::try_new(rectangle())
            .unwrap()
            .tensor_control_enclosures();
        // An interval containing the target is not an exact constant edge.
        controls[1][0][0] = Interval {
            lower: -0.001,
            upper: 0.001,
        };
        assert!(
            certify_unique_straight_edge(controls, [0.0, 1.5], &mut 100)
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn nonuniform_straight_edge_sampling_preserves_parameter_color() {
        let mut points = rectangle();
        for index in [4, 11] {
            points[index][0] = 1.125;
        }
        for index in [5, 10] {
            points[index][0] = 2.125;
        }
        let patch = ColoredPatch {
            geometry: CoonsPatch::try_new(points).unwrap(),
            values: vec![0.0, 0.0, 1.0, 1.0],
            bounds: vec![
                Interval::exact(0.0),
                Interval::exact(0.0),
                Interval::exact(1.0),
                Interval::exact(1.0),
            ],
        };
        // x(u)=3u+(3/8)u(1-u); both edge targets have exact u=1/4.
        for target in [[0.8203125, 0.0], [0.8203125, 3.0]] {
            let mut values = [-99.0];
            let mut bounds = [Interval::exact(-99.0)];
            assert!(
                patch
                    .sample(target, 20, &mut 10000, &mut values, &mut bounds)
                    .unwrap()
            );
            assert!(bounds[0].lower <= 0.25 && bounds[0].upper >= 0.25);
            assert_eq!((255.0 * values[0]).round() as u8, 64);
            assert_eq!((255.0 * values[0] * values[0]).round() as u8, 16);
        }
    }
    #[test]
    fn candidate_contraction_certifies_split_line_without_subdivision() {
        let mut points = rectangle();
        for index in [4, 11] {
            points[index][0] = 1.125;
        }
        for index in [5, 10] {
            points[index][0] = 2.125;
        }
        let patch = CoonsPatch::try_new(points).unwrap();
        let controls = patch.tensor_control_enclosures();
        let uv = [1.0 / 512.0, 0.6875];
        let target = patch.point(uv[0], uv[1]).unwrap();
        assert!(matches!(
            certify_interval_cell(&controls, target).unwrap(),
            CellCertificate::Unresolved
        ));
        let mut remaining = 10000;
        let CellCertificate::Unique(root) =
            contract_candidate_cell(controls, target, &mut remaining).unwrap()
        else {
            panic!("candidate domain must certify the interior split-line root")
        };
        for axis in 0..2 {
            assert!(root[axis].lower <= uv[axis] && root[axis].upper >= uv[axis]);
        }
        let used = 10000 - remaining;
        for mut allowance in 0..used {
            assert!(matches!(
                contract_candidate_cell(controls, target, &mut allowance),
                Err(SearchError::WorkLimit)
            ));
        }
        let mut calls = 0;
        visit_certified_roots(controls, target, 0, &mut 10000, &mut |_| {
            calls += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(calls, 1);
        let colored = ColoredPatch {
            geometry: patch,
            values: vec![0.0, 0.0, 1.0, 1.0],
            bounds: vec![
                Interval::exact(0.0),
                Interval::exact(0.0),
                Interval::exact(1.0),
                Interval::exact(1.0),
            ],
        };
        let mut values = [-99.0];
        let mut bounds = [Interval::exact(-99.0)];
        let mut remaining = 10000;
        assert!(
            colored
                .sample(target, 0, &mut remaining, &mut values, &mut bounds)
                .unwrap()
        );
        let used = 10000 - remaining;
        for mut allowance in 0..used {
            values = [-99.0];
            bounds = [Interval::exact(-99.0)];
            assert!(matches!(
                colored.sample(target, 0, &mut allowance, &mut values, &mut bounds),
                Err(SearchError::WorkLimit)
            ));
            assert_eq!(values, [-99.0]);
            assert_eq!(bounds[0].lower, -99.0);
            assert_eq!(bounds[0].upper, -99.0);
        }
        assert!(
            colored
                .sample(target, 0, &mut used.clone(), &mut values, &mut bounds)
                .unwrap()
        );
        assert!(bounds[0].lower <= uv[0] && bounds[0].upper >= uv[0]);
        assert!((f64::from(values[0]) - uv[0]).abs() < 1e-9);
        assert!(matches!(
            contract_candidate_cell(controls, [-10.0, 1.0], &mut 10000).unwrap(),
            CellCertificate::NoRoot
        ));
        let collapsed = CoonsPatch::try_new([[0.0; 2]; 12])
            .unwrap()
            .tensor_control_enclosures();
        assert!(matches!(
            contract_candidate_cell(collapsed, [0.0; 2], &mut 10000).unwrap(),
            CellCertificate::Unresolved
        ));
    }
    #[test]
    fn nonuniform_edge_refinement_is_mirror_safe_and_budget_atomic() {
        for mirror in [[false, false], [true, false], [false, true], [true, true]] {
            let mut points = rectangle();
            for index in [4, 11] {
                points[index][0] = 1.125;
            }
            for index in [5, 10] {
                points[index][0] = 2.125;
            }
            for point in &mut points {
                for axis in 0..2 {
                    if mirror[axis] {
                        point[axis] = 3.0 - point[axis];
                    }
                }
            }
            let patch = ColoredPatch {
                geometry: CoonsPatch::try_new(points).unwrap(),
                values: vec![0.0, 0.0, 1.0, 1.0],
                bounds: vec![
                    Interval::exact(0.0),
                    Interval::exact(0.0),
                    Interval::exact(1.0),
                    Interval::exact(1.0),
                ],
            };
            for mut target in [[0.8203125, 0.0], [0.8203125, 3.0]] {
                for axis in 0..2 {
                    if mirror[axis] {
                        target[axis] = 3.0 - target[axis];
                    }
                }
                let mut remaining = 10000;
                let mut values = [-99.0];
                let mut bounds = [Interval::exact(-99.0)];
                assert!(
                    patch
                        .sample(target, 20, &mut remaining, &mut values, &mut bounds)
                        .unwrap()
                );
                let used = 10000 - remaining;
                assert!(used > 8);
                assert_eq!((255.0 * values[0]).round() as u8, 64);
                assert_eq!((255.0 * values[0] * values[0]).round() as u8, 16);
                assert!(bounds[0].lower <= 0.25 && bounds[0].upper >= 0.25);
                // Exhaust every prefix, including the refinement steps.
                for allowance in 0..used {
                    values = [-99.0];
                    bounds = [Interval::exact(-99.0)];
                    assert!(matches!(
                        patch.sample(target, 20, &mut allowance.clone(), &mut values, &mut bounds),
                        Err(SearchError::WorkLimit)
                    ));
                    assert_eq!(values, [-99.0]);
                    assert_eq!(bounds[0].lower, -99.0);
                    assert_eq!(bounds[0].upper, -99.0);
                }
                let mut exact_allowance = used;
                assert!(
                    patch
                        .sample(target, 20, &mut exact_allowance, &mut values, &mut bounds)
                        .unwrap()
                );
                assert_eq!(exact_allowance, 0);
                assert_eq!((255.0 * values[0]).round() as u8, 64);
            }
        }
    }
    #[test]
    fn interval_evaluation_encloses_curved_points_and_preserves_endpoints() {
        let mut patch = CoonsPatch::try_new(rectangle()).unwrap();
        patch.points[4] = [4.0, 6.0];
        patch.points[5] = [-1.0, 6.0];
        let controls = patch.tensor_control_enclosures();
        for u in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for v in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let expected = patch.point(u, v).unwrap();
                let enclosure = interval_tensor_point(&controls, u, v).unwrap();
                for axis in 0..2 {
                    assert!(
                        enclosure[axis].lower <= expected[axis]
                            && enclosure[axis].upper >= expected[axis]
                    );
                }
            }
        }
        let mut uncertain = controls;
        uncertain[0][0][0] = Interval {
            lower: -5.0,
            upper: 10.0,
        };
        let endpoint = interval_tensor_point(&uncertain, 0.0, 0.0).unwrap();
        assert_eq!(endpoint[0].lower, -5.0);
        assert_eq!(endpoint[0].upper, 10.0);
        assert!(interval_tensor_point(&controls, f64::NAN, 0.5).is_none());
        assert!(interval_tensor_point(&controls, 0.5, 1.1).is_none());
    }
    #[test]
    fn interval_search_retains_fold_roots_without_manual_padding() {
        let mut patch = CoonsPatch::try_new(rectangle()).unwrap();
        patch.points[11][0] = 4.0;
        patch.points[10][0] = -1.0;
        patch.points[4][0] = 4.0;
        patch.points[5][0] = -1.0;
        let controls = patch.tensor_control_enclosures();
        let mut budget = 10000;
        let mut cells = Vec::new();
        visit_interval_candidates(controls, [1.5, 1.5], 8, &mut budget, &mut |child, rect| {
            let bounds = interval_tensor_bounds(&child).unwrap();
            assert!(bounds.iter().all(|b| b.lower <= 1.5 && b.upper >= 1.5));
            cells.push(rect);
            Ok(())
        })
        .unwrap();
        for u in [0.5 - 3.0_f64.sqrt() / 6.0, 0.5, 0.5 + 3.0_f64.sqrt() / 6.0] {
            assert!(
                cells
                    .iter()
                    .any(|r| r[0] <= u && u <= r[2] && r[1] <= 0.5 && 0.5 <= r[3])
            );
        }
        let mut budget = 1;
        visit_interval_candidates(
            controls,
            [100.0, 100.0],
            8,
            &mut budget,
            &mut |_, _| panic!(),
        )
        .unwrap();
        let mut budget = 10000;
        assert_eq!(
            visit_interval_candidates(controls, [1.5, 1.5], 8, &mut budget, &mut |_, _| Err(
                SearchError::Unresolved
            )),
            Err(SearchError::Unresolved)
        );
    }
    #[test]
    fn interval_hulls_and_derivatives_retain_uncertainty_and_local_scaling() {
        let patch = CoonsPatch::try_new(rectangle()).unwrap();
        let controls = patch.tensor_control_enclosures();
        let child = subdivide_interval_tensor(controls).unwrap()[0];
        let bounds = interval_tensor_bounds(&child).unwrap();
        for axis in 0..2 {
            assert!(bounds[axis].lower <= 0.0 && bounds[axis].upper >= 1.5);
        }
        let derivatives = interval_tensor_derivatives(&child).unwrap();
        for (d, expected) in [[1.5, 0.0], [0.0, 1.5]].into_iter().enumerate() {
            for axis in 0..2 {
                assert!(derivatives[d][axis].lower <= expected[axis]);
                assert!(derivatives[d][axis].upper >= expected[axis]);
            }
        }
        let mut uncertain = controls;
        uncertain[1][1][0] = Interval {
            lower: -10.0,
            upper: 20.0,
        };
        let hull = interval_tensor_bounds(&uncertain).unwrap();
        assert!(hull[0].lower <= -10.0 && hull[0].upper >= 20.0);
        let derivative = interval_tensor_derivatives(&uncertain).unwrap();
        assert!(derivative[0][0].lower < -20.0 && derivative[0][0].upper > 20.0);
        uncertain[1][1][0] = Interval {
            lower: 2.0,
            upper: 1.0,
        };
        assert!(interval_tensor_bounds(&uncertain).is_none());
        assert!(interval_tensor_derivatives(&uncertain).is_none());
    }
    #[test]
    fn interval_subdivision_carries_conversion_error_across_levels() {
        let patch = CoonsPatch::try_new(rectangle()).unwrap();
        let first = subdivide_interval_tensor(patch.tensor_control_enclosures()).unwrap();
        let second = subdivide_interval_tensor(first[0]).unwrap();
        // Child 3 of child 0 covers [.25,.5] in both global parameters.
        for j in 0..4 {
            for i in 0..4 {
                for (axis, exact) in [0.75 + i as f64 * 0.25, 0.75 + j as f64 * 0.25]
                    .into_iter()
                    .enumerate()
                {
                    assert!(second[3][j][i][axis].lower <= exact);
                    assert!(second[3][j][i][axis].upper >= exact);
                }
            }
        }
        for i in 0..4 {
            for axis in 0..2 {
                assert_eq!(first[0][i][3][axis].lower, first[1][i][0][axis].lower);
                assert_eq!(first[0][i][3][axis].upper, first[1][i][0][axis].upper);
            }
        }
        let mut invalid = patch.tensor_control_enclosures();
        invalid[0][0][0] = Interval {
            lower: 1.0,
            upper: 0.0,
        };
        assert!(subdivide_interval_tensor(invalid).is_none());
    }
    #[test]
    fn conversion_intervals_enclose_exact_rectangle_and_curved_controls() {
        let mut patch = CoonsPatch::try_new(rectangle()).unwrap();
        let intervals = patch.tensor_control_enclosures();
        for j in 0..4 {
            for i in 0..4 {
                for (axis, expected) in [i as f64, j as f64].into_iter().enumerate() {
                    assert!(
                        intervals[j][i][axis].lower <= expected
                            && intervals[j][i][axis].upper >= expected
                    );
                }
            }
        }
        patch.points[4] = [4.0, 6.0];
        patch.points[10] = [-1.0, 0.0];
        let intervals = patch.tensor_control_enclosures();
        let controls = patch.tensor_controls().unwrap();
        for j in 0..4 {
            for i in 0..4 {
                for axis in 0..2 {
                    assert!(intervals[j][i][axis].lower <= controls[j][i][axis]);
                    assert!(intervals[j][i][axis].upper >= controls[j][i][axis]);
                }
            }
        }
        let whole = Interval {
            lower: f64::NEG_INFINITY,
            upper: f64::INFINITY,
        };
        let product = whole.mul(Interval::exact(0.0));
        assert_eq!(product.lower, f64::NEG_INFINITY);
        assert_eq!(product.upper, f64::INFINITY);
    }
    #[test]
    fn unresolved_candidate_is_not_a_successful_empty_search() {
        let patch = CoonsPatch::try_new([[0.0; 2]; 12]).unwrap();
        let controls = patch.tensor_controls().unwrap();
        let mut search_budget = 100;
        let mut refinement_budget = 100;
        // Padding retains a candidate whose degenerate Jacobian cannot
        // refine the nearby target. It must not disappear as "no roots".
        let result = visit_candidates(
            controls,
            [0.1, 0.0],
            0.2,
            0,
            &mut search_budget,
            &mut |rect| {
                patch
                    .refine_candidate([0.1, 0.0], rect, 1e-12, &mut refinement_budget)?
                    .ok_or(SearchError::Unresolved)?;
                Ok(())
            },
        );
        assert_eq!(result, Err(SearchError::Unresolved));
    }
    #[test]
    fn subdivision_isolates_positive_and_negative_fold_regions() {
        let mut patch = CoonsPatch::try_new(rectangle()).unwrap();
        patch.points[11][0] = 4.0;
        patch.points[10][0] = -1.0;
        patch.points[4][0] = 4.0;
        patch.points[5][0] = -1.0;
        let halves = subdivide_tensor(patch.tensor_controls().unwrap()).unwrap();
        let quarters = subdivide_tensor(halves[0]).unwrap();
        let eighths = subdivide_tensor(quarters[1]).unwrap();
        // u=[0,.25] is positively oriented; u=[.375,.5] negatively.
        assert!(tensor_jacobian_bounds(&quarters[0]).unwrap()[0] > 0.0);
        assert!(tensor_jacobian_bounds(&eighths[1]).unwrap()[1] < 0.0);
    }
    #[test]
    fn jacobian_enclosure_distinguishes_regular_folded_and_degenerate_nets() {
        let mut patch = CoonsPatch::try_new(rectangle()).unwrap();
        let regular = tensor_jacobian_bounds(&patch.tensor_controls().unwrap()).unwrap();
        assert!(regular[0] > 0.0 && regular[0] <= 9.0 && regular[1] >= 9.0);
        patch.points[11][0] = 4.0;
        patch.points[10][0] = -1.0;
        patch.points[4][0] = 4.0;
        patch.points[5][0] = -1.0;
        let folded = tensor_jacobian_bounds(&patch.tensor_controls().unwrap()).unwrap();
        assert!(folded[0] < 0.0 && folded[1] > 0.0);
        for u in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let [du, dv] = patch.derivatives(u, 0.5).unwrap();
            let determinant = du[0] * dv[1] - du[1] * dv[0];
            assert!(determinant >= folded[0] && determinant <= folded[1]);
        }
        let collapsed = tensor_jacobian_bounds(&[[[0.0; 2]; 4]; 4]).unwrap();
        assert!(collapsed[0] <= 0.0 && collapsed[1] >= 0.0);
        let mut extreme = [[[0.0; 2]; 4]; 4];
        extreme[0][0] = [f64::MAX, -f64::MAX];
        extreme[0][1] = [-f64::MAX, f64::MAX];
        assert_eq!(
            tensor_jacobian_bounds(&extreme),
            Some([f64::NEG_INFINITY, f64::INFINITY])
        );
    }
    #[test]
    fn derivative_hulls_cover_analytic_derivatives_and_fold_sign_changes() {
        let mut patch = CoonsPatch::try_new(rectangle()).unwrap();
        let rectangular = tensor_derivative_bounds(&patch.tensor_controls().unwrap()).unwrap();
        for (direction, vector) in [[3.0, 0.0], [0.0, 3.0]].into_iter().enumerate() {
            for axis in 0..2 {
                assert!(rectangular[direction][axis][0] <= vector[axis]);
                assert!(rectangular[direction][axis][1] >= vector[axis]);
            }
        }
        patch.points[11][0] = 4.0;
        patch.points[10][0] = -1.0;
        patch.points[4] = [4.0, 6.0];
        patch.points[5] = [-1.0, 6.0];
        let bounds = tensor_derivative_bounds(&patch.tensor_controls().unwrap()).unwrap();
        assert!(bounds[0][0][0] < 0.0 && bounds[0][0][1] > 0.0);
        for u in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for v in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let derivatives = patch.derivatives(u, v).unwrap();
                for d in 0..2 {
                    for axis in 0..2 {
                        assert!(derivatives[d][axis] >= bounds[d][axis][0]);
                        assert!(derivatives[d][axis] <= bounds[d][axis][1]);
                    }
                }
            }
        }
        let mut invalid = [[[0.0; 2]; 4]; 4];
        invalid[1][1][0] = f64::NAN;
        assert!(tensor_derivative_bounds(&invalid).is_none());
    }
    #[test]
    fn callback_error_aborts_search() {
        let controls = CoonsPatch::try_new(rectangle())
            .unwrap()
            .tensor_controls()
            .unwrap();
        let mut budget = 100;
        let mut calls = 0;
        let result = visit_candidates(controls, [1.5, 1.5], 0.0, 1, &mut budget, &mut |_| {
            calls += 1;
            Err(SearchError::WorkLimit)
        });
        assert_eq!(result, Err(SearchError::WorkLimit));
        assert_eq!(calls, 1);
        assert!(budget > 0);
    }
    #[test]
    fn parameter_precedence_is_v_then_u_without_epsilon_ties() {
        assert_eq!(preferred_parameter(&[]), Ok(None));
        let roots = [[0.9, 0.4], [0.1, 0.8], [0.7, 0.8], [0.7, 0.8]];
        assert_eq!(preferred_parameter(&roots), Ok(Some([0.7, 0.8])));
        let reverse = [roots[3], roots[2], roots[1], roots[0]];
        assert_eq!(preferred_parameter(&reverse), preferred_parameter(&roots));
        let v = 0.5_f64.next_up();
        assert_eq!(
            preferred_parameter(&[[1.0, 0.5], [0.0, v]]),
            Ok(Some([0.0, v]))
        );
        for bad in [
            [f64::NAN, 0.5],
            [0.5, f64::INFINITY],
            [-0.1, 0.5],
            [0.5, 1.1],
        ] {
            assert_eq!(
                preferred_parameter(&[[0.5, 0.5], bad]),
                Err(SearchError::InvalidInput)
            );
        }
    }
    #[test]
    fn refinement_recovers_three_distinct_fold_parameters() {
        let mut patch = CoonsPatch::try_new(rectangle()).unwrap();
        patch.points[11][0] = 4.0;
        patch.points[10][0] = -1.0;
        patch.points[4][0] = 4.0;
        patch.points[5][0] = -1.0;
        let offset = 3.0_f64.sqrt() / 6.0;
        for (rect, expected_u) in [
            ([0.15, 0.4, 0.3, 0.6], 0.5 - offset),
            ([0.45, 0.4, 0.55, 0.6], 0.5),
            ([0.7, 0.4, 0.85, 0.6], 0.5 + offset),
        ] {
            let mut budget = 1024;
            let uv = patch
                .refine_candidate([1.5, 1.5], rect, 1e-12, &mut budget)
                .unwrap()
                .unwrap();
            assert!((uv[0] - expected_u).abs() < 1e-10);
            assert!((uv[1] - 0.5).abs() < 1e-12);
            let point = patch.point(uv[0], uv[1]).unwrap();
            assert!((point[0] - 1.5).abs() <= 1e-12 && (point[1] - 1.5).abs() <= 1e-12);
        }
        let mut search_budget = 10000;
        let mut refinement_budget = 100000;
        let mut roots = Vec::new();
        visit_candidates(
            patch.tensor_controls().unwrap(),
            [1.5, 1.5],
            1e-12,
            8,
            &mut search_budget,
            &mut |rect| {
                if let Some(uv) =
                    patch.refine_candidate([1.5, 1.5], rect, 1e-12, &mut refinement_budget)?
                {
                    roots.push(uv);
                }
                Ok(())
            },
        )
        .unwrap();
        for expected_u in [0.5 - offset, 0.5, 0.5 + offset] {
            assert!(
                roots
                    .iter()
                    .any(|uv| (uv[0] - expected_u).abs() < 1e-10 && (uv[1] - 0.5).abs() < 1e-10)
            );
        }
        // A collapsed patch has infinitely many roots for its point. A local
        // result is legitimate, but does not certify all roots were found.
        let collapsed = CoonsPatch::try_new([[2.0, 3.0]; 12]).unwrap();
        let mut budget = 1;
        assert_eq!(
            collapsed.refine_candidate([2.0, 3.0], [0.0, 0.0, 1.0, 1.0], 1e-12, &mut budget),
            Ok(Some([0.5, 0.5]))
        );
    }
    #[test]
    fn local_refinement_checks_residual_domain_and_budget() {
        let patch = CoonsPatch::try_new(rectangle()).unwrap();
        let mut budget = 100;
        let uv = patch
            .refine_candidate([0.6, 2.4], [0.0, 0.0, 1.0, 1.0], 1e-12, &mut budget)
            .unwrap()
            .unwrap();
        assert!((uv[0] - 0.2).abs() < 1e-12 && (uv[1] - 0.8).abs() < 1e-12);
        let mut budget = 1024;
        assert_eq!(
            patch.refine_candidate([0.6, 2.4], [0.5, 0.0, 1.0, 1.0], 1e-12, &mut budget),
            Ok(None)
        );
        let mut zero = 0;
        assert_eq!(
            patch.refine_candidate([0.6, 2.4], [0.0, 0.0, 1.0, 1.0], 1e-12, &mut zero),
            Err(SearchError::WorkLimit)
        );
        assert_eq!(
            patch.refine_candidate([0.6, 2.4], [0.0, 0.0, 1.0, 1.0], 0.0, &mut zero),
            Err(SearchError::InvalidInput)
        );
        let singular = CoonsPatch::try_new([[0.0; 2]; 12]).unwrap();
        let mut budget = 100;
        assert_eq!(
            singular.refine_candidate([1.0, 0.0], [0.0, 0.0, 1.0, 1.0], 1e-12, &mut budget),
            Ok(None)
        );
    }
    #[test]
    fn candidate_search_retains_all_fold_roots_and_reports_work_exhaustion() {
        let mut patch = CoonsPatch::try_new(rectangle()).unwrap();
        patch.points[11][0] = 4.0;
        patch.points[10][0] = -1.0;
        patch.points[4][0] = 4.0;
        patch.points[5][0] = -1.0;
        let controls = patch.tensor_controls().unwrap();
        let mut cells = Vec::new();
        let mut budget = 10000;
        visit_candidates(controls, [1.5, 1.5], 1e-12, 8, &mut budget, &mut |r| {
            cells.push(r);
            Ok(())
        })
        .unwrap();
        for u in [0.5 - 3.0_f64.sqrt() / 6.0, 0.5, 0.5 + 3.0_f64.sqrt() / 6.0] {
            assert!(
                cells
                    .iter()
                    .any(|r| r[0] <= u && u <= r[2] && r[1] <= 0.5 && 0.5 <= r[3])
            );
        }
        let mut zero = 0;
        assert_eq!(
            visit_candidates(controls, [1.5, 1.5], 0.0, 8, &mut zero, &mut |_| panic!()),
            Err(SearchError::WorkLimit)
        );
        let mut one = 1;
        visit_candidates(
            controls,
            [100.0, 100.0],
            0.0,
            8,
            &mut one,
            &mut |_| panic!(),
        )
        .unwrap();
        assert_eq!(one, 0);
        assert_eq!(
            visit_candidates(controls, [1.5, 1.5], -1.0, 8, &mut one, &mut |_| panic!()),
            Err(SearchError::InvalidInput)
        );
    }
    #[test]
    fn control_hull_bounds_cover_bowed_interior_and_subdivisions() {
        let mut patch = CoonsPatch::try_new(rectangle()).unwrap();
        patch.points[4][1] = 12.0;
        patch.points[5][1] = 12.0;
        let controls = patch.tensor_controls().unwrap();
        let bounds = tensor_bounds(&controls).unwrap();
        let center = patch.point(0.5, 0.75).unwrap();
        assert!(center[1] > 3.0); // Outside the original corner AABB.
        assert!(center[1] <= bounds[3]);
        for child in subdivide_tensor(controls).unwrap() {
            let child_bounds = tensor_bounds(&child).unwrap();
            for u in [0.0, 0.25, 0.5, 0.75, 1.0] {
                for v in [0.0, 0.25, 0.5, 0.75, 1.0] {
                    let p = cubic(child.map(|row| cubic(row, u)), v);
                    for axis in 0..2 {
                        assert!(p[axis] >= child_bounds[axis]);
                        assert!(p[axis] <= child_bounds[axis + 2]);
                    }
                }
            }
        }
        let mut invalid = controls;
        invalid[2][1][1] = f64::INFINITY;
        assert!(tensor_bounds(&invalid).is_none());
        let zero_bounds = tensor_bounds(&[[[0.0; 2]; 4]; 4]).unwrap();
        assert!(zero_bounds[0] < 0.0 && zero_bounds[2] > 0.0);
        // Infinite outward bounds are conservative, not invalid input.
        assert_eq!(
            tensor_bounds(&[[[f64::MAX; 2]; 4]; 4]).unwrap()[2],
            f64::INFINITY
        );
    }
    #[test]
    fn subdivision_preserves_parent_parameters_and_shared_edges() {
        let mut patch = CoonsPatch::try_new(rectangle()).unwrap();
        patch.points[4][1] = 6.0;
        patch.points[11][0] = 4.0;
        patch.points[10][0] = -1.0;
        let controls = patch.tensor_controls().unwrap();
        let children = subdivide_tensor(controls).unwrap();
        for (index, child) in children.iter().enumerate() {
            for u in [0.0, 0.17, 0.5, 1.0] {
                for v in [0.0, 0.31, 0.5, 1.0] {
                    let actual = cubic(child.map(|row| cubic(row, u)), v);
                    let expected = patch
                        .point(
                            (u + (index % 2) as f64) * 0.5,
                            (v + (index / 2) as f64) * 0.5,
                        )
                        .unwrap();
                    for axis in 0..2 {
                        assert!((actual[axis] - expected[axis]).abs() < 1e-12);
                    }
                }
            }
        }
        for i in 0..4 {
            assert_eq!(children[0][i][3], children[1][i][0]);
            assert_eq!(children[0][3][i], children[2][0][i]);
        }
        let huge = [[[f64::MAX; 2]; 4]; 4];
        assert!(
            subdivide_tensor(huge)
                .unwrap()
                .iter()
                .flatten()
                .flatten()
                .flatten()
                .all(|x| x.is_finite())
        );
        let mut invalid = controls;
        invalid[0][0][0] = f64::NAN;
        assert!(subdivide_tensor(invalid).is_none());
    }
    #[test]
    fn tensor_representation_preserves_curved_and_folded_surface() {
        let mut patch = CoonsPatch::try_new(rectangle()).unwrap();
        patch.points[11][0] = 4.0;
        patch.points[10][0] = -1.0;
        patch.points[4] = [4.0, 6.0];
        patch.points[5] = [-1.0, 6.0];
        let controls = patch.tensor_controls().unwrap();
        for u in [0.0, 0.13, 0.5, 0.91, 1.0] {
            for v in [0.0, 0.27, 0.5, 0.83, 1.0] {
                let rows = controls.map(|row| cubic(row, u));
                let actual = cubic(rows, v);
                let expected = patch.point(u, v).unwrap();
                for axis in 0..2 {
                    assert!((actual[axis] - expected[axis]).abs() < 1e-12);
                }
            }
        }
    }
    #[test]
    fn corner_components_and_parameters_interpolate_before_conversion() {
        let corners = [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
        ];
        let references = corners.each_ref().map(|c| c.as_slice());
        let mut rgb = [-1.0; 3];
        assert_eq!(
            interpolate_corners(references, 0.25, 0.75, &mut rgb),
            Some(())
        );
        assert_eq!(rgb, [0.25, 0.625, 0.25]);
        let parameters = [[0.0], [1.0], [0.0], [1.0]];
        let mut t = [-1.0];
        assert!(
            interpolate_corners(
                parameters.each_ref().map(|c| c.as_slice()),
                0.25,
                0.75,
                &mut t
            )
            .is_some()
        );
        assert_eq!(t, [0.625]);
        assert_eq!(t[0] * t[0], 0.390625);
        assert!(interpolate_corners(references, -0.1, 0.75, &mut rgb).is_none());
        assert_eq!(rgb, [0.25, 0.625, 0.25]);
        assert!(interpolate_corners(references, 0.25, 0.75, &mut t).is_none());
        assert_eq!(t, [0.625]);
        let mut invalid = corners;
        invalid[2][1] = f32::NAN;
        assert!(
            interpolate_corners(
                invalid.each_ref().map(|c| c.as_slice()),
                0.25,
                0.75,
                &mut rgb
            )
            .is_none()
        );
        assert_eq!(rgb, [0.25, 0.625, 0.25]);
    }
    #[test]
    fn folded_surface_retains_multiple_parameter_preimages() {
        let mut p = rectangle();
        // Identical horizontal curves x(u)=12u-27u^2+18u^3 fold twice.
        p[11][0] = 4.0;
        p[10][0] = -1.0;
        p[4][0] = 4.0;
        p[5][0] = -1.0;
        let patch = CoonsPatch::try_new(p).unwrap();
        let offset = 3.0_f64.sqrt() / 6.0;
        for u in [0.5 - offset, 0.5, 0.5 + offset] {
            close(patch.point(u, 0.5).unwrap(), [1.5, 1.5]);
        }
        assert!(patch.derivatives(0.5, 0.5).unwrap()[0][0] < 0.0);
        assert!(patch.derivatives(0.5 - offset, 0.5).unwrap()[0][0] > 0.0);
    }

    #[test]
    fn curved_interior_is_not_corner_bilinear_geometry() {
        let mut p = rectangle();
        p[4][1] = 6.0;
        p[5][1] = 6.0;
        let patch = CoonsPatch::try_new(p).unwrap();
        close(patch.point(0.5, 0.5).unwrap(), [1.5, 2.625]);
        assert_ne!(patch.point(0.5, 0.5).unwrap(), [1.5, 1.5]);
    }

    #[test]
    fn analytic_derivatives_match_rectangle_and_curved_finite_differences() {
        let patch = CoonsPatch::try_new(rectangle()).unwrap();
        assert_eq!(
            patch.derivatives(0.25, 0.75),
            Some([[3.0, 0.0], [0.0, 3.0]])
        );
        assert!(patch.derivatives(f64::NAN, 0.5).is_none());
        let mut p = rectangle();
        p[1][0] = -1.0;
        p[4][1] = 4.0;
        p[7][0] = 4.0;
        p[10][1] = -1.0;
        let curved = CoonsPatch::try_new(p).unwrap();
        let epsilon = 1e-5;
        for (u, v) in [(0.25, 0.25), (0.5, 0.5), (0.75, 0.25)] {
            let analytic = curved.derivatives(u, v).unwrap();
            for (direction, minus, plus) in [
                (
                    0,
                    curved.point(u - epsilon, v).unwrap(),
                    curved.point(u + epsilon, v).unwrap(),
                ),
                (
                    1,
                    curved.point(u, v - epsilon).unwrap(),
                    curved.point(u, v + epsilon).unwrap(),
                ),
            ] {
                for axis in 0..2 {
                    let numerical = (plus[axis] - minus[axis]) / (2.0 * epsilon);
                    assert!((numerical - analytic[direction][axis]).abs() < 1e-8);
                }
            }
        }
    }
    #[test]
    fn shared_edge_flags_preserve_controls_corners_and_curve_direction() {
        let mut points = rectangle();
        points[4][1] = 4.0;
        points[7][0] = 4.0;
        points[10][1] = -1.0;
        let patch = CoonsPatch::try_new(points).unwrap();
        for flag in 1..=3 {
            let (edge, colors) = patch.shared_edge(flag).unwrap();
            let start = usize::from(flag) * 3;
            for i in 0..4 {
                assert_eq!(edge[i], points[(start + i) % 12]);
            }
            assert_eq!(colors, [usize::from(flag), (usize::from(flag) + 1) % 4]);
            let next = patch
                .continue_from(flag, rectangle()[4..].try_into().unwrap())
                .unwrap();
            for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let previous = match flag {
                    1 => patch.point(t, 1.0),
                    2 => patch.point(1.0, 1.0 - t),
                    _ => patch.point(1.0 - t, 0.0),
                }
                .unwrap();
                close(next.point(0.0, t).unwrap(), previous);
            }
        }
        assert!(patch.shared_edge(0).is_none());
        assert!(patch.shared_edge(4).is_none());
        let mut tail: [[f64; 2]; 8] = rectangle()[4..].try_into().unwrap();
        tail[0][0] = f64::NAN;
        assert!(patch.continue_from(1, tail).is_none());
    }
    fn rectangle() -> [[f64; 2]; 12] {
        [
            [0.0, 0.0],
            [0.0, 1.0],
            [0.0, 2.0],
            [0.0, 3.0],
            [1.0, 3.0],
            [2.0, 3.0],
            [3.0, 3.0],
            [3.0, 2.0],
            [3.0, 1.0],
            [3.0, 0.0],
            [2.0, 0.0],
            [1.0, 0.0],
        ]
    }
    fn close(a: [f64; 2], b: [f64; 2]) {
        for axis in 0..2 {
            assert!((a[axis] - b[axis]).abs() < 1e-12, "{a:?} != {b:?}");
        }
    }
    #[test]
    fn rectangle_and_corner_order() {
        let patch = CoonsPatch::try_new(rectangle()).unwrap();
        close(patch.point(0.25, 0.75).unwrap(), [0.75, 2.25]);
        for (u, v, index) in [(0.0, 0.0, 0), (0.0, 1.0, 1), (1.0, 1.0, 2), (1.0, 0.0, 3)] {
            let weights = corner_weights(u, v).unwrap();
            assert_eq!(weights[index], 1.0);
            assert_eq!(weights.iter().sum::<f64>(), 1.0);
        }
        assert_eq!(
            corner_weights(0.25, 0.75),
            Some([0.1875, 0.5625, 0.1875, 0.0625])
        );
    }
    #[test]
    fn curved_boundaries_are_interpolated_in_stream_order() {
        let mut p = rectangle();
        p[1][0] = -1.0;
        p[4][1] = 4.0;
        p[7][0] = 4.0;
        p[10][1] = -1.0;
        let patch = CoonsPatch::try_new(p).unwrap();
        for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
            close(
                patch.point(0.0, t).unwrap(),
                cubic([p[0], p[1], p[2], p[3]], t),
            );
            close(
                patch.point(t, 1.0).unwrap(),
                cubic([p[3], p[4], p[5], p[6]], t),
            );
            close(
                patch.point(1.0, t).unwrap(),
                cubic([p[9], p[8], p[7], p[6]], t),
            );
            close(
                patch.point(t, 0.0).unwrap(),
                cubic([p[0], p[11], p[10], p[9]], t),
            );
        }
    }
    #[test]
    fn rejects_nonfinite_controls_and_outside_parameters() {
        let mut bad = rectangle();
        bad[5][0] = f64::INFINITY;
        assert!(CoonsPatch::try_new(bad).is_none());
        let patch = CoonsPatch::try_new(rectangle()).unwrap();
        for (u, v) in [(-0.1, 0.5), (0.5, 1.1), (f64::NAN, 0.5)] {
            assert!(patch.point(u, v).is_none());
        }
    }
}
