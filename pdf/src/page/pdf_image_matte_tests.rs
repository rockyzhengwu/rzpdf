use std::io::Cursor;

use crate as pdf;

use pdf::{
    document::owned::Document,
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_dict::PdfDict, pdf_stream::PdfStream},
    page::{page_object::PageObject, pdf_image::PdfImage},
    parser::{
        options::ParseOptions,
        syntax::{ParseMode, SyntaxParser},
    },
    pdf_context::PDFContext,
};

fn dict(source: &str) -> PdfDict {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(source.as_bytes())).unwrap())
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap()
}

fn image(
    fields: &str,
    samples: Vec<u8>,
    mask_fields: &str,
    alpha: Vec<u8>,
) -> pdf::error::PdfResult<PdfImage> {
    let mut base = dict(&format!("<< /Subtype /Image {fields} >>"));
    let mask = dict(&format!(
        "<< /Subtype /Image /ColorSpace /DeviceGray {mask_fields} >>"
    ));
    base.try_insert(
        "SMask".into(),
        PdfObject::PdfStream(PdfStream::new(mask, alpha)),
    )
    .unwrap();
    let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
        PdfDict::default(),
        ParseOptions::default(),
    );
    PdfImage::try_new(
        "test".into(),
        &PdfObject::PdfStream(PdfStream::new(base, samples)),
        &ctx,
    )
}

#[test]
fn matte_rgb_unblends_after_decode_and_handles_zero_and_full_alpha() {
    let image = image(
        "/Width 3 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB /Decode [1 0 1 0 1 0]",
        vec![0, 128, 128, 0, 0, 0, 235, 215, 195],
        "/Width 3 /Height 1 /BitsPerComponent 8 /Matte [1 1 1] /Decode [1 0]",
        vec![127, 255, 0],
    )
    .unwrap();
    assert_eq!(
        image.rgba_buffer().unwrap(),
        [255, 0, 0, 128, 0, 0, 0, 0, 20, 40, 60, 255]
    );
    // Raw samples stay available; unblending must not mutate source data or
    // repeatedly subtract Matte when the same image is converted twice.
    assert_eq!(image.data()[..3], [0, 128, 128]);
    assert_eq!(image.rgba_buffer().unwrap(), image.rgba_buffer().unwrap());
}

#[test]
fn matte_16_bit_alpha_is_not_rounded_before_unblending() {
    let image = image(
        "/Width 1 /Height 1 /BitsPerComponent 16 /ColorSpace /DeviceGray",
        vec![0, 1],
        "/Width 1 /Height 1 /BitsPerComponent 16 /Matte [0]",
        vec![0, 1],
    )
    .unwrap();
    // Both samples are 1/65535, so the recovered colour is white. Rounding
    // either sample to 8 bits first would incorrectly recover black.
    assert_eq!(image.rgba_buffer().unwrap(), [255, 255, 255, 0]);
}

#[test]
fn matte_packed_rows_and_mask_decode_preserve_component_values() {
    for bits in [1, 2, 4, 8, 16] {
        let row = if bits == 16 {
            vec![255, 255]
        } else {
            vec![255]
        };
        let image = image(
            &format!(
                "/Width 1 /Height 2 /BitsPerComponent {bits} /ColorSpace /DeviceGray /Decode [1 0]"
            ),
            row.repeat(2),
            &format!("/Width 1 /Height 2 /BitsPerComponent {bits} /Matte [1] /Decode [1 0]"),
            vec![0; row.len() * 2],
        )
        .unwrap();
        assert_eq!(
            image.rgba_buffer().unwrap(),
            [0, 0, 0, 255, 0, 0, 0, 255],
            "{bits}"
        );
    }
}

#[test]
fn matte_cmyk_is_unblended_before_nonlinear_rgb_conversion() {
    let image = image(
        "/Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceCMYK",
        vec![85, 0, 0, 85],
        "/Width 1 /Height 1 /BitsPerComponent 8 /Matte [0 0 0 0]",
        vec![85],
    )
    .unwrap();
    // c' = k' = 1/3, alpha = 1/3 -> c = k = 1 -> black.
    // Unblending after CMYK->RGB would produce a different result.
    assert_eq!(image.rgba_buffer().unwrap(), [0, 0, 0, 85]);
}

