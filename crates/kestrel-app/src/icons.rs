//! Semantic icon definitions for Kestrel-PDF using bundled Phosphor vector icon font.
//!
//! Every icon is backed by a TrueType vector glyph from `egui-phosphor`, ensuring crisp,
//! pixel-perfect rendering across Windows, macOS, Linux, and WebAssembly without relying
//! on host OS emojis or system fonts.

use egui_phosphor::regular;

/// Application logo and brand symbol (Kestrel falcon / bird vector)
pub const APP_LOGO: &str = regular::BIRD;

/// File operations
pub const OPEN_FILE: &str = regular::FOLDER_OPEN;
pub const SAVE_FILE: &str = regular::FLOPPY_DISK;

/// Navigation & Orientation
pub const PREV_PAGE: &str = regular::CARET_LEFT;
pub const NEXT_PAGE: &str = regular::CARET_RIGHT;
pub const CARET_LEFT: &str = regular::CARET_LEFT;
pub const CARET_RIGHT: &str = regular::CARET_RIGHT;
pub const ROTATE_CCW: &str = regular::ARROW_COUNTER_CLOCKWISE;
pub const ROTATE_CW: &str = regular::ARROW_CLOCKWISE;
pub const SIDEBAR: &str = regular::SIDEBAR_SIMPLE;

/// Primary interactive tools
pub const TOOL_PAN: &str = regular::HAND_PALM;
pub const TOOL_SELECT: &str = regular::CURSOR_TEXT;
pub const TOOL_FORMS: &str = regular::TEXTBOX;
pub const TOOL_EDIT_TEXT: &str = regular::PENCIL_SIMPLE;
pub const TOOL_EDIT_IMAGE: &str = regular::IMAGE;
pub const TOOL_SIGN: &str = regular::SIGNATURE;
pub const TOOL_REDACT: &str = regular::SHIELD;
pub const DUPLICATE: &str = regular::COPY;
pub const CARET_UP: &str = regular::CARET_UP;
pub const CARET_DOWN: &str = regular::CARET_DOWN;

/// Selection actions
pub const COPY_TEXT: &str = regular::COPY;
pub const COPY_IMAGE: &str = regular::IMAGE;

/// Zoom & View
pub const ZOOM_IN: &str = regular::PLUS;
pub const ZOOM_OUT: &str = regular::MINUS;
pub const SEARCH: &str = regular::MAGNIFYING_GLASS;

/// Notifications & Status
pub const INFO: &str = regular::INFO;
pub const CLOSE: &str = regular::X;
pub const ADD: &str = regular::PLUS;
pub const TRASH: &str = regular::TRASH;
pub const UNDO: &str = regular::ARROW_U_UP_LEFT;
pub const LOCK: &str = regular::LOCK;
pub const CHECK: &str = regular::CHECK;
pub const CHECK_CIRCLE: &str = regular::CHECK_CIRCLE;
