use pdf::{
    budget::OperationBudget,
    document::{Document, resource_manager::ResourceKind},
    error::PdfError,
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_dict::PdfDict, pdf_stream::PdfStream},
    page::{
        content_builder::ContentBuilder, content_parser::ContentParser,
        content_program::ContentProgram, inline_image::InlineImage,
    },
    parser::{
        options::{Limits, ParseOptions},
        syntax::{ParseMode, SyntaxParser},
    },
};
use std::io::{Cursor, Write};
fn dict(source: &str) -> PdfDict {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(source.as_bytes())).unwrap())
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap()
}
fn raw(payload: &[u8]) -> Vec<u8> {
    let mut body = format!("BI /W {} /H 1 /BPC 8 /CS /G ID ", payload.len()).into_bytes();
    body.extend_from_slice(payload);
    body.extend_from_slice(b"\nEI");
    body
}
fn doc(body: Vec<u8>) -> Document {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, body).unwrap();
    doc
}

#[test]
fn binary_ei_and_neighbor_operations_are_not_truncated() {
    let bytes = b" EI ID %\n\0\xff";
    let mut body = b"q 0 0 m ".to_vec();
    body.extend_from_slice(&raw(bytes));
    body.extend_from_slice(b" 9 9 l S Q");
    let doc = doc(body.clone());
    let mut page = doc.get_page(0).unwrap();
    let program = page.content_program().unwrap();
    assert_eq!(
        program
            .operations()
            .iter()
            .map(|o| o.operator.name())
            .collect::<Vec<_>>(),
        ["q", "m", "BI", "l", "S", "Q"]
    );
    let source = page.extract_inline_image_source(&program, 2).unwrap();
    assert_eq!(source.encoded_data(), bytes);
    let range = source.data_range().unwrap();
    let dictionary_range = source.dictionary_range().unwrap();
    assert!(dictionary_range.end < range.start);
    assert!(
        std::str::from_utf8(&body[dictionary_range.start as usize..dictionary_range.end as usize])
            .unwrap()
            .contains("/BPC 8")
    );
    assert_eq!(&body[range.start as usize..range.end as usize], bytes);
    let images = page.extract_images_paint_order_report().unwrap();
    assert!(images.matches_program(&program));
    assert_eq!(images.images.len(), 1);
    assert!(images.images[0].image.is_inline());
    assert_eq!(images.images[0].source().unwrap().operation_index, 2);
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(
        reopened
            .get_page(0)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes(),
        body
    );
}

#[test]
fn filtered_boundaries_and_abbreviated_key_precedence() {
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(&[255, 0, 0]).unwrap();
    let flate = z.finish().unwrap();
    for (filter, data) in [
        ("AHx", b"FF0000>".to_vec()),
        ("A85", b"rr<$~>".to_vec()),
        ("Fl", flate),
        ("RL", vec![2, 255, 0, 0, 128]),
    ] {
        let mut body =
            format!("q BI /Width 99 /W 1 /H 1 /BPC 8 /CS /RGB /F /{filter} ID ").into_bytes();
        body.extend_from_slice(&data);
        body.extend_from_slice(b" EI Q");
        let doc = doc(body);
        let mut page = doc.get_page(0).unwrap();
        let images = page.extract_images_paint_order().unwrap();
        assert_eq!(
            images[0].image.rgba_buffer().unwrap(),
            [255, 0, 0, 255],
            "{filter}"
        );
        assert_eq!(page.content_program().unwrap().operations().len(), 3);
    }
}