#[test]
fn matte_calibrated_lab_and_tint_spaces_unblend_in_the_source_space() {
    for (space, samples, matte, expected_color) in [
        (
            "[/CalGray << /WhitePoint [0.95047 1 1.08883] /Gamma 2 >>]",
            vec![85],
            "[0]",
            vec![1.0],
        ),
        (
            "[/CalRGB << /WhitePoint [0.95047 1 1.08883] /Gamma [2 2 2] >>]",
            vec![85, 0, 0],
            "[0 0 0]",
            vec![1.0, 0.0, 0.0],
        ),
        (
            "[/Lab << /WhitePoint [0.95047 1 1.08883] /Range [-50 50 -50 50] >>]",
            vec![85, 85, 85],
            "[0 -50 -50]",
            vec![100.0, 50.0, 50.0],
        ),
        (
            "[/Separation /Spot /DeviceGray << /FunctionType 2 /Domain [0 1] /N 2 >>]",
            vec![85],
            "[0]",
            vec![1.0],
        ),
        (
            "[/DeviceN [/Spot] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 2 >>]",
            vec![85],
            "[0]",
            vec![1.0],
        ),
    ] {
        let image = image(
            &format!("/Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace {space}"),
            samples,
            &format!("/Width 1 /Height 1 /BitsPerComponent 8 /Matte {matte}"),
            vec![85],
        )
        .unwrap();
        let rgb = image
            .colorspace()
            .rgb(&pdf::color::value::ColorValue::new(expected_color))
            .unwrap();
        assert_eq!(
            image.rgba_buffer().unwrap(),
            [rgb.r(), rgb.g(), rgb.b(), 85],
            "{space}"
        );
    }
}

#[test]
fn indexed_matte_unblends_palette_colors_per_pixel_not_indices() {
    let image = image(
        "/Width 2 /Height 1 /BitsPerComponent 1 /ColorSpace [/Indexed /DeviceRGB 1 <FFAAAA000000>]",
        vec![0],
        "/Width 2 /Height 1 /BitsPerComponent 8 /Matte [1 1 1]",
        vec![85, 255],
    )
    .unwrap();
    // Same palette index, different alpha: do not cache unblended RGB by index.
    assert_eq!(
        image.rgba_buffer().unwrap(),
        [255, 0, 0, 85, 255, 170, 170, 255]
    );
}

#[test]
fn matte_packed_mask_depths_decode_partial_alpha() {
    for (bits, bytes) in [
        (2, vec![0x40]),
        (4, vec![0x50]),
        (8, vec![85]),
        (16, vec![85, 85]),
    ] {
        let image = image(
            "/Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB",
            vec![255, 170, 170],
            &format!("/Width 1 /Height 1 /BitsPerComponent {bits} /Matte [1 1 1]"),
            bytes,
        )
        .unwrap();
        assert_eq!(image.rgba_buffer().unwrap(), [255, 0, 0, 85], "{bits}");
    }
}

#[test]
fn matte_icc_conversion_resolves_indirect_mask_matte_and_components() {
    use pdf::objects::{pdf_array::PdfArray, pdf_number::PdfNumber, pdf_reference::PdfReference};
    let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
        PdfDict::default(),
        ParseOptions::default(),
    );
    let mut components = PdfArray::default();
    for _ in 0..3 {
        let id = ctx
            .try_insert_object(PdfObject::PdfNumber(PdfNumber::new(1.0)))
            .unwrap();
        components
            .try_add_obj(PdfObject::PdfReference(PdfReference::from_id(id)))
            .unwrap();
    }
    let matte_id = ctx
        .try_insert_object(PdfObject::PdfArray(components))
        .unwrap();
    let mut mask = dict(
        "<< /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 >>",
    );
    mask.try_insert(
        "Matte".into(),
        PdfObject::PdfReference(PdfReference::from_id(matte_id)),
    )
    .unwrap();
    let mask_id = ctx
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(mask, vec![85])))
        .unwrap();
    let mut base = dict("<< /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 >>");
    let mut space = PdfArray::default();
    space
        .try_add_obj(dict("<< /Space /ICCBased >>").get("Space").unwrap().clone())
        .unwrap();
    space
        .try_add_obj(PdfObject::PdfStream(PdfStream::new(
            dict("<< /N 3 >>"),
            pdf::color::icc_profile::ICCProfile::srgb()
                .to_bytes()
                .unwrap(),
        )))
        .unwrap();
    base.try_insert("ColorSpace".into(), PdfObject::PdfArray(space))
        .unwrap();
    base.try_insert(
        "SMask".into(),
        PdfObject::PdfReference(PdfReference::from_id(mask_id)),
    )
    .unwrap();
    let object = PdfObject::PdfStream(PdfStream::new(base, vec![255, 170, 170]));
    let image = PdfImage::try_new("icc".into(), &object, &ctx).unwrap();
    let rgb = image
        .colorspace()
        .rgb(&pdf::color::value::ColorValue::new(vec![1.0, 0.0, 0.0]))
        .unwrap();
    assert_eq!(
        image.rgba_buffer().unwrap(),
        [rgb.r(), rgb.g(), rgb.b(), 85]
    );
    let bytes = image.retained_sample_bytes().unwrap();
    // Matte is retained, so it must be included in the operation accounting.
    let mut without_matte = image.clone();
    without_matte.soft_mask.as_mut().unwrap().matte = None;
    assert_eq!(
        bytes - without_matte.retained_sample_bytes().unwrap(),
        3 * std::mem::size_of::<f32>()
    );
}

