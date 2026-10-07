# ADR-0005: Modern UI Style Guide & Professional Design System

- **Status**: Accepted
- **Date**: 2026-10-08
- **Author**: Antigravity (Advanced Agentic Coding)

---

## 1. Context & Problem Statement

Kestrel-PDF is built as a high-performance universal PDF reader and editor. In early MVP iterations, the application interface prioritized functional verification (rendering tiles, basic toolbar, left sidebar listing all pages). However:
1. **Low Visual Distinction & Contrast**: Standard dark/light mode buttons lacked visual hierarchy, causing primary actions (such as opening a file) to blend into minor utility tools.
2. **Left Sidebar Viewport Theft**: The permanent left sidebar displaying page numbers and titles consumed substantial horizontal viewport space (220px+), distracting from the user's primary goal: focused, high-fidelity PDF reading.
3. **Absence of Unified Design Tokens**: Colors, paddings, corner radii, and button states were declared ad-hoc without a coherent design system.

The application requires an elevated, modern, professional design system centered on:
- High contrast and professional aesthetics with warm salmon / light ochre primary accents.
- High affordance for primary actions ("Abrir fichero" / "Open File").
- A clean, distraction-free document canvas displaying the PDF "as is" with zero color distortion.
- An in-flow, interactive Page Navigator replacing the noisy left sidebar page list.

---

## 2. Design System Tokens & Color Palette

### 2.1 The "Warm Ochre & Salmon" Professional Palette

The palette combines deep, neutral slate backgrounds with warm terracotta/salmon and light ochre accents, calibrated for WCAG AAA/AA contrast against both dark and light surfaces:

| Token Name | Hex Code | egui `Color32` | Usage Description |
| :--- | :--- | :--- | :--- |
| **Accent Primary (Salmon)** | `#D97757` | `Color32::from_rgb(217, 119, 87)` | Primary CTA buttons ("Abrir fichero"), active states, focus rings. |
| **Accent Primary Hover** | `#E58B6D` | `Color32::from_rgb(229, 139, 109)` | Hover state for primary action buttons. |
| **Accent Primary Pressed**| `#BF6243` | `Color32::from_rgb(191, 98, 67)` | Pressed / active state for primary action buttons. |
| **Accent Secondary (Ochre)**| `#E28743` | `Color32::from_rgb(226, 135, 67)` | Secondary accents, badges, highlights, stepper hovers. |
| **Accent Subtle Ochre** | `#FBF3EE` | `Color32::from_rgb(251, 243, 238)` | Light badge backgrounds, selection tint in light mode. |
| **Surface Dark (Header/Bars)**| `#0F172A`| `Color32::from_rgb(15, 23, 42)` | Main top toolbar, window frame, command bars. |
| **Surface Panel Dark** | `#1E293B` | `Color32::from_rgb(30, 41, 59)` | Tool clusters, segmented containers, popups, navigator pill. |
| **Surface Border Dark** | `#334155` | `Color32::from_rgb(51, 65, 85)` | High-contrast borders and dividers. |
| **Text Primary (Dark Theme)** | `#F8FAFC`| `Color32::from_rgb(248, 250, 252)`| Headings, primary labels, button text on dark surfaces. |
| **Text Secondary (Dark Theme)**| `#94A3B8`| `Color32::from_rgb(148, 163, 184)`| Metadata, hotkey hints, page counter denominators (`/ 4`). |
| **Canvas Surround (Backdrop)**| `#334155`| `Color32::from_rgb(51, 65, 85)` | Neutral background surrounding the document page. |
| **PDF Page Canvas** | `White` | `Color32::WHITE` | Clean PDF rendering canvas; zero tinting or filtering applied. |

### 2.2 Typography Hierarchy

- **Title Bar & Document Name**: 14pt semi-bold, middle-truncated with full path tooltip.
- **Primary CTA ("Abrir Fichero")**: 13pt bold, proportional.
- **Page Navigator Counter**: 13pt monospace/semi-bold for page input (`[ 1 ]`), 12pt regular for `/ N`.
- **Toolbar Action Labels**: 11pt medium.
- **Tooltips & Keybinds**: 11pt regular / 10pt muted.

### 2.3 Spacing, Radii & Elevations

- **Button Border Radius**: `6.0px` standard, `8.0px` for Primary CTA.
- **Container Pill Radius**: `8.0px` for Page Navigator and Tool Groups.
- **Paddings**:
  - Primary CTA: `Vec2::new(14.0, 7.0)` (Comfortable, large click target).
  - Compact Steppers: `Vec2::new(8.0, 5.0)`.
  - Navigator Input: `Vec2::new(6.0, 4.0)` with minimum width `38.0px`.

---

## 3. Component Architecture & Specifications

### 3.1 Primary Action: "Abrir Fichero" Button

```
+----------------------------------------------------+
|  [icon] Abrir fichero               (Ctrl+O / Cmd+O) |
+----------------------------------------------------+
```
- **Visual Stance**: Solid salmon fill (`#D97757`), crisp white text (`#FFFFFF`), subtle shadow/stroke.
- **Placement**:
  - Top toolbar (leftmost action next to document status).
  - Center stage of Empty Document State (large card with drag-and-drop affordance).
- **Affordance**: Always visible, never hidden behind sub-menus or ellipsis menus.

### 3.2 In-Flow Page Navigator Widget

Replacing the permanent left sidebar page list with a sleek, centered in-flow navigation pill:

```
+-----------------------------------------+
|   [ < ]   [ 1 ]   /   142   [ > ]       |
+-----------------------------------------+
```
- **Elements**:
  1. **Previous Button (`<`)**: Decrements page index if `page > 1`. Disabled / dim if on page 1.
  2. **Page Input Box (`[ 1 ]`)**:
     - Interactive editable text edit widget.
     - Single-click or keyboard focus selects all text.
     - Typing a number `X` and pressing `Enter` (or losing focus) jumps to page `X`.
     - Clamped safely to `1..=total_pages`.
  3. **Separator & Denominator (`/ 142`)**: Muted secondary text clearly communicating total length.
  4. **Next Button (`>`)**: Increments page index if `page < total`. Disabled / dim if on last page.
- **Location**: Centered in the primary toolbar for immediate thumb/eye accessibility.

### 3.3 Canvas Focus Policy ("PDF As Is")

- **Zero Occlusion**: The left sidebar is closed by default. The document viewer occupies up to 100% of the window width.
- **Color Integrity**: No post-processing, color inversion, or tint is applied to the PDF page raster or vector content.
- **Neutral Backdrop**: Deep slate surround (`#334155`) isolates white PDF page boundaries with a clean 2px drop shadow.

---

## 4. Consequences & Benefits

- **Focused Reading Experience**: Eliminates 220px of wasted horizontal space previously consumed by page lists.
- **Instant Page Jumps**: Users can jump from page 1 to 250 in two keystrokes rather than scrolling through hundreds of sidebar entries.
- **Brand Identity**: Warm salmon and light ochre accents establish a distinct, elegant, professional look while maintaining WCAG AA contrast.
- **Cross-Platform Native Feel**: Touch-friendly button sizes on mobile/tablets, keyboard-first shortcuts for power users.