#[test]
fn inline_build_replace_remove_and_stale_source_are_transactional() {
    let image = InlineImage::new(
        dict("<< /W 1 /H 1 /BPC 8 /CS /RGB >>"),
        vec![255, 0, 0],
        Limits::default(),
    )
    .unwrap();
    let replacement = InlineImage::new(
        dict("<< /W 1 /H 1 /BPC 8 /CS /RGB >>"),
        vec![0, 0, 255],
        Limits::default(),
    )
    .unwrap();
    let mut builder = ContentBuilder::new();
    builder
        .draw_inline_image(&image, [10., 0., 0., 20., 30., 40.], Limits::default())
        .unwrap();
    let doc = doc(builder.as_bytes().to_vec());
    doc.copy_page(0, 1).unwrap();
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    let original = snapshot.source_bytes().to_vec();
    doc.replace_page_inline_image_draw(0, &snapshot, 2, &replacement)
        .unwrap();
    let revision = doc.revision();
    assert!(
        doc.replace_page_inline_image_draw(0, &snapshot, 2, &image)
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    assert!(
        doc.get_page(0)
            .unwrap()
            .extract_inline_image_source(&snapshot, 2)
            .is_err()
    );
    assert_eq!(
        doc.get_page(1)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes(),
        original
    );
    let fresh = doc.get_page(0).unwrap().content_program().unwrap();
    doc.transform_page_image_draw(0, &fresh, 2, [1., 0., 0., 1., 2., 3.])
        .unwrap();
    let mut page = doc.get_page(0).unwrap();
    let fresh = page.content_program().unwrap();
    let index = page.extract_images_paint_order().unwrap()[0]
        .source()
        .unwrap()
        .operation_index;
    doc.remove_page_image_draw(0, &fresh, index).unwrap();
    assert!(
        doc.get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn nested_inline_removal_isolates_form_call_and_shared_page() {
    let doc = doc(Vec::new());
    let form = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(
                dict("<< /Subtype /Form /BBox [0 0 100 100] >>"),
                raw(&[170]),
            )
            .unwrap(),
        ))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, form)
        .unwrap();
    doc.replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes())
        .unwrap();
    doc.copy_page(0, 1).unwrap();
    let mut page = doc.get_page(0).unwrap();
    let program = page.content_program().unwrap();
    let images = page.extract_images_paint_order().unwrap();
    assert_eq!(images.len(), 2);
    doc.remove_nested_image_draw(0, &program, images[0].nested_source().unwrap())
        .unwrap();
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        doc.get_page(1)
            .unwrap()
            .extract_images_paint_order()
            .unwrap()
            .len(),
        2
    );
    let reopened = Document::from_bytes(doc.save_to_bytes().unwrap()).unwrap();
    assert_eq!(
        reopened
            .get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn malformed_and_limit_failures_rewind_and_release() {
    for input in [
        b"BI /W 1 /H 1 /CS /G ID x NO".as_slice(),
        b"BI /W 2 /H 1 /CS /G ID x",
        b"BI /W 0 /H 1 /CS /G ID x EI",
        b"BI /W 1 /H 1 /CS /G /F /CCF ID x EI",
    ] {
        let mut parser = ContentParser::new(SyntaxParser::new(
            StreamReader::try_new(Cursor::new(input)).unwrap(),
        ));
        for _ in 0..2 {
            assert!(parser.read_operator().is_err());
            assert_eq!(parser.position(), 0);
        }
    }
    let limits = Limits {
        max_image_pixels: 1,
        ..Default::default()
    };
    assert!(matches!(
        ContentProgram::from_decoded_bytes(
            raw(b"xx"),
            ParseOptions {
                limits,
                ..Default::default()
            }
        ),
        Err(PdfError::ResourceLimit(_))
    ));
    let pool = OperationBudget::new(128);
    let bytes = pdf::budget::BudgetedBytes::copy_from_slice(&pool, &raw(b"x")).unwrap();
    assert!(
        ContentProgram::from_decoded_bytes_with_budget(bytes, Default::default(), &pool).is_err()
    );
    assert_eq!(pool.live_bytes(), 0);
    assert!(
        ContentProgram::from_decoded_streams(
            vec![b"BI /W 1 /H 1 /CS /G ID ".to_vec(), b"x EI".to_vec()],
            Default::default()
        )
        .is_err()
    );
    assert!(
        InlineImage::new(
            dict("<< /W 1 /H 1 /CS /G >>"),
            b"x EI q".to_vec(),
            Limits::default()
        )
        .is_err()
    );
}

#[test]
fn inline_indexed_stencil_and_decode_share_the_image_pipeline() {
    let doc=doc(b"BI /W 2 /H 1 /BPC 1 /CS [/I /RGB 1 <FF00000000FF>] ID \x40 EI BI /W 2 /H 1 /IM true /D [1 0] ID \x40 EI".to_vec());
    let images = doc
        .get_page(0)
        .unwrap()
        .extract_images_paint_order()
        .unwrap();
    assert_eq!(
        images[0].image.rgba_buffer().unwrap(),
        [255, 0, 0, 255, 0, 0, 255, 255]
    );
    assert!(images[1].image.is_mask());
    assert_eq!(images[1].image.data(), [255, 0]);
}

#[test]
fn inline_lzw_and_flate_predictor_keep_encoded_boundaries() {
    let mut bits = Vec::new();
    for code in [256u16, 255, 0, 0, 257] {
        for shift in (0..9).rev() {
            bits.push(((code >> shift) & 1) as u8);
        }
    }
    while bits.len() % 8 != 0 {
        bits.push(0);
    }
    let encoded: Vec<u8> = bits
        .chunks(8)
        .map(|b| b.iter().fold(0, |v, b| (v << 1) | b))
        .collect();
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(&[0, 255, 0, 0]).unwrap();
    for (filter, params, encoded) in [
        ("LZW", "", encoded),
        (
            "Fl",
            "/DP << /Predictor 12 /Columns 1 /Colors 3 /BitsPerComponent 8 >>",
            z.finish().unwrap(),
        ),
    ] {
        let mut bytes =
            format!("q BI /W 1 /H 1 /CS /RGB /BPC 8 /F /{filter} {params} ID ").into_bytes();
        bytes.extend(encoded);
        bytes.extend_from_slice(b" EI Q");
        let doc = doc(bytes);
        let images = doc
            .get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap();
        assert_eq!(
            images[0].image.rgba_buffer().unwrap(),
            [255, 0, 0, 255],
            "{filter}"
        );
    }
}

#[test]
fn resource_colour_alias_resolves_in_page_and_form_scopes() {
    let doc = doc(Vec::new());
    let name = doc
        .resources()
        .register_color_space(
            0,
            PdfObject::PdfName(pdf::objects::pdf_name::PdfName::new("DeviceRGB".into())),
        )
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("BI /W 1 /H 1 /BPC 8 /CS /{name} /F /AHx ID FF0000> EI").into_bytes(),
    )
    .unwrap();
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap()[0]
            .image
            .rgba_buffer()
            .unwrap(),
        [255, 0, 0, 255]
    );
    let form=doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(dict("<< /Subtype /Form /BBox [0 0 10 10] /Resources << /ColorSpace << /Custom /DeviceRGB >> >> >>"),b"BI /W 1 /H 1 /BPC 8 /CS /Custom /F /AHx ID 0000FF> EI".to_vec()).unwrap())).unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, form)
        .unwrap();
    doc.append_page_contents(0, format!("/{name} Do").into_bytes())
        .unwrap();
    let images = doc
        .get_page(0)
        .unwrap()
        .extract_images_paint_order()
        .unwrap();
    assert_eq!(images[1].image.rgba_buffer().unwrap(), [0, 0, 255, 255]);
}

