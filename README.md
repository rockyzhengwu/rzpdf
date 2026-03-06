# rzpdf

A Rust library for **PDF data extraction** and **rendering**.

> **Note:** This library is currently under active development. Several features, such as Pattern colorspaces and Type3 fonts, are not yet implemented.

---

## 🚀 Features

- **Data Extraction:** Efficiently traverse and extract page objects and document metadata.
- **Rendering:** Robust rendering support powered by `skia-safe` (Rust bindings for the Skia Graphics Engine).
- **Rust-Native:** Designed for safety, speed, and easy integration into the Rust ecosystem.

---

## 🛠 Examples

You can explore the library's capabilities by running the provided examples in the repository.

### 🔍 Document Tracing

Traverse the PDF structure and retrieve all page objects:

```bash
cargo run --example document
```
