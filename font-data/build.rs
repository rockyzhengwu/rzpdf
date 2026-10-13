use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

fn sorted_files(directory: &str) -> Vec<PathBuf> {
    let mut files: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .collect();
    files.sort();
    files
}

fn write_if_changed(path: &Path, content: &[u8]) {
    if fs::read(path).ok().as_deref() != Some(content) {
        fs::write(path, content).unwrap();
    }
}

fn generate_cmaps(out_dir: &Path) {
    let mut source = String::new();
    let mut branches = String::new();
    for path in sorted_files("cmaps") {
        let name = path.file_name().unwrap().to_str().unwrap();
        let ident = format!("_{}", name.replace('-', "_").to_uppercase());
        let mut bytes = Vec::new();
        for line in BufReader::new(fs::File::open(&path).unwrap()).lines() {
            let line = line.unwrap();
            if !line.starts_with('%') {
                bytes.extend_from_slice(line.as_bytes());
                bytes.push(b'\n');
            }
        }
        let data_name = format!("cmap_{name}");
        write_if_changed(&out_dir.join(&data_name), &bytes);
        source.push_str(&format!(
            "const {ident}: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{data_name}\"));\n"
        ));
        branches.push_str(&format!("{name:?} => Some({ident}),\n"));
    }
    source.push_str(&format!(
        "pub fn get_predefine_cmap_data(name: &str) -> Option<&'static [u8]> {{ match name {{ {branches} _ => None }} }}\n"
    ));
    write_if_changed(&out_dir.join("cmap.rs"), source.as_bytes());
}

fn generate_modules(out_dir: &Path, directory: &str, extension: &str) {
    let mut source = String::new();
    for path in sorted_files(directory) {
        if path.extension().and_then(|ext| ext.to_str()) != Some(extension) {
            continue;
        }
        let filename = path.file_name().unwrap().to_str().unwrap();
        let ident = path
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .to_lowercase()
            .replace('-', "_");
        source.push_str(&format!(
            "pub mod {ident} {{ pub const DATA: &[u8] = include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{directory}/{filename}\")); }}\n"
        ));
    }
    write_if_changed(&out_dir.join(format!("{directory}.rs")), source.as_bytes());
}

fn main() {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed=cmaps");
    println!("cargo:rerun-if-changed=fonts");
    println!("cargo:rerun-if-changed=afm");
    generate_cmaps(&out_dir);
    generate_modules(&out_dir, "fonts", "pfb");
    generate_modules(&out_dir, "afm", "afm");
}
