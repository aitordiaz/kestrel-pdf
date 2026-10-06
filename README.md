# Velox-PDF

> **Ultra-High-Performance PDF Reader & Editor for Windows & macOS**

Velox-PDF is engineered from the ground up for instantaneous startup, fluid 120 FPS navigation, in-place text & image editing, digital signing, and true data censorship/redaction.

---

## Key Features

- ⚡ **Instantaneous Performance**: Native multi-threaded rendering pipeline with asynchronous tile rasterization and bounded memory footprint ($< 50\text{ MB}$).
- 📖 **Flawless PDF Viewing (Phase 1 MVP)**: Smooth continuous scrolling, zoom, thumbnail navigation, outlines, and sub-millisecond search.
- 📝 **AcroForms & Contract Signing**: Fill interactive forms and apply legally binding visual & PAdES cryptographically certified signatures.
- ✏️ **True In-Place Editing**: Modify existing text, insert typography, replace and transform images directly within the vector stream.
- 🛡️ **True Cryptographic Redaction**: Permanently strips and zeroes out sensitive text glyphs, vector paths, and image pixels—eliminating data leaks prior to export.

---

## Documentation

- 🔍 **[PDF Engine Investigation](docs/engine-investigation.md)**: Deep benchmark and comparison of PDFium, MuPDF, Poppler, SumatraPDF, Sioyek, and Okular.
- 🏛️ **[Architecture Specification](docs/architecture.md)**: Asynchronous rendering pipeline, memory virtualization, decoupled core engine, and tech stack analysis.
- 🗺️ **[MVP Roadmap](docs/roadmap.md)**: Phased milestones prioritizing the high-performance reader as Feature #1.

---

## Technology Stack

- **Core Engine**: Rust (Zero-cost abstractions, fearless concurrency, memory safety).
- **Rasterization Engine**: Google PDFium (SIMD-accelerated, battle-tested standard compliance, Apache 2.0 / BSD permissive license).
- **Stream Sanitizer / Redactor**: `lopdf` AST stream parser & rewriter.
- **UI Shell**: Native Slint / Direct3D & Metal hardware-accelerated viewport.

---

## License

Apache 2.0 / MIT
