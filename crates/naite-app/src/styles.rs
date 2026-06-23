//! Style primitives — closures that translate design tokens into the
//! per-widget `Style` structs `iced` consumes via `.style(...)`.
//!
//! Widgets in `widgets.rs` should compose these by name; never hand-roll
//! a `container::Style` / `button::Style` literal at the call site.

use iced::overlay::menu;
use iced::widget::{button, container, pick_list, scrollable, text_editor, text_input};
use iced::{Background, Border, Color, Theme};

use crate::theme::{self, color};

// ---------- container surfaces ----------

/// Surface 1 panel (toolbar, sidebar, detail pane backgrounds).
pub fn surface_panel(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::SURFACE_1)),
        ..Default::default()
    }
}

/// Docked sub-panel (terminal panel). Paints on the dedicated
/// SURFACE_TERMINAL tone so it reads as a distinct surface class from
/// both the BG commit list above it and the SURFACE_1 sidebar/detail
/// chrome on the sides — without needing a hairline border. Top corners
/// stay rounded; the panel is flush against the window bottom, so
/// rounded bottom corners would leak through transparent corner pixels.
pub fn floating_panel(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::SURFACE_TERMINAL)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: iced::border::Radius::default().top(theme::R_SM),
        },
        ..Default::default()
    }
}

/// Quiet pill used inside floating panel headers to surface secondary
/// info (e.g. the running shell's reported title) without competing with
/// the panel chrome.
pub fn header_chip(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::SURFACE_2)),
        border: Border {
            color: color::BORDER,
            width: 1.0,
            radius: theme::R_PILL.into(),
        },
        ..Default::default()
    }
}

/// Bare app background — main pane behind the commit list.
pub fn bg_panel(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::BG)),
        ..Default::default()
    }
}

/// Pill chip used for the active-branch indicator in the toolbar.
pub fn pill_chip(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::SURFACE_2)),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: theme::R_PILL.into(),
        },
        ..Default::default()
    }
}

pub fn status_badge(accent: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(color::with_alpha(accent, 0.12))),
        border: Border {
            color: color::with_alpha(accent, 0.30),
            width: 1.0,
            radius: theme::R_PILL.into(),
        },
        ..Default::default()
    }
}

pub fn graph_ref_pill(
    background: Color,
    border_color: Color,
    text_color: Color,
    radius: f32,
) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(background)),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: radius.into(),
        },
        text_color: Some(text_color),
        ..Default::default()
    }
}

/// Card used for compact placeholders and similar inset surfaces.
pub fn inset_card(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::SURFACE_2)),
        border: Border {
            color: color::BORDER,
            width: 1.0,
            radius: theme::R_MD.into(),
        },
        ..Default::default()
    }
}

pub fn rebase_prompt_preview_surface(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::BG)),
        border: Border {
            color: color::BORDER,
            width: 1.0,
            radius: theme::R_MD.into(),
        },
        ..Default::default()
    }
}

pub fn rebase_prompt_preview_row(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        ..Default::default()
    }
}

/// Action chip in the rebase confirmation preview, tinted with the action's
/// color so each operation kind reads at a glance (Linear-style tag pill).
pub fn rebase_prompt_action_chip(tint: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(color::with_alpha(tint, 0.12))),
        border: Border {
            color: color::with_alpha(tint, 0.25),
            width: 1.0,
            radius: theme::R_SM.into(),
        },
        ..Default::default()
    }
}

/// Danger-tinted card used for error messages.
pub fn error_card(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::with_alpha(color::DANGER, 0.08))),
        border: Border {
            color: color::with_alpha(color::DANGER, 0.30),
            width: 1.0,
            radius: theme::R_MD.into(),
        },
        ..Default::default()
    }
}

pub fn warning_card(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::with_alpha(color::WARNING, 0.10))),
        border: Border {
            color: color::with_alpha(color::WARNING, 0.35),
            width: 1.0,
            radius: theme::R_MD.into(),
        },
        ..Default::default()
    }
}