#[test]
fn inline_jpeg_framing_skips_markers_inside_length_delimited_segments() {
    let mut encoder = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
    encoder.set_size(2, 2);
    encoder.set_quality(100.);
    let mut encoder = encoder.start_compress(Vec::new()).unwrap();
    encoder
        .write_scanlines(&[255, 0, 0, 255, 0, 0, 255, 0, 0, 255, 0, 0])
        .unwrap();
    let mut encoded = encoder.finish().unwrap();
    // A valid JPEG COM marker containing false EOI and EI text.
    encoded.splice(
        2..2,
        [255, 254, 0, 9, 255, 217, b' ', b'E', b'I', b' ', b'x'],
    );
    let mut bytes = b"q BI /W 2 /H 2 /CS /RGB /BPC 8 /F /DCT ID ".to_vec();
    bytes.extend_from_slice(&encoded);
    bytes.extend_from_slice(b" EI Q");
    let doc = doc(bytes);
    let mut page = doc.get_page(0).unwrap();
    let program = page.content_program().unwrap();
    assert_eq!(program.operations().len(), 3);
    assert_eq!(
        program.operations()[1]
            .operator
            .inline_image()
            .unwrap()
            .encoded_data(),
        encoded
    );
    assert_eq!(
        page.extract_images_paint_order().unwrap()[0]
            .image
            .rgba_buffer()
            .unwrap()
            .len(),
        16
    );
}

