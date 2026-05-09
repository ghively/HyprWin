use super::rect::Rect;

// ═══════════════════════════════════════════════════════════════════════════════
// AI_AGENT_STOP: DPI_UTILITIES — Pure DPI scaling math.
// Before modifying DPI behavior:
//   1. BASE_DPI = 96 — all scaling is relative to this.
//   2. logical_to_physical() multiplies; physical_to_logical() divides.
//   3. This module is platform-agnostic. Win32 DPI queries live in
//      `platform::dpi` (`get_monitor_dpi`, `get_system_dpi`).
//   4. scale_rect_to_physical() is called in apply_layout() before SetWindowPos.
// ═══════════════════════════════════════════════════════════════════════════════

/// The baseline DPI value (96 DPI) used as the reference for scaling calculations.
pub const BASE_DPI: u32 = 96;

/// Convert logical pixels to physical pixels for a given monitor DPI.
///
/// Logical pixels are device-independent; physical pixels are actual screen pixels.
/// The conversion uses the formula: `logical * dpi / BASE_DPI`.
pub fn logical_to_physical(logical: i32, dpi: u32) -> i32 {
    if dpi == 0 || dpi == BASE_DPI {
        return logical;
    }
    ((logical as i64) * (dpi as i64) / (BASE_DPI as i64)) as i32
}

/// Convert physical pixels to logical pixels for a given monitor DPI.
///
/// The conversion uses the formula: `physical * BASE_DPI / dpi`.
pub fn physical_to_logical(physical: i32, dpi: u32) -> i32 {
    if dpi == 0 || dpi == BASE_DPI {
        return physical;
    }
    ((physical as i64) * (BASE_DPI as i64) / (dpi as i64)) as i32
}

/// Scale a rectangle from logical to physical coordinates for a monitor.
///
/// Each dimension (position and size) is scaled independently using the DPI.
pub fn scale_rect_to_physical(rect: &Rect, dpi: u32) -> Rect {
    if dpi == 0 || dpi == BASE_DPI {
        return *rect;
    }
    Rect {
        x: logical_to_physical(rect.x, dpi),
        y: logical_to_physical(rect.y, dpi),
        width: logical_to_physical(rect.width, dpi),
        height: logical_to_physical(rect.height, dpi),
    }
}

/// Scale a rectangle from physical to logical coordinates for a monitor.
///
/// Each dimension (position and size) is scaled independently using the DPI.
pub fn scale_rect_to_logical(rect: &Rect, dpi: u32) -> Rect {
    if dpi == 0 || dpi == BASE_DPI {
        return *rect;
    }
    Rect {
        x: physical_to_logical(rect.x, dpi),
        y: physical_to_logical(rect.y, dpi),
        width: physical_to_logical(rect.width, dpi),
        height: physical_to_logical(rect.height, dpi),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logical_to_physical_at_base_dpi() {
        assert_eq!(logical_to_physical(100, 96), 100);
        assert_eq!(logical_to_physical(100, 0), 100);
    }

    #[test]
    fn test_logical_to_physical_at_144_dpi() {
        assert_eq!(logical_to_physical(100, 144), 150);
    }

    #[test]
    fn test_physical_to_logical_at_144_dpi() {
        assert_eq!(physical_to_logical(150, 144), 100);
    }

    #[test]
    fn test_round_trip() {
        let original = 133;
        let dpi = 192;
        let physical = logical_to_physical(original, dpi);
        let logical = physical_to_logical(physical, dpi);
        assert!((logical - original).abs() <= 1);
    }

    #[test]
    fn test_scale_rect_to_physical() {
        let rect = Rect::new(10, 20, 100, 200);
        let scaled = scale_rect_to_physical(&rect, 192);
        assert_eq!(scaled.x, 20);
        assert_eq!(scaled.y, 40);
        assert_eq!(scaled.width, 200);
        assert_eq!(scaled.height, 400);
    }

    #[test]
    fn test_scale_rect_to_logical() {
        let rect = Rect::new(20, 40, 200, 400);
        let scaled = scale_rect_to_logical(&rect, 192);
        assert_eq!(scaled.x, 10);
        assert_eq!(scaled.y, 20);
        assert_eq!(scaled.width, 100);
        assert_eq!(scaled.height, 200);
    }

    #[test]
    fn test_base_dpi_no_op() {
        let rect = Rect::new(10, 20, 100, 200);
        let physical = scale_rect_to_physical(&rect, BASE_DPI);
        let logical = scale_rect_to_logical(&rect, BASE_DPI);
        assert_eq!(physical, rect);
        assert_eq!(logical, rect);
    }
}
