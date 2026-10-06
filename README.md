# Kestrel-PDF

> **Ultra-High-Performance, Universal PDF Reader & Editor for Windows, macOS, Linux, FreeBSD & WebAssembly**

Kestrel-PDF is engineered from the ground up for instantaneous startup, fluid 120 FPS navigation, in-place text & image editing, digital contract signing, and true data censorship/redaction.

---

## Key Features

- ⚡ **Instantaneous Performance**: Native multi-threaded rendering pipeline with asynchronous tile rasterization and bounded memory footprint ($< 40\text{ MB}$).
- 📖 **Flawless PDF Viewing (Phase 1 MVP)**: Smooth continuous scrolling, zoom (10%-1000%), thumbnail navigation, outlines, and sub-millisecond search.
- 📝 **AcroForms & Contract Signing**: Fill interactive forms and apply legally binding visual & PAdES cryptographically certified signatures.
- ✏️ **True In-Place Editing**: Modify existing text, insert typography, replace and transform images directly within the vector stream.
- 🛡️ **True Cryptographic Redaction**: Permanently strips and zeroes out sensitive text glyphs, vector paths, and image pixels—eliminating data leaks prior to export.
- 🌐 **Universal Deployment**: Runs natively on Windows, macOS, Linux, FreeBSD, and in the browser via WebAssembly (WASM).

---

## Documentation

- 🔍 **[PDF Engine Investigation](docs/engine-investigation.md)**: Deep benchmark and comparison of PDFium, MuPDF, Poppler, SumatraPDF, Sioyek, and Okular.
- 🏛️ **[Architecture Specification](docs/architecture.md)**: Asynchronous rendering pipeline, memory virtualization, decoupled core engine, and universal tech stack.
- 🗺️ **[MVP Roadmap](docs/roadmap.md)**: Phased milestones prioritizing the high-performance reader as Feature #1.

---

## Technology Stack

- **Core Engine**: Rust (Zero-cost abstractions, fearless concurrency, memory safety).
- **GUI & Graphics Shell**: **egui (`eframe` + `wgpu`)** (Pure Rust, 100% MIT/Apache-2.0, hardware-accelerated via DirectX 12 / Metal / Vulkan / WebGPU).
- **Rasterization Engine**: Google PDFium (SIMD-accelerated, battle-tested standard compliance, Apache 2.0 / BSD permissive license).
- **Stream Sanitizer / Redactor**: `lopdf` AST stream parser & rewriter (Pure Rust, runs on desktop & WASM).

---

## License

Apache 2.0 / MIT