#[test]
fn matte_clips_reconstructed_components_and_accepts_null_as_absent() {
    let clipped = image(
        "/Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB",
        vec![255, 0, 255],
        "/Width 1 /Height 1 /BitsPerComponent 8 /Matte [0.5 0.5 0.5]",
        vec![1],
    )
    .unwrap();
    assert_eq!(clipped.rgba_buffer().unwrap(), [255, 0, 255, 1]);
    let no_matte = image(
        "/Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB",
        vec![255, 170, 170],
        "/Width 1 /Height 1 /BitsPerComponent 8 /Matte null",
        vec![85],
    )
    .unwrap();
    assert_eq!(no_matte.rgba_buffer().unwrap(), [255, 170, 170, 85]);
}

#[test]
fn matte_invalid_types_counts_and_component_ranges_are_rejected() {
    for matte in [
        "1",
        "/Bad",
        "[]",
        "[0]",
        "[0 0 0 0]",
        "[0 /Bad 0]",
        "[-1 0 0]",
        "[0 2 0]",
    ] {
        assert!(
            image(
                "/Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB",
                vec![0; 3],
                &format!("/Width 1 /Height 1 /BitsPerComponent 8 /Matte {matte}"),
                vec![128],
            )
            .is_err(),
            "{matte}"
        );
    }
}

fn synthetic_pdf() -> Vec<u8> {
    let mut pdf = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = vec![0];
    let objects: [&[u8]; 6] = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] /Resources << /XObject << /Im 5 0 R >> >> /Contents 4 0 R >>",
        b"<< /Length 6 >>\nstream\n/Im Do\nendstream",
        b"<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB /SMask 6 0 R /Length 3 >>\nstream\n\xff\xaa\xaa\nendstream",
        b"<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceGray /Matte [1 1 1] /Length 1 >>\nstream\n\x55\nendstream",
    ];
    for (index, object) in objects.into_iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
        pdf.extend_from_slice(object);
        pdf.extend_from_slice(b"\nendobj\n");
    }
    let xref = pdf.len();
    pdf.extend_from_slice(b"xref\n0 0\n0 7\n0000000000 65535 f \n");
    for offset in &offsets[1..] {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!("7 0\ntrailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    pdf
}

#[test]
fn empty_xref_and_matte_work_together_in_strict_and_compatible_display_lists() {
    for mode in [ParseMode::Strict, ParseMode::Compatible] {
        let doc = Document::from_bytes_with_mode(synthetic_pdf(), mode).unwrap();
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        let image = list
            .objects()
            .iter()
            .find_map(|object| match object {
                PageObject::Image(image) => Some(image),
                _ => None,
            })
            .unwrap();
        assert_eq!(image.image.rgba_buffer().unwrap(), [255, 0, 0, 85]);
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        reopened.get_page(0).unwrap().display_list().unwrap();
    }
}

#[test]
#[ignore = "requires RZPDF_COMPATIBILITY_SAMPLE pointing to a local, non-redistributed PDF"]
fn local_financial_example_opens_and_builds_first_page() {
    let path =
        std::env::var_os("RZPDF_COMPATIBILITY_SAMPLE").expect("set RZPDF_COMPATIBILITY_SAMPLE");
    let bytes = std::fs::read(path).unwrap();
    for mode in [ParseMode::Strict, ParseMode::Compatible] {
        let doc = Document::from_bytes_with_mode(bytes.clone(), mode).unwrap();
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        let images = doc
            .get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap();
        for image in &images {
            if !image.image.is_mask() {
                image.image.rgba_buffer().unwrap();
            }
        }
        eprintln!(
            "{mode:?}: first page: {} display objects, {} images",
            list.objects().len(),
            images.len()
        );
    }
}