#[test]
fn r6_inline_payload_is_decrypted_once_and_new_edit_api_enforces_permissions() {
    use pdf::security::{EncryptionOptions, SaveSecurity};
    let doc = doc(raw(b" EI "));
    let mut settings = EncryptionOptions::new("user", "owner").unwrap();
    settings.permissions.modify = false;
    let bytes = doc
        .save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&settings))
        .unwrap();
    let user =
        Document::from_bytes_with_password_and_options(bytes.clone(), "user", Default::default())
            .unwrap();
    let mut page = user.get_page(0).unwrap();
    let program = page.content_program().unwrap();
    assert_eq!(
        page.extract_inline_image_source(&program, 0)
            .unwrap()
            .encoded_data(),
        b" EI "
    );
    assert_eq!(
        page.extract_images_paint_order().unwrap()[0].image.data(),
        b" EI "
    );
    let image = InlineImage::new(
        dict("<< /W 1 /H 1 /CS /G /BPC 8 >>"),
        vec![42],
        Limits::default(),
    )
    .unwrap();
    let stamp = user.stamp();
    assert!(matches!(
        user.replace_page_inline_image_draw(0, &program, 0, &image),
        Err(PdfError::PermissionDenied(_))
    ));
    assert!(user.is_stamp_current(&stamp));
    let owner =
        Document::from_bytes_with_password_and_options(bytes, "owner", Default::default()).unwrap();
    let program = owner.get_page(0).unwrap().content_program().unwrap();
    owner
        .replace_page_inline_image_draw(0, &program, 0, &image)
        .unwrap();
    assert_eq!(
        owner
            .get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap()[0]
            .image
            .data(),
        [42]
    );
}

