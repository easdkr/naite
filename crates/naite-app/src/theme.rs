//! Design tokens for naite.
//!
//! The visual language is almost-monochrome dark with a single restrained
//! accent, stepped surfaces, hairline borders, and typography weight carrying
//! the visual hierarchy.

use iced::theme::Palette;
use iced::{Color, Theme};

pub mod color {
    use iced::Color;

    // Stepped surfaces (deepest → top).
    pub const BG: Color = Color {
        r: 0.031,
        g: 0.031,
        b: 0.043,
        a: 1.0,
    }; // #08080B
    pub const SURFACE_1: Color = Color {
        r: 0.086,
        g: 0.086,
        b: 0.114,
        a: 1.0,
    }; // #16161D
    pub const SURFACE_2: Color = Color {
        r: 0.122,
        g: 0.122,
        b: 0.149,
        a: 1.0,
    }; // #1F1F26
    /// Dedicated surface for the docked terminal panel. Sits between
    /// SURFACE_1 (chrome) and SURFACE_2 (chips/selection) so the panel
    /// reads as a distinct surface class from both the sidebar/detail
    /// chrome and the commit list (BG), without colliding with the
    /// hover/selection tones used inside it.
    pub const SURFACE_TERMINAL: Color = Color {
        r: 0.106,
        g: 0.106,
        b: 0.133,
        a: 1.0,
    }; // #1B1B22
    #[allow(dead_code)]
    pub const SURFACE_3: Color = Color {
        r: 0.165,
        g: 0.165,
        b: 0.200,
        a: 1.0,
    }; // #2A2A33

    /// Quiet 1px border; sits just above SURFACE_3 in luminance.
    pub const BORDER: Color = Color {
        r: 0.180,
        g: 0.180,
        b: 0.220,
        a: 1.0,
    }; // #2E2E38

    // Text scale — three levels are usually enough.
    pub const TEXT: Color = Color {
        r: 0.945,
        g: 0.945,
        b: 0.953,
        a: 1.0,
    }; // #F1F1F3
    pub const TEXT_MUTED: Color = Color {
        r: 0.631,
        g: 0.631,
        b: 0.667,
        a: 1.0,
    }; // #A1A1AA
    pub const TEXT_SUBTLE: Color = Color {
        r: 0.431,
        g: 0.431,
        b: 0.471,
        a: 1.0,
    }; // #6E6E78

    /// Sole brand accent. Use sparingly — selection bars, primary action,
    /// status when nothing else will do.
    pub const ACCENT: Color = Color {
        r: 0.369,
        g: 0.416,
        b: 0.820,
        a: 1.0,
    }; // #5E6AD2

    // Semantic accents — used for state, never decoration.
    pub const SUCCESS: Color = Color {
        r: 0.298,
        g: 0.733,
        b: 0.510,
        a: 1.0,
    }; // #4CBB82
    pub const WARNING: Color = Color {
        r: 0.918,
        g: 0.667,
        b: 0.235,
        a: 1.0,
    }; // #EAAA3C
    pub const DANGER: Color = Color {
        r: 0.847,
        g: 0.412,
        b: 0.388,
        a: 1.0,
    }; // #D86963

    /// Reserved for the future commit-graph canvas (gix-driven lane assignment).
    /// Not used by ordinary widgets in this build — keep the chrome calm.
    #[allow(dead_code)]
    pub const LANES: [Color; 5] = [
        ACCENT,
        Color {
            r: 0.212,
            g: 0.773,
            b: 0.941,
            a: 1.0,
        }, // cyan
        WARNING,
        SUCCESS,
        Color {
            r: 0.808,
            g: 0.404,
            b: 0.745,
            a: 1.0,
        }, // magenta
    ];

