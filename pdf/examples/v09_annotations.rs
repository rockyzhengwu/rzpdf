//! Generated annotation corpus; only standard Courier, no downloaded assets.
use pdf::{
    document::{
        PdfDocument,
        annotations::{Annotation, AnnotationKind as Kind, TextAlignment},
    },
    error::{PdfError, PdfResult},
    page::page_builder::PageBuilder,
};
fn main() -> PdfResult<()> {
    let output = std::env::args_os()
        .nth(1)
        .ok_or_else(|| PdfError::DocumentError("expected output PDF".into()))?;
    let doc = PdfDocument::new()?;
    doc.add_page_with_builder(0, PageBuilder::new().size(400., 600.))?;
    doc.add_attachment("note.txt", b"v0.9 annotation payload".to_vec())?;
    let file = doc
        .attachment_filespec("note.txt")?
        .ok_or_else(|| PdfError::DocumentError("Filespec missing".into()))?;
    let mut handles = Vec::new();
    for i in 0..15 {
        let (x, y) = (20. + (i % 3) as f32 * 125., 20. + (i / 3) as f32 * 110.);
        let quad = vec![[
            x + 10.,
            y + 50.,
            x + 90.,
            y + 50.,
            x + 10.,
            y + 20.,
            x + 90.,
            y + 20.,
        ]];
        let kind = match i {
            0 => Kind::Square,
            1 => Kind::Circle,
            2 => Kind::Text {
                icon: "Note".into(),
                open: false,
            },
            3 => Kind::Popup { open: false },
            4 => Kind::FreeText {
                font_size: 11.,
                alignment: TextAlignment::Center,
            },
            5 => Kind::Line {
                points: [x + 10., y + 10., x + 90., y + 60.],
            },
            6 => Kind::Polygon {
                vertices: vec![[x + 10., y + 10.], [x + 90., y + 10.], [x + 50., y + 60.]],
            },
            7 => Kind::PolyLine {
                vertices: vec![[x + 10., y + 10.], [x + 50., y + 60.], [x + 90., y + 10.]],
            },
            8 => Kind::Highlight { quads: quad },
            9 => Kind::Underline { quads: quad },
            10 => Kind::Squiggly { quads: quad },
            11 => Kind::StrikeOut { quads: quad },
            12 => Kind::Ink {
                strokes: vec![vec![
                    [x + 10., y + 10.],
                    [x + 50., y + 60.],
                    [x + 90., y + 10.],
                ]],
            },
            13 => Kind::Stamp {
                name: "Approved".into(),
            },
            _ => Kind::FileAttachment {
                file: file.clone(),
                icon: "PushPin".into(),
            },
        };
        let mut a = Annotation::new(kind, [x, y, x + 100., y + 70.]);
        a.contents = if i == 4 {
            "Free text\nsecond line".into()
        } else {
            format!("Annotation {i}")
        };
        a.color = if i == 8 {
            [1., 1., 0.]
        } else {
            [0.1, 0.2, 0.7]
        };
        if i == 1 || i == 6 {
            a.interior_color = Some([0.7, 0.9, 0.8]);
        }
        handles.push(doc.add_annotation(0, &a)?);
    }
    doc.set_annotation_popup(&handles[2], Some(&handles[3]))?;
    doc.set_annotation_reply(&handles[12], Some(&handles[2]))?;
    doc.set_annotation_reply(&handles[13], Some(&handles[12]))?;
    doc.add_page_with_builder(1, PageBuilder::new().size(200., 200.))?;
    doc.replace_page_contents(
        1,
        b"0 0 1 rg 10 110 80 80 re f 0 0 0 rg 110 140 80 10 re f".to_vec(),
    )?;
    let mut a = Annotation::new(Kind::Square, [10., 110., 90., 190.]);
    a.interior_color = Some([1., 0., 0.]);
    a.color = [1., 0., 0.];
    a.opacity = 0.5;
    doc.add_annotation(1, &a)?;
    let mut a = Annotation::new(
        Kind::Highlight {
            quads: vec![[110., 160., 190., 160., 110., 130., 190., 130.]],
        },
        [105., 125., 195., 165.],
    );
    a.color = [1., 1., 0.];
    a.opacity = 0.5;
    doc.add_annotation(1, &a)?;
    let mut a = Annotation::new(
        Kind::Ink {
            strokes: vec![vec![[20., 20.], [80., 80.]], vec![[20., 80.], [80., 20.]]],
        },
        [10., 10., 90., 90.],
    );
    a.color = [0., 0., 0.];
    a.stroke_width = 10.;
    a.opacity = 0.5;
    doc.add_annotation(1, &a)?;
    doc.add_checkbox_field(1, [115., 20., 145., 50.], "check", true)?;
    doc.add_text_field(1, [110., 65., 190., 90.], "text", "Widget")?;
    doc.save_to_file(output)
}