#[test]
fn inline_transform_depth_and_builder_failure_leave_sources_unchanged() {
    let image = InlineImage::new(
        dict("<< /W 1 /H 1 /CS /G /BPC 8 >>"),
        vec![128],
        Limits::default(),
    )
    .unwrap();
    let mut builder = ContentBuilder::new();
    builder.save_state();
    let before = builder.as_bytes().to_vec();
    assert!(
        builder
            .draw_inline_image(&image, [f32::NAN, 0., 0., 1., 0., 0.], Limits::default())
            .is_err()
    );
    assert_eq!(builder.as_bytes(), before);
    assert!(
        builder
            .draw_inline_image(
                &image,
                [1., 0., 0., 1., 0., 0.],
                Limits {
                    max_depth: 0,
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(builder.as_bytes(), before);
    let mut source = b"q ".to_vec();
    source.extend_from_slice(&raw(b"x"));
    source.extend_from_slice(b" Q");
    let program = ContentProgram::from_decoded_bytes(source.clone(), Default::default()).unwrap();
    assert!(
        program
            .with_transformed_image_operation(
                1,
                [1., 0., 0., 1., 0., 0.],
                ParseOptions {
                    limits: Limits {
                        max_depth: 1,
                        ..Default::default()
                    },
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(program.source_bytes(), source);
}

#[test]
fn out_of_context_delimiters_and_forbidden_inline_codecs_are_explicit_errors() {
    for source in [
        b"ID q".as_slice(),
        b"EI",
        b"BI /W 1 /H 1 /BPC 8 /CS /G /F /JPXDecode ID x EI",
        b"BI /W 1 /H 1 /BPC 1 /CS /G /F [/AHx /JBIG2Decode] ID 00> EI",
    ] {
        assert!(ContentProgram::from_decoded_bytes(source.to_vec(), Default::default()).is_err());
    }
    let streams = vec![b"BI /W 1 /H 1 /BPC 8 /CS /G ID ".to_vec(), b"x EI".to_vec()];
    assert!(matches!(
        ContentProgram::from_decoded_streams(streams, Default::default()),
        Err(PdfError::UnsupportedFeature(_))
    ));
}

#[test]
fn nested_inline_transform_and_xobject_replacement_isolate_selected_call() {
    for replace in [false, true] {
        let doc = doc(Vec::new());
        let replacement=doc.try_insert_object(PdfObject::PdfStream(PdfStream::new(dict("<< /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB >>"),vec![255,0,0]))).unwrap();
        let handle = doc.object_handle(replacement).unwrap();
        let form = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(
                    dict("<< /Subtype /Form /BBox [0 0 100 100] >>"),
                    raw(&[170]),
                )
                .unwrap(),
            ))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, form)
            .unwrap();
        doc.replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes())
            .unwrap();
        doc.copy_page(0, 1).unwrap();
        let mut page = doc.get_page(0).unwrap();
        let root = page.content_program().unwrap();
        let before = page.extract_images_paint_order().unwrap();
        let source = before[0].nested_source().unwrap();
        if replace {
            doc.replace_nested_image_draw(0, &root, source, &handle)
                .unwrap();
        } else {
            doc.transform_nested_image_draw(0, &root, source, [1., 0., 0., 1., 2., 3.])
                .unwrap();
        }
        let after = doc
            .get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap();
        assert_eq!(after.len(), 2);
        assert_eq!(after[1].image.data(), before[1].image.data());
        assert_eq!(after[1].matrix.e(), before[1].matrix.e());
        if replace {
            assert_eq!(after[0].image.rgba_buffer().unwrap(), [255, 0, 0, 255]);
            assert!(!after[0].image.is_inline());
        } else {
            assert!(after[0].image.is_inline());
            assert_eq!(after[0].matrix.e(), before[0].matrix.e() + 2.);
        }
        let unchanged = doc
            .get_page(1)
            .unwrap()
            .extract_images_paint_order()
            .unwrap();
        for (a, b) in unchanged.iter().zip(&before) {
            assert_eq!(a.image.data(), b.image.data());
            assert_eq!(a.matrix.e(), b.matrix.e());
        }
        let reopened = Document::from_bytes(doc.save_to_bytes().unwrap()).unwrap();
        assert_eq!(
            reopened
                .get_page(0)
                .unwrap()
                .extract_images_paint_order()
                .unwrap()[0]
                .image
                .data(),
            after[0].image.data()
        );
    }
}

#[test]
fn ascii_wrapped_inline_ccitt_uses_existing_decode_parameters() {
    let mut encoder = fax::encoder::Encoder::new(fax::VecWriter::new());
    encoder
        .encode_line([fax::Color::Black, fax::Color::White].into_iter(), 2)
        .unwrap();
    encoder
        .encode_line([fax::Color::White, fax::Color::Black].into_iter(), 2)
        .unwrap();
    let data = encoder.finish().unwrap().finish();
    let encoded: String = data.iter().map(|b| format!("{b:02X}")).collect();
    let doc=doc(format!("BI /W 2 /H 2 /BPC 1 /CS /G /F [/AHx /CCF] /DP [null << /K -1 /Columns 2 /Rows 2 /BlackIs1 false >>] ID {encoded}> EI").into_bytes());
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap()[0]
            .image
            .rgba_buffer()
            .unwrap(),
        [
            0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255
        ]
    );
}

#[test]
fn inline_edit_preserves_neighbor_text_codes_positions_and_unmodified_bytes() {
    use pdf::document::resource_manager::StandardFont;
    let doc = doc(Vec::new());
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    let prefix = format!("BT /{font} 10 Tf 1 0 0 1 20 30 Tm (Before) Tj ET\n");
    let suffix = format!("\nBT /{font} 10 Tf 1 0 0 1 20 60 Tm (After) Tj ET");
    let mut bytes = prefix.as_bytes().to_vec();
    bytes.extend_from_slice(&raw(b"EI "));
    bytes.extend_from_slice(suffix.as_bytes());
    doc.replace_page_contents(0, bytes).unwrap();
    let mut page = doc.get_page(0).unwrap();
    let source = page.content_program().unwrap();
    let original = page.extract_characters_paint_order().unwrap();
    let operation = page.extract_images_paint_order().unwrap()[0]
        .source()
        .unwrap()
        .operation_index;
    doc.remove_page_image_draw(0, &source, operation).unwrap();
    let mut page = doc.get_page(0).unwrap();
    let text = page.extract_characters_paint_order().unwrap();
    assert_eq!(text.len(), original.len());
    for (a, b) in text.iter().zip(&original) {
        assert_eq!(a.unicode, b.unicode);
        assert_eq!(a.position.x(), b.position.x());
        assert_eq!(a.position.y(), b.position.y());
    }
    let program = page.content_program().unwrap();
    assert!(program.source_bytes().starts_with(prefix.as_bytes()));
    assert!(program.source_bytes().ends_with(suffix.as_bytes()));
}