pub fn selected_hunk_header(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::with_alpha(color::ACCENT, 0.10))),
        border: Border {
            color: color::with_alpha(color::ACCENT, 0.35),
            width: 1.0,
            radius: theme::R_MD.into(),
        },
        ..Default::default()
    }
}

pub fn diff_add(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::with_alpha(color::SUCCESS, 0.08))),
        ..Default::default()
    }
}

pub fn diff_del(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::with_alpha(color::DANGER, 0.08))),
        ..Default::default()
    }
}

pub fn diff_ctx(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        ..Default::default()
    }
}

/// Chrome surface used above and between row lists (rebase toolbar, column
/// headers, commit list header). Reads as a chrome plate that lifts above the
/// BG row area through surface tone alone — no border, no grid line — so
/// stacked headers don't produce the table-grid effect iced's all-sides
/// `Border` would otherwise force.
pub fn commit_list_header(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::SURFACE_1)),
        ..Default::default()
    }
}

/// Factory: a solid-color 2px bar used as the active indicator on the left
/// edge of sidebar items and commit rows. The same factory feeds both
/// active/selected and inactive variants by closing over the desired color.
pub fn solid_bar(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(color)),
        ..Default::default()
    }
}

// ---------- scrollable ----------

pub fn thin_scrollbar(_: &Theme, status: scrollable::Status) -> scrollable::Style {
    let scroller_alpha = match status {
        scrollable::Status::Active => 0.0,
        scrollable::Status::Hovered {
            is_vertical_scrollbar_hovered,
            ..
        } => {
            if is_vertical_scrollbar_hovered {
                0.55
            } else {
                0.25
            }
        }
        scrollable::Status::Dragged { .. } => 0.65,
    };

    let rail = scrollable::Rail {
        background: None,
        border: Border::default(),
        scroller: scrollable::Scroller {
            color: color::with_alpha(color::TEXT_MUTED, scroller_alpha),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: theme::R_SM.into(),
            },
        },
    };

    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
    }
}

pub fn thin_scrollbar_dir() -> scrollable::Direction {
    scrollable::Direction::Vertical(
        scrollable::Scrollbar::new()
            .width(6)
            .scroller_width(4)
            .margin(0),
    )
}

pub fn release_prep_pick_list(_: &Theme, status: pick_list::Status) -> pick_list::Style {
    let (background, border_color) = match status {
        pick_list::Status::Active => (color::BG, color::with_alpha(color::TEXT_SUBTLE, 0.72)),
        pick_list::Status::Hovered => {
            (color::SURFACE_1, color::with_alpha(color::TEXT_MUTED, 0.82))
        }
        pick_list::Status::Opened => (color::SURFACE_1, color::ACCENT),
    };

    pick_list::Style {
        text_color: color::TEXT,
        placeholder_color: color::TEXT_SUBTLE,
        handle_color: color::TEXT_MUTED,
        background: Background::Color(background),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: theme::R_MD.into(),
        },
    }
}

pub fn release_prep_pick_list_menu(_: &Theme) -> menu::Style {
    menu::Style {
        background: Background::Color(color::SURFACE_2),
        border: Border {
            color: color::BORDER,
            width: 1.0,
            radius: theme::R_MD.into(),
        },
        text_color: color::TEXT,
        selected_text_color: Color::WHITE,
        selected_background: Background::Color(color::ACCENT),
    }
}

/// Factory: rebase row action chip as a pick_list. Reads as a chip in the
/// resting state — SURFACE_2 fill, no border — and only takes on a 1px ACCENT
/// outline while its menu is open so the active row is unambiguous. The
/// action's semantic color carries the closed-state cue (purple/yellow/red).
pub fn rebase_action_pick_list(
    label_color: Color,
) -> impl Fn(&Theme, pick_list::Status) -> pick_list::Style {
    move |_, status| {
        let (background, border_color, border_width) = match status {
            pick_list::Status::Active => (color::SURFACE_2, Color::TRANSPARENT, 0.0),
            pick_list::Status::Hovered => (color::SURFACE_3, Color::TRANSPARENT, 0.0),
            pick_list::Status::Opened => (color::SURFACE_3, color::ACCENT, 1.0),
        };
        pick_list::Style {
            text_color: label_color,
            placeholder_color: color::TEXT_SUBTLE,
            handle_color: color::with_alpha(color::TEXT_MUTED, 0.7),
            background: Background::Color(background),
            border: Border {
                color: border_color,
                width: border_width,
                radius: theme::R_SM.into(),
            },
        }
    }
}

