use geom::{Angle, Speed};
use widgetry::{EventCtx, Key};

// TODO The timestep accumulation seems fine. What's wrong? Clamping errors repeated?
const HACK: f64 = 5.0;

pub struct InstantController {
    /// Which of the 8 directions are we facing, based on the last set of keys pressed down?
    pub facing: Angle,
}

impl InstantController {
    pub fn new() -> InstantController {
        InstantController {
            facing: Angle::ZERO,
        }
    }

    pub fn displacement(&mut self, ctx: &mut EventCtx, speed: Speed) -> Option<(f64, f64)> {
        let dt = ctx.input.nonblocking_is_update_event()?;

        // Work around a few bugs here.
        //
        // 1) The Santa sprites are all facing 180 degrees, not 0, so invert X.
        // 2) Invert y so that negative is up.
        //
        // It's confusing, but self.facing winds up working for rotating the sprite, and the output
        // displacement works.

        let mut x: f64 = 0.0;
        let mut y: f64 = 0.0;
        if ctx.is_key_down(Key::LeftArrow) {
            x += 1.0;
        }
        if ctx.is_key_down(Key::RightArrow) {
            x -= 1.0;
        }
        if ctx.is_key_down(Key::UpArrow) {
            y += 1.0;
        }
        if ctx.is_key_down(Key::DownArrow) {
            y -= 1.0;
        }

        if x == 0.0 && y == 0.0 {
            return None;
        }

        self.facing = Angle::new_rads(y.atan2(x));
        let magnitude = (dt * HACK * speed).inner_meters();
        let (sin, cos) = self.facing.normalized_radians().sin_cos();
        Some((-magnitude * cos, -magnitude * sin))
    }
}
