use super::rect::Rect;

// ═══════════════════════════════════════════════════════════════════════════════
// AI_AGENT_STOP: ANIMATION_FRAMEWORK — Easing and interpolation.
// Before adding new animations:
//   1. Easing::apply(t) maps [0,1] → [0,1] with curve shape.
//   2. Animation::tick(delta_ms) advances elapsed time, returns progress.
//   3. interpolate_rect() computes intermediate positions for window moves.
//   4. Currently animations are applied directly; timer-based loop is a future enhancement.
//   5. Keep animation duration < 200ms to feel responsive.
// ═══════════════════════════════════════════════════════════════════════════════

/// Easing function for animation interpolation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    /// Linear interpolation — constant velocity throughout.
    Linear,
    /// Ease-out cubic — fast start, smooth deceleration.
    /// Formula: `1 - (1 - t)^3`
    EaseOutCubic,
    /// Ease-out exponential — very fast start, gentle landing.
    /// Formula: `t == 1.0 ? 1.0 : 1 - 2^(-10t)`
    EaseOutExpo,
}

impl Easing {
    /// Apply the easing function to a progress value `t` in the range [0, 1].
    ///
    /// Returns the eased value, also in the range [0, 1].
    pub fn apply(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseOutCubic => {
                let t1 = 1.0 - t;
                1.0 - t1 * t1 * t1
            }
            Easing::EaseOutExpo => {
                if t >= 1.0 {
                    1.0
                } else {
                    1.0 - 2f64.powf(-10.0 * t)
                }
            }
        }
    }
}

/// An animation with a configurable duration and easing function.
///
/// Call `tick` with the elapsed milliseconds since the last frame to
/// advance the animation. The returned value is the eased progress
/// in the range [0, 1].
#[derive(Debug, Clone)]
pub struct Animation {
    /// Total animation duration in milliseconds.
    pub duration_ms: u32,
    /// Current elapsed time in milliseconds.
    pub elapsed_ms: u32,
    /// The easing function applied to the raw progress.
    pub easing: Easing,
}

impl Animation {
    /// Create a new animation with the given duration and easing.
    ///
    /// # Panics
    ///
    /// Panics if `duration_ms` is zero.
    pub fn new(duration_ms: u32, easing: Easing) -> Self {
        assert!(
            duration_ms > 0,
            "animation duration must be greater than zero"
        );
        Self {
            duration_ms,
            elapsed_ms: 0,
            easing,
        }
    }

    /// Advance the animation by `delta_ms` milliseconds.
    ///
    /// Returns the eased progress in the range [0, 1], where 1.0 means
    /// the animation is complete.
    pub fn tick(&mut self, delta_ms: u32) -> f64 {
        self.elapsed_ms = (self.elapsed_ms + delta_ms).min(self.duration_ms);
        let progress = self.elapsed_ms as f64 / self.duration_ms as f64;
        self.easing.apply(progress)
    }

    /// Check if the animation has reached its full duration.
    pub fn is_complete(&self) -> bool {
        self.elapsed_ms >= self.duration_ms
    }

    /// Reset the animation to the beginning.
    pub fn reset(&mut self) {
        self.elapsed_ms = 0;
    }
}

/// Linearly interpolate between two `f64` values.
///
/// `t` should be in the range [0, 1]. Values outside this range are
/// clamped.
pub fn lerp(start: f64, end: f64, t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    start + (end - start) * t
}

/// Interpolate between two rectangles.
///
/// Each field (x, y, width, height) is independently interpolated.
/// `progress` should be in the range [0, 1].
pub fn interpolate_rect(from: &Rect, to: &Rect, progress: f64) -> Rect {
    let progress = progress.clamp(0.0, 1.0);
    Rect {
        x: lerp(from.x as f64, to.x as f64, progress).round() as i32,
        y: lerp(from.y as f64, to.y as f64, progress).round() as i32,
        width: lerp(from.width as f64, to.width as f64, progress).round() as i32,
        height: lerp(from.height as f64, to.height as f64, progress).round() as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easing_linear() {
        assert_eq!(Easing::Linear.apply(0.0), 0.0);
        assert!((Easing::Linear.apply(0.5) - 0.5).abs() < f64::EPSILON);
        assert_eq!(Easing::Linear.apply(1.0), 1.0);
    }

    #[test]
    fn test_easing_ease_out_cubic() {
        assert_eq!(Easing::EaseOutCubic.apply(0.0), 0.0);
        assert_eq!(Easing::EaseOutCubic.apply(1.0), 1.0);
        // At t=0.5: 1 - (0.5)^3 = 1 - 0.125 = 0.875
        assert!((Easing::EaseOutCubic.apply(0.5) - 0.875).abs() < 0.001);
    }

    #[test]
    fn test_easing_ease_out_expo() {
        assert_eq!(Easing::EaseOutExpo.apply(0.0), 0.0);
        assert_eq!(Easing::EaseOutExpo.apply(1.0), 1.0);
        // At t=0.5: 1 - 2^(-5) = 1 - 0.03125 = 0.96875
        assert!((Easing::EaseOutExpo.apply(0.5) - 0.96875).abs() < 0.001);
    }

    #[test]
    fn test_easing_clamps() {
        assert_eq!(Easing::Linear.apply(-0.5), 0.0);
        assert_eq!(Easing::Linear.apply(1.5), 1.0);
    }

    #[test]
    fn test_animation_tick() {
        let mut anim = Animation::new(100, Easing::Linear);
        assert_eq!(anim.tick(50), 0.5);
        assert!(!anim.is_complete());
        assert_eq!(anim.tick(50), 1.0);
        assert!(anim.is_complete());
    }

    #[test]
    fn test_animation_reset() {
        let mut anim = Animation::new(100, Easing::Linear);
        anim.tick(100);
        assert!(anim.is_complete());
        anim.reset();
        assert!(!anim.is_complete());
        assert_eq!(anim.elapsed_ms, 0);
    }

    #[test]
    fn test_lerp() {
        assert!((lerp(0.0, 100.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((lerp(0.0, 100.0, 0.5) - 50.0).abs() < f64::EPSILON);
        assert!((lerp(0.0, 100.0, 1.0) - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_interpolate_rect() {
        let from = Rect::new(0, 0, 100, 100);
        let to = Rect::new(100, 200, 300, 400);
        let mid = interpolate_rect(&from, &to, 0.5);
        assert_eq!(mid.x, 50);
        assert_eq!(mid.y, 100);
        assert_eq!(mid.width, 200);
        assert_eq!(mid.height, 250);
    }

    #[test]
    fn test_interpolate_rect_clamped() {
        let from = Rect::new(0, 0, 100, 100);
        let to = Rect::new(100, 200, 300, 400);
        let beyond = interpolate_rect(&from, &to, 2.0);
        assert_eq!(beyond, to);
        let before = interpolate_rect(&from, &to, -1.0);
        assert_eq!(before, from);
    }
}
