//! Win32 `RECT` ↔ [`Rect`] conversions.
//!
//! [`crate::util::rect`] is the platform-agnostic geometry module and must not
//! depend on `windows-rs`. The conversions live here so platform-layer callers
//! can move data across the Win32 boundary without leaking `RECT` into pure
//! code.

use windows::Win32::Foundation::RECT;

use crate::util::rect::Rect;

/// Convert from a Win32 `RECT` to a [`Rect`].
///
/// The Win32 `RECT` uses left/top/right/bottom, so width is `right - left`
/// and height is `bottom - top`.
pub fn rect_from_win32(rect: &RECT) -> Rect {
    Rect {
        x: rect.left,
        y: rect.top,
        width: rect.right - rect.left,
        height: rect.bottom - rect.top,
    }
}

/// Convert a [`Rect`] to a Win32 `RECT`.
pub fn rect_to_win32(rect: &Rect) -> RECT {
    RECT {
        left: rect.x,
        top: rect.y,
        right: rect.x + rect.width,
        bottom: rect.y + rect.height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_from_win32() {
        let rect = RECT {
            left: 10,
            top: 20,
            right: 110,
            bottom: 220,
        };
        let r = rect_from_win32(&rect);
        assert_eq!(r.x, 10);
        assert_eq!(r.y, 20);
        assert_eq!(r.width, 100);
        assert_eq!(r.height, 200);
    }

    #[test]
    fn test_rect_to_win32() {
        let r = Rect::new(10, 20, 100, 200);
        let rect = rect_to_win32(&r);
        assert_eq!(rect.left, 10);
        assert_eq!(rect.top, 20);
        assert_eq!(rect.right, 110);
        assert_eq!(rect.bottom, 220);
    }

    #[test]
    fn test_round_trip() {
        let r = Rect::new(5, 7, 50, 70);
        let round = rect_from_win32(&rect_to_win32(&r));
        assert_eq!(r, round);
    }
}
