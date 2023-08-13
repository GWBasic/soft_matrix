use std::f32::consts::{PI, TAU};

use crate::structs::FrequencyPans;

pub trait Matrix {
    fn steer(
        &self,
        left_amplitude: f32,
        left_phase: f32,
        right_amplitude: f32,
        right_phase: f32,
    ) -> FrequencyPans;

    fn phase_shift(
        &self,
        left_front_phase: &mut f32,
        right_front_phase: &mut f32,
        left_rear_phase: &mut f32,
        right_rear_phase: &mut f32,
    );

    fn adjust_levels(
        &self,
        left_front: &mut f32,
        right_front: &mut f32,
        left_rear: &mut f32,
        right_rear: &mut f32,
        lfe: &mut Option<f32>,
        center: &mut Option<f32>,
    );
}

pub struct DefaultMatrix {
    widen_factor: f32,
    left_rear_shift: f32,
    right_rear_shift: f32,
    left_front_adjustment: f32,
    right_front_adjustment: f32,
    center_front_adjustment: f32,
    left_rear_adjustment: f32,
    right_rear_adjustment: f32,
    subwoofer_adjustment: f32,
}

// Note that it is intended that PhaseMatrix can be configured to support the old quad matrixes
impl DefaultMatrix {
    pub fn new() -> DefaultMatrix {
        DefaultMatrix {
            widen_factor: 1.0,
            left_rear_shift: -0.5 * PI,
            right_rear_shift: 0.5 * PI,
            left_front_adjustment: 1.0,
            right_front_adjustment: 1.0,
            center_front_adjustment: 1.0,
            left_rear_adjustment: 1.0,
            right_rear_adjustment: 1.0,
            subwoofer_adjustment: 1.0,
        }
    }

    pub fn qs() -> DefaultMatrix {
        let largest_sum = 0.924 + 0.383;
        let largest_pan = (0.924 / largest_sum) * 2.0 - 1.0;

        DefaultMatrix {
            widen_factor: 1.0 / largest_pan,
            left_rear_shift: -0.5 * PI,
            right_rear_shift: 0.5 * PI,
            left_front_adjustment: 1.0,
            right_front_adjustment: 1.0,
            center_front_adjustment: 1.0,
            left_rear_adjustment: 1.0,
            right_rear_adjustment: 1.0,
            subwoofer_adjustment: 1.0,
        }
    }

    pub fn horseshoe() -> DefaultMatrix {
        DefaultMatrix {
            widen_factor: 2.0,
            left_rear_shift: -0.5 * PI,
            right_rear_shift: 0.5 * PI,
            left_front_adjustment: 1.0,
            right_front_adjustment: 1.0,
            center_front_adjustment: 1.0,
            left_rear_adjustment: 1.0,
            right_rear_adjustment: 1.0,
            subwoofer_adjustment: 1.0,
        }
    }

    pub fn dolby_stereo_safe() -> DefaultMatrix {
        // Rust does not allow .sqrt() in constants
        let dolby_lower = 1.0 / 2.0_f32.sqrt();

        DefaultMatrix {
            widen_factor: 1.0,
            left_rear_shift: -0.5 * PI,
            right_rear_shift: 0.5 * PI,
            left_front_adjustment: dolby_lower,
            right_front_adjustment: dolby_lower,
            center_front_adjustment: 1.0,
            left_rear_adjustment: 1.0,
            right_rear_adjustment: 1.0,
            subwoofer_adjustment: dolby_lower,
        }
    }

    pub fn dolby_stereo_loud() -> DefaultMatrix {
        // Rust does not allow .sqrt() in constants
        let dolby_boost = 2.0_f32.sqrt();

        DefaultMatrix {
            widen_factor: 1.0,
            left_rear_shift: -0.5 * PI,
            right_rear_shift: 0.5 * PI,
            left_front_adjustment: 1.0,
            right_front_adjustment: 1.0,
            center_front_adjustment: dolby_boost,
            left_rear_adjustment: dolby_boost,
            right_rear_adjustment: dolby_boost,
            subwoofer_adjustment: 1.0,
        }
    }
}

impl Matrix for DefaultMatrix {
    fn steer(
        &self,
        left_amplitude: f32,
        left_phase: f32,
        right_amplitude: f32,
        right_phase: f32,
    ) -> FrequencyPans {
        // Will range from 0 to tau
        // 0 is in phase, pi is out of phase, tau is in phase (think circle)
        let phase_difference_tau = (left_phase - right_phase).abs();

        // 0 is in phase, pi is out of phase, tau is in phase (think half circle)
        let phase_difference_pi = if phase_difference_tau > PI {
            PI - (TAU - phase_difference_tau)
        } else {
            phase_difference_tau
        };

        // phase ratio: 0 is in phase, 1 is out of phase
        let back_to_front_from_phase = phase_difference_pi / PI;

        let amplitude_sum = left_amplitude + right_amplitude;
        let mut left_to_right = (left_amplitude / amplitude_sum) * 2.0 - 1.0;

        left_to_right *= self.widen_factor;

        let back_to_front_from_panning = (left_to_right.abs() - 1.0).max(0.0);

        left_to_right = left_to_right.min(1.0).max(-1.0);

        FrequencyPans {
            left_to_right,
            back_to_front: (back_to_front_from_panning + back_to_front_from_phase).min(1.0),
        }
    }

    fn phase_shift(
        &self,
        _left_front_phase: &mut f32,
        _right_front_phase: &mut f32,
        left_rear_phase: &mut f32,
        right_rear_phase: &mut f32,
    ) {
        shift(left_rear_phase, self.left_rear_shift);
        shift(right_rear_phase, self.right_rear_shift);
    }

    fn adjust_levels(
        &self,
        left_front: &mut f32,
        right_front: &mut f32,
        left_rear: &mut f32,
        right_rear: &mut f32,
        lfe: &mut Option<f32>,
        center: &mut Option<f32>,
    ) {
        *left_front *= self.left_front_adjustment;
        *right_front *= self.right_front_adjustment;
        *left_rear *= self.left_rear_adjustment;
        *right_rear *= self.right_rear_adjustment;

        match *center {
            Some(center_front_value) => {
                *center = Some(center_front_value * self.center_front_adjustment)
            }
            None => {}
        }

        match *lfe {
            Some(subwoofer_value) => *lfe = Some(subwoofer_value * self.subwoofer_adjustment),
            None => {}
        }
    }
}

fn shift(phase: &mut f32, shift: f32) {
    *phase += shift;

    if *phase > PI {
        *phase -= TAU;
    } else if *phase < -PI {
        *phase += TAU;
    }
}
