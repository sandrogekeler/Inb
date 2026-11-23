//! The settled frame design, in logical points at scale 1. Design canvas:
//! https://claude.ai/artifact/PsJpNtGnKZMB2LnFxRz8kq (Graphite palette).

use std::time::Duration;

/// A straight (not premultiplied) sRGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

/// Bar and frame background while idle.
pub const BG: Rgba = Rgba(0x1c, 0x1c, 0x1e, 0xff);
/// Text and icons.
pub const FG: Rgba = Rgba(0xf5, 0xf5, 0xf7, 0xff);
/// Secondary text: the size label, percentages, the close cross.
pub const MUTED: Rgba = Rgba(0xa1, 0xa1, 0xa6, 0xff);
/// Fill of the fps chip, the Cancel button and the record dot's ring.
pub const CHIP: Rgba = Rgba(0xff, 0xff, 0xff, 26);
/// One-point outline around the frame band, the bar and the menu.
pub const EDGE: Rgba = Rgba(0, 0, 0, 140);
/// The record dot.
pub const DOT: Rgba = Rgba(0xff, 0x45, 0x3a, 0xff);
/// The frame, the growing circle and the Quit button while live.
pub const REC_BG: Rgba = Rgba(0xe5, 0x34, 0x2b, 0xff);
/// Text on `REC_BG`.
pub const REC_FG: Rgba = Rgba(0xff, 0xff, 0xff, 0xff);
/// Opacity of the four overlays that dim the screen outside the area.
pub const DIM_ALPHA: f32 = 0.15;

/// Width of the coloured band around the area.
pub const BAND: f64 = 2.0;
/// Corner radius of the band, the bar, buttons and the menu.
pub const RADIUS: f64 = 3.0;
/// How far each edge window reaches outwards from the area. Only the inner
/// `BAND` is drawn; the rest is near-transparent so it still catches the mouse.
pub const HIT: f64 = 10.0;
/// A press this close to a frame corner resizes instead of moving.
pub const CORNER_ZONE: f64 = 14.0;
/// The square handle drawn at each outer corner while idle.
pub const HANDLE: f64 = 6.0;
/// Smallest area a resize allows.
pub const MIN_W: f64 = 160.0;
pub const MIN_H: f64 = 100.0;
/// The area Inb opens with, centred on the display.
pub const START_W: f64 = 640.0;
pub const START_H: f64 = 400.0;

/// Space between the frame and the bar.
pub const GAP: f64 = 6.0;
/// The bar keeps one width in every state.
pub const BAR_W: f64 = 224.0;
pub const BAR_H: f64 = 36.0;
pub const BAR_PAD_LEFT: f64 = 10.0;
pub const BAR_PAD_RIGHT: f64 = 4.0;
pub const BAR_SPACING: f64 = 6.0;
/// Height of every button, and width of the icon buttons.
pub const BUTTON: f64 = 28.0;
pub const FONT_SIZE: f32 = 13.0;
/// Keep the bar this far from the bottom of the display before it flips above.
pub const SCREEN_MARGIN: f64 = 4.0;

pub const MENU_PAD: f64 = 4.0;
pub const MENU_ITEM_H: f64 = 28.0;
pub const MENU_ITEM_W: f64 = 76.0;

pub const FPS_OPTIONS: [u32; 6] = [10, 15, 20, 25, 30, 50];
pub const DEFAULT_FPS: u32 = 15;

/// The record dot is wiped clockwise for this long; a second press cancels.
pub const ARM: Duration = Duration::from_millis(1000);
/// Then a red circle grows from the record button until it fills the bar.
pub const GROW: Duration = Duration::from_millis(600);
/// The frame fades to red as the circle grows, and back after Stop.
pub const FADE: Duration = Duration::from_millis(350);
/// After Stop the circle shrinks back into the bar.
pub const SHRINK: Duration = Duration::from_millis(450);

/// Fill of the hovered fps menu row: white at 10%.
pub const HOVER: Rgba = Rgba(0xff, 0xff, 0xff, 26);
/// Extra corner radius of the fps menu over `RADIUS`.
pub const MENU_EXTRA_RADIUS: f64 = 2.0;
/// Space between a menu row's edge and its text or check mark.
pub const MENU_TEXT_PAD: f64 = 10.0;
/// Width of the outline around the bar and the menu.
pub const OUTLINE: f64 = 1.0;

/// The record dot and the ring around it.
pub const DOT_SIZE: f64 = 16.0;
pub const DOT_RING: f64 = 2.0;
/// The small dot before the time while recording.
pub const REC_DOT_SIZE: f64 = 8.0;
/// Size and stroke of the close cross.
pub const CROSS_SIZE: f64 = 12.0;
pub const CROSS_STROKE: f64 = 1.6;
/// The fps chip's chevron: distance from the chip's right edge, width, height.
pub const CHEVRON_INSET: f64 = 7.0;
pub const CHEVRON_W: f64 = 7.0;
pub const CHEVRON_H: f64 = 4.0;
/// Left padding of the chip's text.
pub const CHIP_TEXT_PAD: f64 = 9.0;
/// The check mark in the Done state and the fps menu.
pub const CHECK_W: f64 = 10.0;
pub const CHECK_H: f64 = 8.0;
/// Space between an icon and the text after it.
pub const ICON_GAP: f64 = 6.0;
/// The square in the Stop button.
pub const STOP_ICON: f64 = 9.0;
/// The progress line along the bar's bottom edge while encoding.
pub const PROGRESS_H: f64 = 2.0;
/// Extra horizontal ink that fakes a bold weight, in points.
pub const BOLD_SMEAR: f64 = 0.4;
