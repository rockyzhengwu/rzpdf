use super::{Annotation, AnnotationKind, TextAlignment};
use crate::{
    document::{
        PdfDocument,
        appearance::{courier, name, nums, reference},
    },
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, pdf_bool::PdfBool, pdf_dict::PdfDict, pdf_number::PdfNumber,
        pdf_string::PdfString,
    },
};
use std::{
    fmt::Write,
    io::{Read, Seek},
};

pub(super) fn boolean(value: bool) -> PdfObject {
    PdfObject::PdfBool(PdfBool::new(if value { b"true" } else { b"false" }))
}
pub(super) fn number(n: f32) -> PdfObject {
    PdfObject::PdfNumber(PdfNumber::new(n))
}
fn invalid(s: &str) -> PdfError {
    PdfError::DocumentError(s.into())
}

struct Painter {
    content: String,
    limit: usize,
    segments: usize,
    max_segments: usize,
}
impl Painter {
    fn add(&mut self, text: String) -> PdfResult<()> {
        if text.len() > self.limit.saturating_sub(self.content.len()) {
            return Err(PdfError::ResourceLimit(
                "annotation appearance bytes".into(),
            ));
        }
        self.content.push_str(&text);
        Ok(())
    }
    fn segment(&mut self, text: String) -> PdfResult<()> {
        self.segments += 1;
        if self.segments > self.max_segments {
            return Err(PdfError::ResourceLimit(
                "annotation appearance segments".into(),
            ));
        }
        self.add(text)
    }
}

impl<R: Read + Seek> PdfDocument<R> {
    pub(super) fn encode_annotation(&self, a: &Annotation, dict: PdfDict) -> PdfResult<PdfDict> {
        self.encode_annotation_prepared(a, dict, None)
    }