/// Form text input: deep BG surface, hairline border that lights up to ACCENT
/// on focus. Mirrors the pick_list aesthetic so inputs and selectors read as
/// one system.
pub fn form_text_input(_: &Theme, status: text_input::Status) -> text_input::Style {
    let (background, border_color) = match status {
        text_input::Status::Active => (color::BG, color::BORDER),
        text_input::Status::Hovered => (color::BG, color::with_alpha(color::TEXT_SUBTLE, 0.72)),
        text_input::Status::Focused => (color::BG, color::ACCENT),
        text_input::Status::Disabled => (color::SURFACE_1, color::BORDER),
    };

    text_input::Style {
        background: Background::Color(background),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: theme::R_MD.into(),
        },
        icon: color::TEXT_MUTED,
        placeholder: color::TEXT_SUBTLE,
        value: color::TEXT,
        selection: color::with_alpha(color::ACCENT, 0.32),
    }
}

/// Multi-line form editor; same surface and border treatment
/// as [`form_text_input`] so single-line and multi-line surfaces
/// stack together without a visual seam.
pub fn form_text_editor(_: &Theme, status: text_editor::Status) -> text_editor::Style {
    let (background, border_color) = match status {
        text_editor::Status::Active => (color::BG, color::BORDER),
        text_editor::Status::Hovered => (color::BG, color::with_alpha(color::TEXT_SUBTLE, 0.72)),
        text_editor::Status::Focused => (color::BG, color::ACCENT),
        text_editor::Status::Disabled => (color::SURFACE_1, color::BORDER),
    };

    text_editor::Style {
        background: Background::Color(background),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: theme::R_MD.into(),
        },
        icon: color::TEXT_MUTED,
        placeholder: color::TEXT_SUBTLE,
        value: color::TEXT,
        selection: color::with_alpha(color::ACCENT, 0.32),
    }
}

pub fn status_row_container(selected: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(if selected {
            color::SURFACE_2
        } else {
            Color::TRANSPARENT
        })),
        ..Default::default()
    }
}

/// Plan row background for the interactive rebase editor. While a drag is
/// in flight and this row is the source, dim it so the user sees the ghost
/// overlay carry the "real" row instead.
pub fn rebase_row_container(
    selected: bool,
    is_drag_source: bool,
) -> impl Fn(&Theme) -> container::Style {
    move |_| {
        let bg = if is_drag_source {
            color::with_alpha(color::SURFACE_1, 0.35)
        } else if selected {
            color::SURFACE_2
        } else {
            Color::TRANSPARENT
        };
        container::Style {
            background: Some(Background::Color(bg)),
            ..Default::default()
        }
    }
}

/// Floating ghost row used as drag-and-drop affordance in the rebase editor.
/// Translucent surface with a subtle accent border so it reads as "the thing
/// being dragged" without being mistaken for a real row.
pub fn ghost_row(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::with_alpha(color::SURFACE_2, 0.92))),
        border: Border {
            color: color::with_alpha(color::ACCENT, 0.55),
            width: 1.0,
            radius: theme::R_SM.into(),
        },
        ..Default::default()
    }
}

/// Faint chip background used for the action label inside a ghost row.
pub fn ghost_action_chip(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::with_alpha(color::SURFACE_1, 0.85))),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: theme::R_SM.into(),
        },
        ..Default::default()
    }
}

// ---------- progress overlay ----------

