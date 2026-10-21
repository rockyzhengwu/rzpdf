//! Read-only probes for corpus failures; no corpus paths are built in.
use pdf::{
    document::{Document, cross_ref_table::ObjectState},
    io::stream_reader::StreamReader,
    page::page_object::PageObject,
    parser::{options::ParseOptions, parser::parse_xref, syntax::SyntaxParser},
};
use std::{io::Cursor, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("jpx") => {
            let bytes = std::fs::read(&args[1])?;
            let (info,pixels) = pdf::filter::jpx::decode_with_budget(&bytes, Default::default(), &ParseOptions::default().limits.operation_budget())?;
            println!("{info:?}");
            std::fs::write(&args[2],pixels.as_slice())?;
        }
        Some("fax") => {
            let bytes = std::fs::read(&args[1])?;
            let width: u16 = args[2].parse()?;
            let height: u16 = args[3].parse()?;
            for stop in [None, Some(height)] {
                let mut lines = 0usize;
                let mut pixels = Vec::new();
                let consumed = std::cell::Cell::new(0usize);
                let mut decoder = fax::decoder::Group4Decoder::new(bytes.iter().copied().inspect(|_| consumed.set(consumed.get()+1)).map(Ok::<_, std::convert::Infallible>), width)?;
                loop {
                    if stop.is_some_and(|h| lines == usize::from(h)) {
                        println!("fax stop={stop:?} rows={lines} result=height_reached"); break;
                    }
                    match decoder.advance() {
                        Ok(fax::decoder::DecodeStatus::End) => { println!("fax stop={stop:?} rows={lines} result=end"); break; }
                        Ok(_) => {
                            pixels.extend(fax::decoder::pels(decoder.transition(),width).map(|color| if color == fax::Color::White {255u8} else {0u8}));
                            lines += 1;
                        },
                        Err(error) => { println!("fax stop={stop:?} rows={lines} consumed={}/{} error={error:?}",consumed.get(),bytes.len()); break; }
                    }
                    if lines > 65535 { return Err("Fax probe row limit".into()); }
                }
                if let Some(path) = args.get(4) {
                    std::fs::write(format!("{path}.{}.gray", if stop.is_some() {"height"} else {"eof"}), &pixels)?;
                }
            }
        }
        Some("xref") => {
            let bytes = std::fs::read(&args[1])?;
            let mut syntax = SyntaxParser::new(StreamReader::try_new(Cursor::new(bytes.clone()))?);
            let table = parse_xref(&mut syntax)?;
            println!("trailer={:?}", table.trailer());
            let mut entries: Vec<_> = table.objects().iter().collect(); entries.sort_by_key(|(id,_)| **id);
            let mut bad = 0;
            for (id, entry) in entries {
                if !matches!(entry.state(), ObjectState::Normal) { continue; }
                let offset = entry.offset() as usize;
                let data = bytes.get(offset..).unwrap_or_default();
                let prefix = &data[..data.len().min(60)];
                syntax.set_pos(entry.offset())?;
                let words: Vec<_> = (0..3).map(|_| syntax.get_next_word().map(|word|word.raw().to_vec()).unwrap_or_default()).collect();
                if words.len() != 3 || words[0] != id.to_string().as_bytes() || words[1] != entry.gennum().to_string().as_bytes() || words[2] != b"obj" {
                    bad += 1;
                    if bad <= 20 { println!("bad_xref id={id} generation={} offset={offset} bytes={prefix:?}", entry.gennum()); }
                }
            }
            println!("bad_xref_entries={bad}");
        }
        Some("font") => {
            let face = pdf::font::font_program::load_freetype_face(std::fs::read(&args[1])?)?;
            println!("glyph_count={}", face.num_glyphs());
            for n in 0..face.num_charmaps() {
                let map = face.get_charmap(n as isize);
                face.set_charmap(&map)?;
                println!("charmap platform={} encoding={} entries={:?}", map.platform_id(), map.encoding_id(), face.chars().take(16).collect::<Vec<_>>());
            }
            println!("selected={:?}", pdf::font::font_program::use_charmap(true, &face)?);
            for code in [0x20,0xd7,0xf020] { println!("code=0x{code:X} gid={:?}",face.get_char_index(code)); }
        }
        Some("page") => {
            let start = Instant::now();
            let number: u32 = args[2].parse()?;
            let mut options = ParseOptions::default();
            if let Some(value) = args.get(3) { options.limits.max_content_operations = value.parse()?; }
            let doc = Document::open_with_options(&args[1], options)?;
            println!("open_ms={}",start.elapsed().as_millis());
            let mut page = doc.get_page(number.checked_sub(1).ok_or("Page starts at 1")?)?;
            let start = Instant::now(); let program = page.content_program()?;
            println!("page={number} root_operations={} program_ms={}",program.operations().len(),start.elapsed().as_millis());
            let start = Instant::now();
            let list = page.display_list()?;
            println!("objects={} display_ms={}",list.objects().len(),start.elapsed().as_millis());
            let mut fonts = std::collections::BTreeMap::new();
            for object in list.objects() {
                if let PageObject::Image(image) = object {
                    println!("image name={} dimensions={:?} matrix={:?} clip={} alpha={} softmask={} mask={}",image.image.name(),image.image.pixel_dimensions(),image.matrix,image.graphic_state.clip_path.elements.len(),image.graphic_state.general_state.fill_alpha,image.graphic_state.general_state.soft_mask.is_some(),image.image.has_explicit_mask());
                }
                if let PageObject::Text(text) = object {
                    if let Some(font) = &text.state.text_state.font {
                        let count = fonts.entry(font.name().to_string()).or_insert(0usize);
                        *count += 1;
                        if *count == 1 {
                            println!("font={:?} source={:?} program_bytes={}",font.name(),font.font_program_source(),font.fontfile().map_or(0,|b|b.len()));
                            for item in text.items.iter().take(2) {
                                println!("code={} unicode={:?} glyph={:?}",item.charcode().code(),item.unicode(),font.get_glyph(item.charcode()));
                            }
                        }
                    }
                }
            }
            println!("unique_fonts={} text_objects={}", fonts.len(),fonts.values().sum::<usize>());
        }
        _ => return Err("Usage: diagnose_pdf fax RAW WIDTH HEIGHT | jpx RAW OUTPUT | xref PDF | font FONTFILE | page PDF PAGE [MAX_OPERATIONS]".into()),
    }
    Ok(())
}