    pub(super) fn encode_annotation_prepared(
        &self,
        a: &Annotation,
        mut dict: PdfDict,
        custom_text: Option<(PdfDict, String)>,
    ) -> PdfResult<PdfDict> {
        use crate::document::save_options::PdfVersion;
        let minimum = if a.flags & 512 != 0 {
            PdfVersion::V1_7
        } else if a.flags & 256 != 0
            || matches!(
                a.kind,
                AnnotationKind::Polygon { .. } | AnnotationKind::PolyLine { .. }
            )
        {
            PdfVersion::V1_5
        } else {
            PdfVersion::V1_4
        };
        if self.effective_version()? < minimum {
            return Err(PdfError::UnsupportedFeature(format!(
                "annotation requires PDF {} or later",
                minimum.as_str()
            )));
        }
        let limits = self.ctx().limits();
        let [x, y, right, top] = a.rect;
        let (w, h) = (right - x, top - y);
        if a.rect.iter().any(|v| !v.is_finite())
            || !w.is_finite()
            || !h.is_finite()
            || w < 1.
            || h < 1.
            || a.color
                .iter()
                .chain(a.interior_color.iter().flatten())
                .any(|c| !c.is_finite() || !(0. ..=1.).contains(c))
            || !a.opacity.is_finite()
            || !(0. ..=1.).contains(&a.opacity)
            || !a.stroke_width.is_finite()
            || a.stroke_width < 0.
            || a.stroke_width > w.min(h)
        {
            return Err(invalid("invalid annotation geometry/color/style"));
        }
        if a.flags & !1023 != 0 {
            return Err(PdfError::UnsupportedFeature(
                "unknown annotation flags".into(),
            ));
        }
        let contents = PdfString::from_text_with_limits(&a.contents, &limits)?;
        let is_shape = matches!(
            a.kind,
            AnnotationKind::Square | AnnotationKind::Circle | AnnotationKind::Polygon { .. }
        );
        if a.interior_color.is_some() && !is_shape {
            return Err(invalid(
                "interior color is only supported for closed shapes",
            ));
        }
        let mut painter = Painter {
            content: String::new(),
            limit: limits.max_raw_stream_bytes.min(limits.max_decoded_bytes),
            segments: 0,
            max_segments: limits.max_path_segments,
        };
        painter.add(format!(
            "q {} {} {} RG {} {} {} rg {} w 1 J 1 j\n",
            a.color[0], a.color[1], a.color[2], a.color[0], a.color[1], a.color[2], a.stroke_width
        ))?;
        let mut resources = PdfDict::default();
        for key in [
            "Name",
            "Open",
            "Q",
            "DA",
            "L",
            "Vertices",
            "QuadPoints",
            "InkList",
            "FS",
            "IC",
        ] {
            dict.remove(key);
        }
        dict.try_insert("Type".into(), name("Annot"))?;
        dict.try_insert("Subtype".into(), name(a.kind.subtype()))?;
        dict.try_insert("Rect".into(), nums(&a.rect)?)?;
        dict.try_insert("C".into(), nums(&a.color)?)?;
        dict.try_insert("Contents".into(), PdfObject::PdfString(contents))?;
        dict.try_insert(
            "F".into(),
            PdfObject::PdfNumber(PdfNumber::new_from_lexeme(a.flags.to_string().as_bytes())?),
        )?;
        dict.try_insert("CA".into(), number(a.opacity))?;
        let mut bs = PdfDict::default();
        bs.try_insert("S".into(), name("S"))?;
        bs.try_insert("W".into(), number(a.stroke_width))?;
        dict.try_insert("BS".into(), PdfObject::PdfDict(bs))?;
        let fill = if let Some(c) = a.interior_color {
            dict.try_insert("IC".into(), nums(&c)?)?;
            painter.add(format!("{} {} {} rg\n", c[0], c[1], c[2]))?;
            true
        } else {
            false
        };
        let point = |px: f32, py: f32| -> PdfResult<(f32, f32)> {
            if !px.is_finite() || !py.is_finite() || px < x || px > right || py < y || py > top {
                return Err(invalid("annotation point outside Rect"));
            }
            Ok((px - x, py - y))
        };
        let icon_name = |s: &str| -> PdfResult<()> {
            if s.is_empty()
                || s.len() > limits.max_token_bytes
                || !s.is_ascii()
                || s.bytes().any(|b| !(0x21..=0x7e).contains(&b))
            {
                return Err(invalid("annotation icon/name must be printable ASCII"));
            }
            Ok(())
        };
        let path = |painter: &mut Painter,
                    vertices: &[[f32; 2]],
                    minimum: usize,
                    close: bool|
         -> PdfResult<()> {
            if vertices.len() < minimum || vertices.len() > limits.max_path_segments {
                return Err(invalid("annotation vertex count"));
            }
            for (i, p) in vertices.iter().enumerate() {
                let (px, py) = point(p[0], p[1])?;
                painter.segment(format!("{px} {py} {}\n", if i == 0 { "m" } else { "l" }))?;
            }
            if close {
                painter.segment("h\n".into())?;
            }
            Ok(())
        };
        match &a.kind {
            AnnotationKind::Square => {
                let t = a.stroke_width / 2.;
                painter.segment(format!(
                    "{t} {t} {} {} re {}\n",
                    w - 2. * t,
                    h - 2. * t,
                    if fill { "B" } else { "S" }
                ))?;
            }
            AnnotationKind::Circle => {
                let t = a.stroke_width / 2.;
                let (rx, ry) = ((w - 2. * t) / 2., (h - 2. * t) / 2.);
                let (cx, cy) = (w / 2., h / 2.);
                let k = 0.5522848;
                painter.segment(format!("{} {cy} m\n", cx + rx))?;
                for (a, b, c, d, e, f) in [
                    (cx + rx, cy + k * ry, cx + k * rx, cy + ry, cx, cy + ry),
                    (cx - k * rx, cy + ry, cx - rx, cy + k * ry, cx - rx, cy),
                    (cx - rx, cy - k * ry, cx - k * rx, cy - ry, cx, cy - ry),
                    (cx + k * rx, cy - ry, cx + rx, cy - k * ry, cx + rx, cy),
                ] {
                    painter.segment(format!("{a} {b} {c} {d} {e} {f} c\n"))?;
                }
                painter.segment(format!("h {}\n", if fill { "B" } else { "S" }))?;
            }
            AnnotationKind::Text { icon, .. } | AnnotationKind::FileAttachment { icon, .. } => {
                icon_name(icon)?;
                dict.try_insert("Name".into(), name(icon))?;
                if let AnnotationKind::Text { open, .. } = a.kind {
                    dict.try_insert("Open".into(), boolean(open))?;
                }
                // Deterministic note/file icon, not an application-specific UI window.
                if w < 4. || h < 4. {
                    return Err(invalid("annotation icon Rect too small"));
                }
                painter.segment(format!("1 1 {} {} re B\n", w - 2., h - 2.))?;
                painter.add("0 0 0 RG\n".into())?;
                painter.segment(format!(
                    "{} {} m {} {} l {} {} l S\n",
                    w * 0.55,
                    h - 1.,
                    w * 0.55,
                    h * 0.6,
                    w - 1.,
                    h * 0.6
                ))?;
                if let AnnotationKind::FileAttachment { file, .. } = &a.kind {
                    let id = self.validate_handle(file)?;
                    let fs = self
                        .resolve_object(id)?
                        .into_dict()
                        .ok_or_else(|| invalid("file attachment FS must be dictionary"))?;
                    if fs.get("EF").is_none() {
                        return Err(invalid("file attachment requires embedded Filespec"));
                    }
                    if fs
                        .get("Type")
                        .is_some_and(|t| t.as_name().map(|n| n.name()) != Some("Filespec"))
                    {
                        return Err(invalid("file attachment requires Filespec"));
                    }
                    let ef = self
                        .ctx()
                        .resolve(fs.get("EF").unwrap())?
                        .into_dict()
                        .ok_or_else(|| invalid("Filespec EF must be dictionary"))?;
                    let payload = ef
                        .get("UF")
                        .or_else(|| ef.get("F"))
                        .ok_or_else(|| invalid("embedded Filespec payload missing"))?;
                    if self.ctx().resolve(payload)?.as_stream().is_none() {
                        return Err(invalid("embedded file must be stream"));
                    }
                    dict.try_insert("FS".into(), reference(id))?;
                }
            }
            AnnotationKind::Popup { open } => {
                dict.try_insert("Open".into(), boolean(*open))?;
                return Ok(dict);
            }
            AnnotationKind::FreeText {
                font_size,
                alignment,
            } => {
                if !font_size.is_finite() || *font_size <= 0. {
                    return Err(invalid("invalid FreeText font size"));
                }
                let custom = custom_text;
                if custom.is_none() {
                    if !a.contents.is_ascii()
                        || a.contents
                            .bytes()
                            .any(|b| b != b'\n' && !(0x20..=0x7e).contains(&b))
                    {
                        return Err(PdfError::UnsupportedFeature("FreeText appearance supports ASCII with LF; Unicode font layout is separate".into()));
                    }
                    resources = courier()?;
                }
                dict.try_insert(
                    "DA".into(),
                    PdfObject::PdfString(PdfString::new(
                        format!("/F0 {font_size} Tf").into_bytes(),
                        false,
                    )),
                )?;
                let q = match alignment {
                    TextAlignment::Left => 0.,
                    TextAlignment::Center => 1.,
                    TextAlignment::Right => 2.,
                };
                dict.try_insert("Q".into(), number(q))?;
                if let Some((fonts, commands)) = custom {
                    resources = fonts;
                    painter.add(commands)?;
                } else {
                    let mut baseline = h - 2. - font_size;
                    for line in a.contents.split('\n') {
                        let width = line.len() as f32 * font_size * 0.6;
                        if baseline < 2. || width > w - 4. || !width.is_finite() {
                            return Err(invalid(
                                "FreeText does not fit; automatic layout is not supported",
                            ));
                        }
                        let left = match alignment {
                            TextAlignment::Left => 2.,
                            TextAlignment::Center => (w - width) / 2.,
                            TextAlignment::Right => w - 2. - width,
                        };
                        if line.len() > painter.limit.saturating_sub(painter.content.len()) / 2 {
                            return Err(PdfError::ResourceLimit(
                                "FreeText appearance bytes".into(),
                            ));
                        }
                        let mut hex = String::new();
                        for byte in line.bytes() {
                            write!(&mut hex, "{byte:02X}").map_err(|_| invalid("hex encoding"))?;
                        }
                        painter.add(format!(
                            "BT /F0 {font_size} Tf 1 0 0 1 {left} {baseline} Tm <{hex}> Tj ET\n"
                        ))?;
                        baseline -= font_size * 1.2;
                    }
                }
            }
            AnnotationKind::Line { points } => {
                dict.try_insert("L".into(), nums(points)?)?;
                path(
                    &mut painter,
                    &[[points[0], points[1]], [points[2], points[3]]],
                    2,
                    false,
                )?;
                painter.add("S\n".into())?;
            }
            AnnotationKind::Polygon { vertices } | AnnotationKind::PolyLine { vertices } => {
                if vertices.len() > limits.max_path_segments
                    || vertices.len() > limits.max_objects / 2
                {
                    return Err(PdfError::ResourceLimit("annotation vertices".into()));
                }
                let closed = matches!(a.kind, AnnotationKind::Polygon { .. });
                let flat: Vec<_> = vertices.iter().flatten().copied().collect();
                dict.try_insert("Vertices".into(), nums(&flat)?)?;
                path(&mut painter, vertices, if closed { 3 } else { 2 }, closed)?;
                painter.add(format!("{}\n", if closed && fill { "B" } else { "S" }))?;
            }
            AnnotationKind::Ink { strokes } => {
                if strokes.is_empty() || strokes.len() > limits.max_objects {
                    return Err(invalid("invalid Ink strokes"));
                }
                let mut lists = crate::objects::pdf_array::PdfArray::default();
                for stroke in strokes {
                    if stroke.len() > limits.max_objects / 2 {
                        return Err(PdfError::ResourceLimit("Ink stroke coordinates".into()));
                    }
                    path(&mut painter, stroke, 2, false)?;
                    painter.add("S\n".into())?;
                    lists.try_add_obj(nums(
                        &stroke.iter().flatten().copied().collect::<Vec<_>>(),
                    )?)?;
                }
                dict.try_insert("InkList".into(), PdfObject::PdfArray(lists))?;
            }
            AnnotationKind::Highlight { quads }
            | AnnotationKind::Underline { quads }
            | AnnotationKind::Squiggly { quads }
            | AnnotationKind::StrikeOut { quads } => {
                if quads.is_empty()
                    || quads.len() > limits.max_path_segments / 4
                    || quads.len() > limits.max_objects / 8
                {
                    return Err(invalid("invalid annotation QuadPoints count"));
                }
                dict.try_insert(
                    "QuadPoints".into(),
                    nums(&quads.iter().flatten().copied().collect::<Vec<_>>())?,
                )?;
                for quad in quads {
                    let mut p = [[0.; 2]; 4];
                    for i in 0..4 {
                        let v = point(quad[2 * i], quad[2 * i + 1])?;
                        p[i] = [v.0, v.1];
                    }
                    // Perimeter order and Adobe Z order both preserve the first
                    // edge's text direction; choose the uncrossed perimeter.
                    let cross = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| {
                        (b[0] as f64 - a[0] as f64) * (c[1] as f64 - a[1] as f64)
                            - (b[1] as f64 - a[1] as f64) * (c[0] as f64 - a[0] as f64)
                    };
                    let z_order = cross(p[0], p[1], p[2]) * cross(p[1], p[2], p[3]) < 0.;
                    if z_order {
                        p.swap(2, 3);
                    }
                    let signs = [
                        cross(p[0], p[1], p[2]),
                        cross(p[1], p[2], p[3]),
                        cross(p[2], p[3], p[0]),
                        cross(p[3], p[0], p[1]),
                    ];
                    if signs
                        .iter()
                        .any(|s| *s == 0. || s.signum() != signs[0].signum())
                    {
                        return Err(invalid(
                            "QuadPoints must form a convex nondegenerate quadrilateral",
                        ));
                    }
                    if matches!(a.kind, AnnotationKind::Highlight { .. }) {
                        for (i, v) in p.iter().enumerate() {
                            painter.segment(format!(
                                "{} {} {}\n",
                                v[0],
                                v[1],
                                if i == 0 { "m" } else { "l" }
                            ))?;
                        }
                        painter.segment("h f\n".into())?;
                    } else {
                        let (start, end) = if matches!(a.kind, AnnotationKind::StrikeOut { .. }) {
                            (
                                [p[0][0] * 0.5 + p[3][0] * 0.5, p[0][1] * 0.5 + p[3][1] * 0.5],
                                [p[1][0] * 0.5 + p[2][0] * 0.5, p[1][1] * 0.5 + p[2][1] * 0.5],
                            )
                        } else if z_order {
                            (p[3], p[2])
                        } else {
                            (p[0], p[1])
                        };
                        painter.segment(format!("{} {} m\n", start[0], start[1]))?;
                        if matches!(a.kind, AnnotationKind::Squiggly { .. }) {
                            let dx = end[0] - start[0];
                            let dy = end[1] - start[1];
                            let length = dx.hypot(dy);
                            let n = (length / 2.).ceil().max(1.);
                            if !n.is_finite() || n > limits.max_path_segments as f32 {
                                return Err(PdfError::ResourceLimit("squiggly segments".into()));
                            }
                            for i in 1..=n as usize {
                                let t = i as f32 / n;
                                let amp = if i % 2 == 0 { 0. } else { 1. };
                                painter.segment(format!(
                                    "{} {} l\n",
                                    start[0] + dx * t - dy / length * amp,
                                    start[1] + dy * t + dx / length * amp
                                ))?;
                            }
                        } else {
                            painter.segment(format!("{} {} l\n", end[0], end[1]))?;
                        }
                        painter.add("S\n".into())?;
                    }
                }
            }
            AnnotationKind::Stamp { name: stamp } => {
                icon_name(stamp)?;
                dict.try_insert("Name".into(), name(stamp))?;
                resources = courier()?;
                painter.segment(format!("1 1 {} {} re S\n", w - 2., h - 2.))?;
                let size = ((w - 4.) / (stamp.len() as f32 * 0.6)).min(h / 2.).min(20.);
                if size <= 0. || !size.is_finite() {
                    return Err(invalid("Stamp too small"));
                }
                if stamp.len() > painter.limit.saturating_sub(painter.content.len()) / 2 {
                    return Err(PdfError::ResourceLimit("Stamp appearance bytes".into()));
                }
                let mut hex = String::new();
                for b in stamp.bytes() {
                    write!(&mut hex, "{b:02X}").map_err(|_| invalid("hex encoding"))?;
                }
                painter.add(format!(
                    "BT /F0 {size} Tf 1 0 0 1 2 {} Tm <{hex}> Tj ET\n",
                    (h - size) / 2.
                ))?;
            }
        }
        painter.add("Q\n".into())?;
        let blend = if matches!(a.kind, AnnotationKind::Highlight { .. }) {
            "Multiply"
        } else {
            "Normal"
        };
        let ap = self.normal_appearance(
            w,
            h,
            resources,
            painter.content.into_bytes(),
            a.opacity,
            blend,
        )?;
        let mut appearance = match dict.get("AP") {
            Some(value) => self
                .ctx()
                .resolve(value)?
                .into_dict()
                .ok_or_else(|| invalid("AP must be dictionary"))?,
            None => PdfDict::default(),
        };
        appearance.try_insert("N".into(), reference(ap))?;
        dict.try_insert("AP".into(), PdfObject::PdfDict(appearance))?;
        dict.remove("AS");
        Ok(dict)
    }
}
