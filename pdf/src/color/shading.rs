use std::io::{Read, Seek};

use crate::{
    color::{
        colorspace::ColorSpace,
        value::{ColorRgb, ColorValue},
    },
    error::{PdfError, PdfResult},
    function::{Function, create_function_at_depth},
    objects::PdfObject,
    pdf_context::PDFContext,
};

#[derive(Debug, Clone, Copy)]
pub enum ShadingGeometry {
    FunctionBased { domain: [f32; 4], matrix: [f32; 6] },
    Axial([f32; 4]),
    Radial([f32; 6]),
}

/// Function-based, axial and radial shading, independent of page graphics state.
#[derive(Debug, Clone)]
pub struct Shading {
    geometry: ShadingGeometry,
    colorspace: ColorSpace,
    functions: Vec<Function>,
    domain: [f32; 2],
    extend: [bool; 2],
    bbox: Option<[f32; 4]>,
    background: Option<ColorValue>,
    anti_alias: bool,
    retained_payload_bytes: usize,
}

fn invalid(message: &str) -> PdfError {
    PdfError::ColorError(format!("shading {message}"))
}

fn numbers<R: Read + Seek, const N: usize>(
    object: &PdfObject,
    ctx: &PDFContext<R>,
) -> PdfResult<[f32; N]> {
    let resolved = ctx.resolve(object)?;
    let array = resolved
        .as_array()
        .filter(|array| array.len() == N)
        .ok_or_else(|| invalid("numeric array dimension mismatch"))?;
    let mut values = [0.0; N];
    for (value, object) in values.iter_mut().zip(array) {
        *value = ctx
            .resolve(object)?
            .as_f32()
            .filter(|value| value.is_finite())
            .ok_or_else(|| invalid("parameters must be finite numbers"))?;
    }
    Ok(values)
}