    // Syntax highlighting palette tuned to stay quiet beside SUCCESS/DANGER
    // row tints. SYNTAX_STRING is
    // intentionally a different green (sage) from SUCCESS so added strings do
    // not disappear into the Add row background.
    #[allow(dead_code)]
    pub const SYNTAX_KEYWORD: Color = Color {
        r: 0.780,
        g: 0.573,
        b: 0.918,
        a: 1.0,
    }; // #C792EA — muted lavender
    #[allow(dead_code)]
    pub const SYNTAX_TYPE: Color = Color {
        r: 0.510,
        g: 0.667,
        b: 1.0,
        a: 1.0,
    }; // #82AAFF — soft blue
    #[allow(dead_code)]
    pub const SYNTAX_STRING: Color = Color {
        r: 0.765,
        g: 0.906,
        b: 0.553,
        a: 1.0,
    }; // #C3E88D — muted sage (distinct from SUCCESS)
    #[allow(dead_code)]
    pub const SYNTAX_NUMBER: Color = Color {
        r: 0.969,
        g: 0.549,
        b: 0.424,
        a: 1.0,
    }; // #F78C6C — warm peach
    #[allow(dead_code)]
    pub const SYNTAX_COMMENT: Color = Color {
        r: 0.482,
        g: 0.494,
        b: 0.549,
        a: 1.0,
    }; // #7B7E8C — just above TEXT_SUBTLE
    #[allow(dead_code)]
    pub const SYNTAX_FUNCTION: Color = Color {
        r: 0.749,
        g: 0.792,
        b: 1.0,
        a: 1.0,
    }; // #BFCAFF — pale indigo, near-accent

    /// Apply alpha to a base color while keeping its hue.
    pub const fn with_alpha(base: Color, a: f32) -> Color {
        Color {
            r: base.r,
            g: base.g,
            b: base.b,
            a,
        }
    }
}

// Spacing scale (multiples of 4).
#[allow(dead_code)]
pub const SP_XS: u16 = 4;
pub const SP_SM: u16 = 8;
pub const SP_MD: u16 = 12;
pub const SP_LG: u16 = 16;
#[allow(dead_code)]
pub const SP_XL: u16 = 24;

// Type scale (px), intentionally tight for dense desktop UI.
pub const FS_XS: u16 = 10;
pub const FS_SM: u16 = 11;
pub const FS_BASE: u16 = 13;
#[allow(dead_code)]
pub const FS_MD: u16 = 14;
#[allow(dead_code)]
pub const FS_LG: u16 = 16;
pub const FS_XL: u16 = 19;

// Radius scale: small radii almost everywhere.
#[allow(dead_code)]
pub const R_SM: f32 = 3.0;
pub const R_MD: f32 = 5.0;
#[allow(dead_code)]
pub const R_LG: f32 = 8.0;
pub const R_PILL: f32 = 999.0;

// Layout, history, and timing tokens — single source of truth for status
// bars, modals, toasts, the operation tracker, and central overlay
// thresholds consumed by Wave 2+ feature work.
pub const MIN_WINDOW_WIDTH: f32 = 1024.0;
pub const MIN_WINDOW_HEIGHT: f32 = 640.0;
pub const STATUS_BAR_HEIGHT: f32 = 24.0;
pub const MAX_MODAL_HEIGHT: f32 = 600.0;
pub const OVERLAY_TRIGGER_SECS: u64 = 2;
pub const OP_HISTORY_CAP: usize = 50;
pub const TOAST_SUCCESS_TTL_SECS: u64 = 3;

/// Custom naite dark theme registered with iced.
pub fn naite_dark() -> Theme {
    Theme::custom(
        "naite Dark".to_string(),
        Palette {
            background: color::BG,
            text: color::TEXT,
            primary: color::ACCENT,
            success: color::SUCCESS,
            danger: color::DANGER,
        },
    )
}

pub fn naite_high_contrast() -> Theme {
    Theme::custom(
        "naite High Contrast".to_string(),
        Palette {
            background: color::BG,
            text: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            },
            primary: color::ACCENT,
            success: color::SUCCESS,
            danger: color::DANGER,
        },
    )
}

/// Family used for all non-monospace text. macOS ships Apple SD Gothic Neo
/// which has full Hangul coverage; on other platforms fontdb will fall back
/// to the closest available sans-serif.
const UI_FAMILY: iced::font::Family = iced::font::Family::Name("Apple SD Gothic Neo");

/// Default UI font — routes through a Hangul-capable family so Korean
/// commit messages, author names, branch labels etc. render correctly.
pub fn font_regular() -> iced::Font {
    iced::Font {
        family: UI_FAMILY,
        ..iced::Font::DEFAULT
    }
}

/// Semibold variant used for emphasis (active branch, selected commit).
pub fn font_semibold() -> iced::Font {
    iced::Font {
        family: UI_FAMILY,
        weight: iced::font::Weight::Semibold,
        ..iced::Font::DEFAULT
    }
}

/// Code-like text. Diffs and blame depend on monospace alignment of leading
/// whitespace and `+`/`-` prefixes; alignment is non-negotiable for code, so
/// use the system monospace family. cosmic-text falls back per-glyph for
/// Hangul characters that the monospace family lacks.
pub fn font_code() -> iced::Font {
    iced::Font::MONOSPACE
}
