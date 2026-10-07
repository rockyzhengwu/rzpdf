use pdf::{
    document::{
        PdfDocument,
        metadata::InfoTextField,
        resource_manager::StandardFont,
        save_options::{SaveOptions, StreamCompression},
    },
    error::PdfResult,
    page::{content_builder::ContentBuilder, page_builder::PageBuilder},
};

fn main() -> PdfResult<()> {
    let path = std::env::args_os()
        .nth(1)
        .unwrap_or_else(|| "created-document.pdf".into());
    let doc = PdfDocument::new()?;
    for index in 0..2 {
        doc.add_page_with_builder(index, PageBuilder::new().size(595.0, 842.0))?;
        let font = doc
            .resources()
            .register_standard_font(index, StandardFont::Helvetica)?;
        let image =
            doc.resources()
                .embed_rgb_image(index, 2, 1, vec![230, 70, 60, 30, 130, 210])?;
        let mut content = ContentBuilder::new();
        content.begin_text();
        content.try_set_font(&font, 24.0)?;
        content.move_text(48.0, 770.0);
        content
            .show_encoded_text(&StandardFont::Helvetica.encode_text("rzpdf - Creation example")?)?;
        content.end_text();
        content.begin_text();
        content.try_set_font(&font, 12.0)?;
        content.move_text(48.0, 735.0);
        content.show_encoded_text(
            &StandardFont::Helvetica
                .encode_text(&format!("Page {} | Text, image and metadata", index + 1))?,
        )?;
        content.end_text();
        content.draw_image(&image, [240.0, 0.0, 0.0, 90.0, 48.0, 600.0])?;
        content
            .set_stroke_rgb(0.2, 0.2, 0.2)
            .set_line_width(1.0)
            .rectangle(48.0, 600.0, 240.0, 90.0)
            .stroke();
        doc.replace_page_content_builder(index, &content)?;
    }
    doc.set_info_text(InfoTextField::Title, Some("rzpdf 构建示例"))?;
    doc.set_info_text(InfoTextField::Author, Some("rzpdf"))?;
    doc.set_info_text(
        InfoTextField::Creator,
        Some("rzpdf create_document example"),
    )?;
    let packet = br#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:creator><rdf:Seq><rdf:li>rzpdf</rdf:li></rdf:Seq></dc:creator>
</rdf:Description></rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#;
    doc.set_metadata_packet(Some(packet.to_vec()))?;
    doc.save_to_file_with_options(
        path,
        SaveOptions {
            compression: StreamCompression::FlateUnencoded,
            ..Default::default()
        },
    )
}