/// Semi-transparent backdrop used behind the central progress overlay
/// (Task 15). Dims the underlying UI without capturing pointer events —
/// unlike the modal primitive, the overlay is non-blocking (no
/// click-to-dismiss; the trigger condition in Task 20 removes it when
/// the operation completes). The alpha is static (no entry animation);
/// the animated motion lives on the moving bar inside the card.
pub fn progress_overlay_backdrop(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::with_alpha(color::BG, 0.55))),
        ..Default::default()
    }
}

/// Elevated surface card used to host the central progress overlay
/// (spinner + label + step counter). Sits one step above the dimmed
/// backdrop via SURFACE_2 fill + hairline BORDER + R_MD, matching the
/// modal's card surface family so the two feel like the same system.
pub fn progress_overlay_card(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::SURFACE_2)),
        border: Border {
            color: color::BORDER,
            width: 1.0,
            radius: theme::R_MD.into(),
        },
        ..Default::default()
    }
}

// ---------- buttons ----------

/// Primary CTA button (Open repository …).
pub fn primary_button(_: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => color::with_alpha(color::ACCENT, 0.92),
        button::Status::Pressed => color::with_alpha(color::ACCENT, 0.80),
        button::Status::Disabled => color::with_alpha(color::SURFACE_2, 0.72),
        _ => color::ACCENT,
    };
    let (text_color, border_color, border_width) = if matches!(status, button::Status::Disabled) {
        (
            color::TEXT_MUTED,
            color::with_alpha(color::BORDER, 0.85),
            1.0,
        )
    } else {
        (Color::WHITE, Color::TRANSPARENT, 0.0)
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color,
        border: Border {
            color: border_color,
            width: border_width,
            radius: theme::R_MD.into(),
        },
        ..Default::default()
    }
}

/// Factory: commit-row button. Selected rows keep an SURFACE_2 background
/// across all interaction states; unselected rows lift slightly on hover.
pub fn commit_row_button(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let bg = if selected {
            color::SURFACE_2
        } else {
            match status {
                button::Status::Hovered => color::SURFACE_1,
                button::Status::Pressed => color::SURFACE_2,
                _ => Color::TRANSPARENT,
            }
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: color::TEXT,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        }
    }
}

/// Factory: command palette row button. The keyboard-selected row needs to read
/// as the active command without relying on text weight alone.
pub fn command_palette_button(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let bg = if selected {
            color::SURFACE_3
        } else {
            match status {
                button::Status::Hovered => color::SURFACE_1,
                button::Status::Pressed => color::SURFACE_2,
                _ => Color::TRANSPARENT,
            }
        };
        let border_color = if selected {
            color::with_alpha(color::ACCENT, 0.42)
        } else {
            Color::TRANSPARENT
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: color::TEXT,
            border: Border {
                color: border_color,
                width: if selected { 1.0 } else { 0.0 },
                radius: theme::R_MD.into(),
            },
            ..Default::default()
        }
    }
}

pub fn command_palette_selection_rail(selected: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(if selected {
            color::ACCENT
        } else {
            Color::TRANSPARENT
        })),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: theme::R_PILL.into(),
        },
        ..Default::default()
    }
}

pub fn command_palette_shortcut(selected: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(if selected {
            color::with_alpha(color::ACCENT, 0.14)
        } else {
            color::SURFACE_2
        })),
        border: Border {
            color: if selected {
                color::with_alpha(color::ACCENT, 0.30)
            } else {
                Color::TRANSPARENT
            },
            width: if selected { 1.0 } else { 0.0 },
            radius: theme::R_PILL.into(),
        },
        ..Default::default()
    }
}

/// Factory: sidebar ref row button. The caller-owned hover state is included
/// so branch rows update through explicit enter/exit events, not only through
/// iced's transient button status. Hover matches the pressed row treatment.
pub fn sidebar_ref_button(hovered: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let bg = match status {
            button::Status::Pressed => color::SURFACE_2,
            button::Status::Hovered if !hovered => color::SURFACE_2,
            _ if hovered => color::SURFACE_2,
            _ => Color::TRANSPARENT,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: color::TEXT,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        }
    }
}