impl Shading {
    pub fn try_new<R: Read + Seek>(object: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let object = ctx.resolve(object)?;
        let dict = object
            .as_dict()
            .ok_or_else(|| invalid("requires a dictionary"))?;
        let required = |key: &str| {
            dict.get(key)
                .ok_or_else(|| invalid(&format!("missing {key}")))
        };
        let kind = ctx.resolve(required("ShadingType")?)?;
        let kind = kind
            .as_number()
            .ok_or_else(|| invalid("ShadingType must be an integer"))?
            .as_u32_checked()?;
        let geometry = match kind {
            1 => {
                let domain: [f32; 4] = dict
                    .get("Domain")
                    .map(|value| numbers(value, ctx))
                    .transpose()?
                    .unwrap_or([0.0, 1.0, 0.0, 1.0]);
                if domain[0] > domain[1] || domain[2] > domain[3] {
                    return Err(invalid("two-dimensional Domain must be ordered"));
                }
                let matrix = dict
                    .get("Matrix")
                    .map(|value| numbers(value, ctx))
                    .transpose()?
                    .unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
                ShadingGeometry::FunctionBased { domain, matrix }
            }
            2 => ShadingGeometry::Axial(numbers(required("Coords")?, ctx)?),
            3 => {
                let values = numbers(required("Coords")?, ctx)?;
                if values[2] < 0.0 || values[5] < 0.0 {
                    return Err(invalid("radii must be nonnegative"));
                }
                ShadingGeometry::Radial(values)
            }
            _ => return Err(PdfError::UnsupportedFeature(format!("ShadingType {kind}"))),
        };
        let color_object = ctx.resolve(required("ColorSpace")?)?;
        if color_object.as_name().is_some_and(|name| {
            !matches!(
                name.name(),
                "DeviceGray" | "DeviceRGB" | "DeviceCMYK" | "G" | "RGB" | "CMYK" | "Pattern"
            )
        }) {
            return Err(invalid(
                "ColorSpace must be an explicit definition, not a resource alias",
            ));
        }
        let colorspace = ColorSpace::try_new(&color_object, ctx)?;
        if matches!(colorspace, ColorSpace::Pattern(_) | ColorSpace::Indexed(_)) {
            return Err(invalid(
                "axial/radial ColorSpace must not be Pattern or Indexed",
            ));
        }
        let components = colorspace.number_of_components()?;
        let domain = if let ShadingGeometry::FunctionBased { domain, .. } = geometry {
            [domain[0], domain[1]]
        } else {
            dict.get("Domain")
                .map(|object| numbers(object, ctx))
                .transpose()?
                .unwrap_or([0.0, 1.0])
        };
        if domain[0] > domain[1] {
            return Err(invalid("Domain must be ordered"));
        }
        let mut extend = [false; 2];
        if let Some(object) = dict.get("Extend") {
            let object = ctx.resolve(object)?;
            let array = object
                .as_array()
                .filter(|array| array.len() == 2)
                .ok_or_else(|| invalid("Extend requires two booleans"))?;
            for (target, value) in extend.iter_mut().zip(array) {
                *target = ctx
                    .resolve(value)?
                    .as_bool()
                    .ok_or_else(|| invalid("Extend requires booleans"))?
                    .value();
            }
        }
        let bbox = dict
            .get("BBox")
            .map(|object| numbers(object, ctx))
            .transpose()?;
        if bbox.is_some_and(|b| b[0] > b[2] || b[1] > b[3]) {
            return Err(invalid("BBox must be ordered"));
        }
        let anti_alias = match dict.get("AntiAlias") {
            None => false,
            Some(object) => ctx
                .resolve(object)?
                .as_bool()
                .ok_or_else(|| invalid("AntiAlias must be boolean"))?
                .value(),
        };
        let function = ctx.resolve(required("Function")?)?;
        let count = function.as_array().map_or(1, |array| array.len());
        if function.as_array().is_some() && count != components {
            return Err(invalid("Function array must match color components"));
        }
        if count > ctx.limits().max_objects
            || count
                .checked_mul(std::mem::size_of::<Function>())
                .is_none_or(|bytes| bytes > ctx.limits().max_decoded_bytes)
        {
            return Err(PdfError::ResourceLimit("shading functions".into()));
        }
        let mut functions = Vec::new();
        functions
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("shading function allocation".into()))?;
        let mut remaining = ctx.limits().max_objects;
        let inputs = if kind == 1 { 2 } else { 1 };
        if let Some(array) = function.as_array() {
            for object in array {
                let function = create_function_at_depth(object, ctx, 0, &mut remaining)?;
                if function.dimensions() != (inputs, 1) {
                    return Err(invalid(
                        "component function input/output dimensions do not match shading",
                    ));
                }
                functions.push(function);
            }
        } else {
            let function = create_function_at_depth(&function, ctx, 0, &mut remaining)?;
            if function.dimensions() != (inputs, components) {
                return Err(invalid("function dimensions must match ColorSpace"));
            }
            functions.push(function);
        }
        if let ShadingGeometry::FunctionBased { domain, .. } = geometry {
            for function in &functions {
                let input_domain = function
                    .two_input_domain()
                    .filter(|values| values.len() == 4)
                    .ok_or_else(|| invalid("Type1 function requires a two-dimensional Domain"))?;
                if input_domain[0] > domain[0]
                    || input_domain[1] < domain[1]
                    || input_domain[2] > domain[2]
                    || input_domain[3] < domain[3]
                {
                    return Err(invalid("function Domain must contain shading Domain"));
                }
            }
        }
        let background = if let Some(object) = dict.get("Background") {
            let object = ctx.resolve(object)?;
            let array = object
                .as_array()
                .filter(|array| array.len() == components)
                .ok_or_else(|| invalid("Background component count mismatch"))?;
            if components
                .checked_mul(std::mem::size_of::<f32>())
                .is_none_or(|bytes| bytes > ctx.limits().max_decoded_bytes)
            {
                return Err(PdfError::ResourceLimit("shading background".into()));
            }
            let mut values = Vec::new();
            values
                .try_reserve_exact(components)
                .map_err(|_| PdfError::ResourceLimit("shading background allocation".into()))?;
            for value in array {
                values.push(
                    ctx.resolve(value)?
                        .as_f32()
                        .filter(|value| value.is_finite())
                        .ok_or_else(|| invalid("Background must contain finite numbers"))?,
                );
            }
            Some(ColorValue::new(values))
        } else {
            None
        };
        let mut retained_payload_bytes = functions
            .capacity()
            .checked_mul(std::mem::size_of::<Function>())
            .ok_or_else(|| PdfError::ResourceLimit("shading retained payload overflow".into()))?;
        for function in &functions {
            retained_payload_bytes = retained_payload_bytes
                .checked_add(function.retained_payload_bytes(&ctx.limits())?)
                .filter(|bytes| *bytes <= ctx.limits().max_decoded_bytes)
                .ok_or_else(|| {
                    PdfError::ResourceLimit("shading retained function payload".into())
                })?;
        }
        if let Some(background) = &background {
            retained_payload_bytes = retained_payload_bytes
                .checked_add(background.retained_bytes()?)
                .filter(|bytes| *bytes <= ctx.limits().max_decoded_bytes)
                .ok_or_else(|| PdfError::ResourceLimit("shading retained background".into()))?;
        }
        Ok(Self {
            geometry,
            colorspace,
            functions,
            domain,
            extend,
            bbox,
            background,
            anti_alias,
            retained_payload_bytes,
        })
    }

    pub fn geometry(&self) -> ShadingGeometry {
        self.geometry
    }
    pub(crate) fn retained_payload_bytes(&self) -> usize {
        self.retained_payload_bytes
    }
    pub fn colorspace(&self) -> &ColorSpace {
        &self.colorspace
    }
    /// Scalar blend domain; for Type1, returns the x interval. The complete
    /// two-dimensional domain and matrix are available through `geometry()`.
    pub fn domain(&self) -> [f32; 2] {
        self.domain
    }
    pub fn extend(&self) -> [bool; 2] {
        self.extend
    }
    pub fn bbox(&self) -> Option<[f32; 4]> {
        self.bbox
    }
    pub fn background(&self) -> Option<&ColorValue> {
        self.background.as_ref()
    }
    pub fn anti_alias(&self) -> bool {
        self.anti_alias
    }

    /// Sample a point in shading coordinates for a direct `sh` invocation.
    /// Background is intentionally not used: PDF applies it to shading patterns,
    /// not to the sh operator. The caller applies graphics-state clipping/alpha.
    pub fn color_at_point(&self, x: f64, y: f64) -> PdfResult<Option<ColorRgb>> {
        if !x.is_finite() || !y.is_finite() {
            return Err(invalid("sample coordinates must be finite"));
        }
        if self.bbox.is_some_and(|bounds| {
            x < f64::from(bounds[0])
                || y < f64::from(bounds[1])
                || x > f64::from(bounds[2])
                || y > f64::from(bounds[3])
        }) {
            return Ok(None);
        }
        let allowed = |fraction: f64| {
            fraction.is_finite()
                && (fraction >= 0.0 || self.extend[0])
                && (fraction <= 1.0 || self.extend[1])
        };
        let fraction = match self.geometry {
            ShadingGeometry::FunctionBased { domain, matrix } => {
                let [a, b, c, d, e, f] = matrix.map(f64::from);
                let determinant = a * d - b * c;
                if determinant == 0.0 {
                    return Ok(None);
                }
                let px = x - e;
                let py = y - f;
                let u = (d * px - c * py) / determinant;
                let v = (-b * px + a * py) / determinant;
                if !u.is_finite() || !v.is_finite() {
                    return Err(invalid("Type1 inverse coordinates overflow"));
                }
                if u < f64::from(domain[0])
                    || u > f64::from(domain[1])
                    || v < f64::from(domain[2])
                    || v > f64::from(domain[3])
                {
                    return Ok(None);
                }
                return self.evaluate_color(&[u as f32, v as f32]).map(Some);
            }
            ShadingGeometry::Axial(coords) => {
                let [x0, y0, x1, y1] = coords.map(f64::from);
                let dx = x1 - x0;
                let dy = y1 - y0;
                let length = dx * dx + dy * dy;
                if length == 0.0 {
                    return Ok(None);
                }
                let fraction = ((x - x0) * dx + (y - y0) * dy) / length;
                allowed(fraction).then_some(fraction)
            }
            ShadingGeometry::Radial(coords) => {
                let [x0, y0, r0, x1, y1, r1] = coords.map(f64::from);
                let dx = x1 - x0;
                let dy = y1 - y0;
                let dr = r1 - r0;
                let px = x - x0;
                let py = y - y0;
                let a = dx * dx + dy * dy - dr * dr;
                let b = -2.0 * (px * dx + py * dy + r0 * dr);
                let c = px * px + py * py - r0 * r0;
                if ![a, b, c].iter().all(|value| value.is_finite()) {
                    return Err(invalid("radial sample arithmetic overflow"));
                }
                let mut roots = [f64::NAN; 2];
                if a == 0.0 {
                    if b != 0.0 {
                        roots[0] = -c / b;
                    } else if c == 0.0 {
                        roots[0] = 1.0;
                    }
                } else {
                    let discriminant = b * b - 4.0 * a * c;
                    if !discriminant.is_finite() {
                        return Err(invalid("radial discriminant overflow"));
                    }
                    if discriminant >= 0.0 {
                        // Stable quadratic roots avoid cancellation near a cone tip.
                        let q = -0.5 * (b + discriminant.sqrt().copysign(b));
                        if q == 0.0 {
                            roots[0] = -b / (2.0 * a);
                        } else {
                            roots = [q / a, c / q];
                        }
                    }
                }
                roots
                    .into_iter()
                    .filter(|fraction| allowed(*fraction) && r0 + fraction * dr >= 0.0)
                    .max_by(f64::total_cmp)
            }
        };
        fraction
            .map(|fraction| self.color_at(fraction.clamp(0.0, 1.0)))
            .transpose()
    }

    /// Evaluate a normalized location within the shading's domain.
    /// Geometry/Extend/BBox and background application belong to the painter.
    pub fn color_at(&self, fraction: f64) -> PdfResult<ColorRgb> {
        if matches!(self.geometry, ShadingGeometry::FunctionBased { .. }) {
            return Err(invalid("Type1 requires two-dimensional point sampling"));
        }
        if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
            return Err(invalid("fraction must be in [0,1]"));
        }
        let parameter = (f64::from(self.domain[0])
            + fraction * (f64::from(self.domain[1]) - f64::from(self.domain[0])))
            as f32;
        self.evaluate_color(&[parameter])
    }

    // Shared by one-dimensional blends and future two-dimensional Type1
    // sampling; coordinates arrive in the function's own domain.
    fn evaluate_color(&self, parameters: &[f32]) -> PdfResult<ColorRgb> {
        let values = if self.functions.len() == 1 {
            self.functions[0].eval(parameters)?
        } else {
            let mut values = Vec::new();
            values
                .try_reserve_exact(self.functions.len())
                .map_err(|_| PdfError::ResourceLimit("shading color allocation".into()))?;
            for function in &self.functions {
                let output = function.eval(parameters)?;
                let [component] = output.as_slice() else {
                    return Err(invalid("component function must return one value"));
                };
                values.push(*component);
            }
            values
        };
        self.colorspace.rgb(&ColorValue::new(values))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::PdfDocument, io::stream_reader::StreamReader, parser::syntax::SyntaxParser,
    };
    use std::io::Cursor;
    fn parse(source: &str) -> PdfObject {
        SyntaxParser::new(StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap())
            .get_object()
            .unwrap()
    }

    #[test]
    fn function_based_shading_maps_two_dimensional_domain_and_matrix() {
        use crate::objects::pdf_stream::PdfStream;
        let doc = PdfDocument::new().unwrap();
        let function = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                parse("<< /FunctionType 4 /Domain [-1 2 -1 2] /Range [0 1] >>")
                    .into_dict()
                    .unwrap(),
                b"{ add 2 div }".to_vec(),
            )))
            .unwrap();
        let make = |extra: &str| {
            Shading::try_new(
                &parse(&format!(
                    "<< /ShadingType 1 /ColorSpace /DeviceGray /Function {} {} R {extra} >>",
                    function.obj_num(),
                    function.gen_num()
                )),
                doc.ctx(),
            )
        };
        let default = make("").unwrap();
        assert_eq!(
            default.color_at_point(0.25, 0.75).unwrap().unwrap().r(),
            128
        );
        assert!(default.color_at_point(2.0, 0.5).unwrap().is_none());
        assert!(default.color_at(0.5).is_err());
        let mapped = make("/Matrix [2 1 0 3 10 20] /BBox [10 20 12 24]").unwrap();
        assert_eq!(mapped.color_at_point(10.5, 22.5).unwrap().unwrap().r(), 128);
        assert!(mapped.color_at_point(9.0, 22.0).unwrap().is_none());
        let collapsed = make("/Matrix [0 0 0 1 0 0]").unwrap();
        assert!(collapsed.color_at_point(0.0, 0.5).unwrap().is_none());
        for extra in [
            "/Domain [1 0 0 1]",
            "/Domain [0 3 0 1]",
            "/Domain [0 1]",
            "/Matrix [1 0 0 1 0]",
        ] {
            assert!(make(extra).is_err(), "{extra}");
        }
        assert!(Shading::try_new(&parse(
            "<< /ShadingType 1 /ColorSpace /DeviceGray /Function << /FunctionType 2 /Domain [0 1] /N 1 >> >>"
        ), doc.ctx()).is_err());
    }

    #[test]
    fn function_based_shading_component_arrays_and_sampled_functions() {
        use crate::objects::pdf_stream::PdfStream;
        let doc = PdfDocument::new().unwrap();
        let stream = |dict: &str, bytes: Vec<u8>| {
            doc.try_insert_object(PdfObject::PdfStream(PdfStream::new(
                parse(dict).into_dict().unwrap(),
                bytes,
            )))
            .unwrap()
        };
        let ids: Vec<_> = ["{ pop }", "{ exch pop }", "{ add 2 div }"]
            .iter()
            .map(|code| {
                stream(
                    "<< /FunctionType 4 /Domain [0 1 0 1] /Range [0 1] >>",
                    code.as_bytes().to_vec(),
                )
            })
            .collect();
        let refs = ids
            .iter()
            .map(|id| format!("{} {} R", id.obj_num(), id.gen_num()))
            .collect::<Vec<_>>()
            .join(" ");
        let rgb = Shading::try_new(
            &parse(&format!(
                "<< /ShadingType 1 /ColorSpace /DeviceRGB /Function [{refs}] >>"
            )),
            doc.ctx(),
        )
        .unwrap();
        let color = rgb.color_at_point(0.25, 0.75).unwrap().unwrap();
        assert_eq!([color.r(), color.g(), color.b()], [64, 191, 128]);
        let sampled = stream(
            "<< /FunctionType 0 /Domain [0 1 0 1] /Range [0 1] /Size [2 2] /BitsPerSample 8 >>",
            vec![0, 255, 255, 0],
        );
        let gray = Shading::try_new(
            &parse(&format!(
                "<< /ShadingType 1 /ColorSpace /DeviceGray /Function {} {} R >>",
                sampled.obj_num(),
                sampled.gen_num()
            )),
            doc.ctx(),
        )
        .unwrap();
        assert_eq!(gray.color_at_point(0.0, 0.0).unwrap().unwrap().r(), 0);
        assert_eq!(gray.color_at_point(1.0, 0.0).unwrap().unwrap().r(), 255);
        assert_eq!(gray.color_at_point(0.25, 0.75).unwrap().unwrap().r(), 159);
        assert_eq!(gray.color_at_point(0.5, 0.5).unwrap().unwrap().r(), 128);
    }

    #[test]
    fn shading_point_sampling_handles_extend_bbox_and_radial_roots() {
        let doc = PdfDocument::new().unwrap();
        let make = |kind, coords, extend: &str, bbox: &str| {
            Shading::try_new(&parse(&format!("<< /ShadingType {kind} /ColorSpace /DeviceGray /Coords {coords} /Extend {extend} {bbox} /Background [1] /Function << /FunctionType 2 /Domain [0 1] /N 1 >> >>")), doc.ctx()).unwrap()
        };
        let axial = make(2, "[0 0 10 0]", "[false false]", "");
        assert_eq!(axial.color_at_point(5.0, 100.0).unwrap().unwrap().r(), 128);
        assert!(axial.color_at_point(-1.0, 0.0).unwrap().is_none());
        assert!(axial.color_at_point(11.0, 0.0).unwrap().is_none());
        let axial = make(2, "[0 0 10 0]", "[true true]", "/BBox [-5 -5 15 5]");
        assert_eq!(axial.color_at_point(-1.0, 0.0).unwrap().unwrap().r(), 0);
        assert_eq!(axial.color_at_point(11.0, 0.0).unwrap().unwrap().r(), 255);
        assert!(axial.color_at_point(5.0, 6.0).unwrap().is_none());
        assert!(axial.color_at_point(f64::NAN, 0.0).is_err());
        let radial = make(3, "[0 0 2 0 0 10]", "[false false]", "");
        assert_eq!(radial.color_at_point(5.0, 0.0).unwrap().unwrap().r(), 96);
        assert!(radial.color_at_point(0.0, 0.0).unwrap().is_none());
        assert!(radial.color_at_point(11.0, 0.0).unwrap().is_none());
        let radial = make(3, "[0 0 2 0 0 10]", "[true true]", "");
        assert_eq!(radial.color_at_point(0.0, 0.0).unwrap().unwrap().r(), 0);
        assert_eq!(radial.color_at_point(20.0, 0.0).unwrap().unwrap().r(), 255);
        let radial = make(3, "[0 0 10 0 0 2]", "[false false]", "");
        assert_eq!(radial.color_at_point(5.0, 0.0).unwrap().unwrap().r(), 159);
        let cylinder = make(3, "[0 0 2 10 0 2]", "[false false]", "");
        assert_eq!(cylinder.color_at_point(5.0, 0.0).unwrap().unwrap().r(), 179);
        assert!(cylinder.color_at_point(5.0, 3.0).unwrap().is_none());
        let cone = make(3, "[0 0 0 10 0 10]", "[true true]", "");
        assert_eq!(cone.color_at_point(5.0, 2.0).unwrap().unwrap().r(), 74);
        assert!(cone.color_at_point(-5.0, 0.0).unwrap().is_none());
    }

    #[test]
    fn shading_invalid_parameters_and_shared_function_budget_recover() {
        use crate::parser::options::{Limits, ParseOptions};
        let doc = PdfDocument::new().unwrap();
        let tail = "/Function << /FunctionType 2 /Domain [0 1] /N 1 >>";
        for source in [
            format!("<< /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 1] {tail} >>"),
            format!("<< /ShadingType 3 /ColorSpace /DeviceGray /Coords [0 0 -1 1 1 2] {tail} >>"),
            format!(
                "<< /ShadingType 2 /ColorSpace [/Indexed /DeviceGray 0 <00>] /Coords [0 0 1 0] {tail} >>"
            ),
            format!("<< /ShadingType 2 /ColorSpace /Pattern /Coords [0 0 1 0] {tail} >>"),
            format!(
                "<< /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 1 0] /Domain [1 0] {tail} >>"
            ),
            format!(
                "<< /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 1 0] /Extend [1 false] {tail} >>"
            ),
            format!(
                "<< /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 1 0] /BBox [1 0 0 1] {tail} >>"
            ),
            format!(
                "<< /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 1 0] /Background [0 1] {tail} >>"
            ),
            format!(
                "<< /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 1 0] /AntiAlias 1 {tail} >>"
            ),
            format!("<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 1 0] {tail} >>"),
        ] {
            assert!(
                Shading::try_new(&parse(&source), doc.ctx()).is_err(),
                "{source}"
            );
        }
        let doc = PdfDocument::new_with_options(ParseOptions {
            limits: Limits {
                max_objects: 4,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        let stamp = doc.stamp();
        let child = "<< /FunctionType 2 /Domain [0 1] /N 1 >>";
        let source = format!(
            "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 1 0] /Function [<< /FunctionType 3 /Domain [0 1] /Bounds [0.5] /Encode [0 1 0 1] /Functions [{child} {child}] >> {child} {child}] >>"
        );
        assert!(matches!(
            Shading::try_new(&parse(&source), doc.ctx()),
            Err(PdfError::ResourceLimit(_))
        ));
        let source = format!(
            "<< /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 1 0] /Function {child} >>"
        );
        assert!(Shading::try_new(&parse(&source), doc.ctx()).is_ok());
        assert!(doc.is_stamp_current(&stamp));
    }

    #[test]
    fn axial_and_radial_parameters_evaluate_single_and_component_functions() {
        let doc = PdfDocument::new().unwrap();
        let source = "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 10 0] /Extend [true false] /BBox [0 0 10 10] /Background [0 1 0] /AntiAlias true /Function << /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >> >>";
        let shading = Shading::try_new(&parse(source), doc.ctx()).unwrap();
        assert!(matches!(shading.geometry(), ShadingGeometry::Axial(_)));
        assert_eq!(shading.extend(), [true, false]);
        assert!(shading.anti_alias());
        let color = shading.color_at(0.5).unwrap();
        assert_eq!([color.r(), color.g(), color.b()], [128, 0, 128]);
        let source = "<< /ShadingType 3 /ColorSpace /DeviceRGB /Coords [0 0 0 0 0 10] /Domain [0.25 0.75] /Function [<< /FunctionType 2 /Domain [0 1] /N 1 >> << /FunctionType 2 /Domain [0 1] /N 1 >> << /FunctionType 2 /Domain [0 1] /N 1 >>] >>";
        let shading = Shading::try_new(&parse(source), doc.ctx()).unwrap();
        assert!(matches!(shading.geometry(), ShadingGeometry::Radial(_)));
        let color = shading.color_at(0.0).unwrap();
        assert_eq!([color.r(), color.g(), color.b()], [64, 64, 64]);
        assert!(shading.color_at(f64::NAN).is_err());
    }
}
