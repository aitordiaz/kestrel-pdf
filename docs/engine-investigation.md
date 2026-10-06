# PDF Open-Source Engines & Readers Investigation

## Executive Summary

Building the most performant PDF reader and editor requires understanding the trade-offs between rendering speed, memory consumption, standards compliance, editing capabilities, and software licensing.

PDF is not merely an image format; it is a complex Turing-complete-adjacent display description language supporting PostScript-like vector operations, font embedding, raster streams, interactive forms, digital signatures, and object incremental updates.

---

## 1. Core PDF Engines Benchmark & Comparison

| Engine | Primary Language | License | Raw Rendering Speed | Memory Overhead | Form Filling (AcroForms) | Text & Image Editing | True Redaction | Commercial Friendly? |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **PDFium** | C++ | BSD 3-Clause / Apache 2.0 | **Ultra Fast** (SIMD optimized) | Low (~25-50 MB) | Native (Full support) | Object-level APIs (requires wrapper) | Requires custom content stream sanitizer | **Yes** (Free & Unrestricted) |
| **MuPDF** | C | **AGPLv3** (Commercial license $$$) | **Ultra Fast** (Minimalist) | Very Low (~15-30 MB) | Native (Full support) | Annotations & basic stream editing | Native (`fz_apply_redactions`) | **No** (AGPL requires open-sourcing or paying Artifex) |
| **Poppler** | C++ | **GPLv2/v3** | Fast | Moderate (~50-100 MB) | Native (Cairo/Splash) | Limited | Manual stream surgery | **No** (GPL viral copyleft) |
| **lopdf** | Rust | MIT | N/A (Parser/Writer only, no rasterizer) | Minimal (~5-10 MB) | Manual dictionary ops | Full low-level AST manipulation | Excellent for object stripping | **Yes** (MIT) |
| **PDF.js** | JavaScript | Apache 2.0 | Moderate / Slow on large files | High (200-800 MB V8 Heap) | Basic / Partial | Annotations only | Visual only (Dangerous) | **Yes** |

---

## 2. In-Depth Analysis of Leading Engines

### A. PDFium (Google / Chromium)
* **Used by**: Google Chrome, Microsoft Edge, Android, Electron PDF viewer.
* **Architecture**: Originally developed by Foxit, open-sourced by Google in 2014. Contains highly tuned C++ rendering pipelines with SSE2/AVX2 on x86_64 and NEON on ARM64.
* **Strengths**:
  - Unrivaled standard compliance: Handles millions of real-world "broken" or non-standard PDFs gracefully.
  - Permissive BSD-3 license: Can be embedded in commercial or closed-source applications without royalty.
  - Native AcroForm & XFA support.
  - Battle-tested security fuzzing via Chromium ClusterFuzz.
* **Weaknesses**:
  - High-level text reflow and WYSIWYG editing are not built-in; object-level mutation APIs are low-level.
  - Native Redaction API is not built-in; redacting requires combining PDFium with a structural stream rewriter (e.g., `lopdf` or custom PDF stream filter).

### B. MuPDF (Artifex Software)
* **Used by**: SumatraPDF, Sioyek, PyMuPDF (Fitz).
* **Architecture**: Written in clean, ANSI C with its own graphics rasterizer (Fitz).
* **Strengths**:
  - Blazing startup and rasterization speed.
  - Built-in, production-grade **True Redaction**: The `fz_apply_redactions` API actually parses vector glyph instructions, removes matching glyphs, clips raster images, and purges the deleted data from the document body.
* **Critical Drawback**:
  - **License is AGPLv3**. If you distribute an application using MuPDF, you must either open-source your entire application under AGPLv3 or purchase a costly commercial license from Artifex.

### C. Poppler (Freedesktop.org / Xpdf fork)
* **Used by**: Okular, Evince, GIMP.
* **Strengths**: Standard Linux rendering engine with Cairo / Qt backends.
* **Weaknesses**: Slower than PDFium and MuPDF; GPLv2/v3 license creates copyleft restrictions.

---

## 3. Notable Open-Source PDF Readers Analyzed

### 1. SumatraPDF (Windows)
* **Tech Stack**: C++ / Win32 API / MuPDF engine.
* **Why it's fast**:
  - Zero heavy framework overhead (direct native Win32/GDI+ calls).
  - Memory virtualization: Only visible pages are rasterized and kept in a bounded LRU bitmap cache.
  - Cold startup time is under 80ms; memory footprint is often under 25MB.
* **Limitations**: Windows-only; view/annotation-focused, lacks in-place text reflow editing.

### 2. Sioyek (macOS / Windows / Linux)
* **Tech Stack**: C++ / Qt / OpenGL / MuPDF.
* **Why it's fast**: Offloads page blitting to OpenGL textures, giving 60-120 FPS continuous panning.
* **Focus**: Academic / research navigation (portals, equation jump), not editing.

### 3. Okular (KDE / Cross-platform)
* **Tech Stack**: C++ / Qt 5/6 / Poppler.
* **Strengths**: Modular generator architecture, digital signatures, extensive annotation support.
* **Weaknesses**: Heavier startup overhead and higher memory footprint than SumatraPDF.

---

## 4. Recommended Engine Strategy for Velox-PDF

To achieve **maximum performance**, **unrestricted licensing (permissive)**, and **complete feature support**:

1. **Rendering & Form Filling**: **PDFium** (via precompiled C++ binaries or Rust `pdfium-render` crate). Gives sub-millisecond page rendering and native AcroForm interaction.
2. **Structural Manipulation & True Redaction**: **`lopdf` (Rust)** or custom PDF object stream filter. Inspects the PDF object tree, locates text runs (`BT ... ET`, `Tj`, `TJ`), deletes sensitive glyphs and vector paths, chops and re-encodes image streams, and removes metadata before export.
3. **Digital Signatures**: **PAdES / PKCS#7 signing module** using cryptographic standards (OpenSSL / `ring` / Windows CryptoAPI / macOS Security framework).