/// Factory: segmented-control chip used in PR filter row.
/// Inactive chips sit on SURFACE_1; the active chip lifts to SURFACE_3 with
/// brighter text to read as the current segment. Small radius keeps the row
/// tight and flat.
pub fn segmented_chip(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let bg = if active {
            color::SURFACE_3
        } else {
            match status {
                button::Status::Hovered => color::SURFACE_2,
                button::Status::Pressed => color::SURFACE_3,
                button::Status::Disabled => color::with_alpha(color::SURFACE_1, 0.55),
                _ => color::SURFACE_1,
            }
        };
        let text_color = if matches!(status, button::Status::Disabled) {
            color::TEXT_SUBTLE
        } else if active {
            color::TEXT
        } else {
            color::TEXT_MUTED
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: theme::R_SM.into(),
            },
            ..Default::default()
        }
    }
}

/// Ghost-style button used in the top toolbar. Stays transparent at rest so a
/// row of nine actions reads as quiet chrome, and only lifts to SURFACE_2 on
/// hover. No visible border; hover state carries the clickable affordance.
pub fn toolbar_button(_: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => color::SURFACE_2,
        button::Status::Pressed => color::SURFACE_3,
        button::Status::Disabled => Color::TRANSPARENT,
        _ => Color::TRANSPARENT,
    };
    let text_color = if matches!(status, button::Status::Disabled) {
        color::TEXT_SUBTLE
    } else {
        color::TEXT
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: theme::R_MD.into(),
        },
        ..Default::default()
    }
}

pub fn subtle_button(_: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => color::SURFACE_3,
        button::Status::Pressed => color::SURFACE_2,
        button::Status::Disabled => color::with_alpha(color::SURFACE_2, 0.55),
        _ => color::SURFACE_2,
    };
    let text_color = if matches!(status, button::Status::Disabled) {
        color::TEXT_SUBTLE
    } else {
        color::TEXT
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color,
        border: Border {
            color: color::BORDER,
            width: 1.0,
            radius: theme::R_MD.into(),
        },
        ..Default::default()
    }
}

/// Factory: vertical-tab button used in the sidebar tab strip. The active
/// tab keeps a SURFACE_2 background with an ACCENT left edge; inactive tabs
/// lift on hover.
pub fn tab_strip_button(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, status| {
        let bg = if active {
            color::SURFACE_2
        } else {
            match status {
                button::Status::Hovered => color::SURFACE_1,
                button::Status::Pressed => color::SURFACE_2,
                _ => Color::TRANSPARENT,
            }
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: color::TEXT,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: theme::R_SM.into(),
            },
            ..Default::default()
        }
    }
}

/// Hairline horizontal divider used to separate adjacent sidebar sections
/// (e.g. tab strip → ref tree).
pub fn hairline_divider(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color::BORDER)),
        ..Default::default()
    }
}

/// Icon-only ghost button: transparent background, no border, subtle
/// surface tint on hover. Disabled state uses TEXT_SUBTLE icon color (set
/// by the caller); the style itself does not alter it.
pub fn ghost_icon_button(_: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => color::SURFACE_1,
        button::Status::Pressed => color::SURFACE_2,
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: color::TEXT,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: theme::R_SM.into(),
        },
        ..Default::default()
    }
}

pub fn danger_button(_: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => color::with_alpha(color::DANGER, 0.92),
        button::Status::Pressed => color::with_alpha(color::DANGER, 0.78),
        button::Status::Disabled => color::with_alpha(color::SURFACE_2, 0.72),
        _ => color::DANGER,
    };
    let (text_color, border_color, border_width) = if matches!(status, button::Status::Disabled) {
        (
            color::TEXT_MUTED,
            color::with_alpha(color::DANGER, 0.35),
            1.0,
        )
    } else {
        (Color::WHITE, Color::TRANSPARENT, 0.0)
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color,
        border: Border {
            color: border_color,
            width: border_width,
            radius: theme::R_MD.into(),
        },
        ..Default::default()
    }
}
