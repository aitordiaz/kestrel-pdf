# ADR-0001: System Architecture & Decoupled Engine

- **Status**: Accepted
- **Date**: 2026-10-06
- **Context**: Foundation Architecture Specification for Kestrel-PDF

---

## 1. High-Level Architecture Overview

Kestrel-PDF is designed with a **Strictly Decoupled Core-to-UI Architecture**. The performance-critical engine is completely isolated from the platform UI shell.

```mermaid
flowchart TD
    subgraph UI_Layer["Universal egui Shell"]
        DesktopUI["Desktop Shell (Windows, macOS, Linux, FreeBSD via wgpu)"]
        WebUI["Web Shell (WebAssembly via WebGL2/WebGPU)"]
        InputHandler["Gesture, Wheel & Stylus Input"]
        Viewport["Virtualized Page Viewport (Texture Blit)"]
    end

    subgraph App_Layer["Application & State Engine"]
        DocController["Document Controller"]
        UndoRedo["Transactional Undo/Redo Engine"]
        ToolManager["Tool State (Pan, Select, Edit, Sign, Redact, FormFill)"]
    end

    subgraph Core_Engine["Core PDF & Graphics Engine (Rust)"]
        RenderPipeline["Async Tiled Render Pipeline"]
        TileCache["LRU Bitmap / Texture Cache"]
        TextEngine["Text Layout, Glyph Bounds & Search"]
        FormEngine["AcroForms & Interactive Widgets"]
        MutationEngine["In-Place Content Mutator (Text / Image)"]
        Redactor["True Redaction & Stream Sanitizer"]
        Signer["PAdES / PKCS#7 Cryptographic Signer"]
    end

    subgraph Backend_Engines["Underlying PDF Subsystems"]
        PDFium["Google PDFium (Rasterization & AcroForms)"]
        StreamManip["lopdf / PDF Stream Parser (AST Surgery)"]
        Crypto["Ring / OS KeyStore (Certificates & Hashes)"]
    end

    UI_Layer --> App_Layer
    App_Layer --> Core_Engine
    Core_Engine --> Backend_Engines
```

---

## 2. Core Performance Design Principles

To be the **most performant PDF reader across desktop and web**, Kestrel-PDF implements four architectural pillars:

### A. Asynchronous, Tiled Viewport Rendering
- **Main Thread Never Blocks**: The UI thread only manages input events (scroll, zoom, click) and blits cached GPU textures.
- **Priority-Based Worker Pool**: Page rasterization requests are dispatched to a multi-threaded worker thread pool.
  - Priority 1: Currently visible viewport area at current DPI.
  - Priority 2: 1-2 pages above and below the current viewport (predictive prefetch).
  - Priority 3: Thumbnails and outline navigation.
- **Tiled Decomposition**: Large pages (e.g. A0 blueprints or CAD drawings) are subdivided into $512 \times 512$ pixel tiles. Only tiles intersecting the active viewport are rasterized, preventing out-of-memory crashes.

### B. Bounded LRU Cache & Virtualized Memory
- Even when viewing a 10,000-page PDF, memory usage remains bounded ($< 40\text{ MB}$ footprint).
- As pages scroll out of the predictive window, their rasterized bitmaps are evicted from the LRU cache. The raw PDF file is memory-mapped (`mmap`), loading byte slices on-demand.

### C. True Redaction Engine vs. Superficial Masking
Most basic PDF tools perform "pseudo-redaction" by simply painting a black rectangle annotation over sensitive content. The underlying text remains in the content stream, easily extracted by copying or inspecting the file.

Kestrel-PDF enforces **Cryptographic & Structural True Redaction**:
1. **Content Stream Parsing**: Decompresses the page content stream (`/Filter /FlateDecode`).
2. **Text Operator Stripping**: Identifies text showing operators (`Tj`, `TJ`, `'`, `"`) whose bounding boxes intersect the redaction polygon. The operators and glyph indexes are surgically excised or replaced with sanitized spaces.
3. **Raster Image Scrubbing**: If an image intersects the redaction box, the underlying image pixels are permanently zeroed out in the raster stream; if fully enclosed, the image XObject dictionary is unlinked.
4. **Metadata Sanitization**: Removes document author, edit history, XML metadata packets (`/Metadata`), and deleted object references from the cross-reference (`xref`) table.

### D. Digital Signatures & Form Filling
- **Forms**: Direct two-way binding with PDFium's AcroForm subsystem.
- **Signing**:
  - *Visual*: Smooth cubic Bézier stroke interpolation with pressure sensitivity for stylus and mouse signatures.
  - *Digital (PAdES)*: Signs the PDF byte range using standard PKCS#7 / CMS cryptographic envelopes, embedding the X.509 certificate and SHA-256 hash without breaking document validity.

---

## 3. Technology Stack Selection & Universal Target Matrix

### Language Recommendation: **Rust (Engine Core)**
- **Memory Safety & Exploit Prevention**: PDF parsers handle arbitrary untrusted user files. Rust eliminates buffer overruns and use-after-free vulnerabilities at compile time.
- **Fearless Concurrency**: Managing multi-threaded tile caches and asynchronous render queues without data races is trivial in Rust (`crossbeam`, `rayon`, `tokio`).
- **Zero Runtime Overhead**: Compiles to LLVM native machine code; identical microsecond performance to C++.

---

### Universal Cross-Platform Evaluation (Windows, macOS, Linux, FreeBSD & WebAssembly)

| Framework | Architecture | Windows / macOS / Linux | FreeBSD Support | WebAssembly (WASM) | Rendering Backend | Licensing | Verdict |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **egui** (`eframe` + `wgpu`) | Immediate Mode (Rust) | Native `wgpu` (DirectX 12 / Metal / Vulkan) | Supported (via `winit` / `wgpu`) | **First-Class** (Canvas / WebGL2 / WebGPU) | Pure `wgpu` | MIT / Apache-2.0 | **Adopted**: Pure Rust, 100% permissive license, effortless GPU texture blitting for PDF tiles. |
| **Slint** (Rust) | Native Retained DSL | Native Direct3D / Metal / Skia | Supported (Cargo / X11 / Wayland) | **First-Class** (Canvas / WebGL) | FemtoVG / Skia / Software | Dual (GPLv3 / Royalty-Free / Commercial) | Declarative UI, but dual-licensing requires care for commercial use. |
| **Tauri v2** (Rust + Web) | Hybrid (Rust + Webview) | Native WebView2 / WebKit | Supported via WebKitGTK | Standalone web app | Browser DOM / Canvas | MIT / Apache-2.0 | IPC serialization overhead when streaming 4K page bitmaps to JavaScript. |

---

## 4. Subsystem Class & Module Contracts

```mermaid
classDiagram
    class DocumentSession {
        +open_file(path)
        +close()
        +get_page_count() int
        +get_page_size(page_idx) Size
        +render_tile_async(tile_desc, callback)
    }

    class RenderEngine {
        -pdfium_instance
        -worker_threads
        +rasterize_page_slice(page_idx, rect, dpi) Bitmap
    }

    class RedactionManager {
        +mark_redaction_zone(rect)
        +apply_true_redaction(stream_parser) Document
    }

    class FormManager {
        +get_form_fields(page_idx) List~FormField~
        +set_field_value(field_id, value)
    }

    class SignatureService {
        +add_visual_signature(page_idx, bezier_strokes)
        +apply_pades_digital_signature(cert, private_key)
    }

    DocumentSession --> RenderEngine
    DocumentSession --> RedactionManager
    DocumentSession --> FormManager
    DocumentSession --> SignatureService
```
