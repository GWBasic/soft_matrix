use std::f32::consts::{PI, TAU};

use crate::structs::FrequencyPans;

const NPI: f32 = PI * -1.0;
const HALF_PI: f32 = PI / 2.0;
const HALF_NPI: f32 = NPI / 2.0;

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

// Note that it is intended that DefaultMatrix can be configured to support the old quad matrixes
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

pub struct SQMatrix {
    left_front_adjustment: f32,
    right_front_adjustment: f32,
    center_front_adjustment: f32,
    left_rear_adjustment: f32,
    right_rear_adjustment: f32,
    subwoofer_adjustment: f32,
}

const SQ_LOWER: f32 = 0.7;
const SQ_RAISE: f32 = 1.0 / 0.7;
const SQ_LEFT_REAR_SHIFT: f32 = PI / 2.0;
const SQ_RIGHT_REAR_SHIFT: f32 = SQ_LEFT_REAR_SHIFT * -1.0;

impl SQMatrix {
    pub fn sq_safe() -> SQMatrix {
        SQMatrix {
            left_front_adjustment: SQ_LOWER,
            right_front_adjustment: SQ_LOWER,
            center_front_adjustment: SQ_LOWER,
            left_rear_adjustment: 1.0,
            right_rear_adjustment: 1.0,
            subwoofer_adjustment: SQ_LOWER,
        }
    }

    pub fn sq_loud() -> SQMatrix {
        SQMatrix {
            left_front_adjustment: 1.0,
            right_front_adjustment: 1.0,
            center_front_adjustment: 1.0,
            left_rear_adjustment: SQ_RAISE,
            right_rear_adjustment: SQ_RAISE,
            subwoofer_adjustment: 1.0,
        }
    }
}

impl Matrix for SQMatrix {
    fn steer(
        &self,
        left_amplitude: f32,
        left_phase: f32,
        right_amplitude: f32,
        right_phase: f32,
    ) -> FrequencyPans {

/*

right front to right rear
right_rear_amplitude: 0, right_front_amplitude: 1; phase_difference: 0, amplitude: 1
right_rear_amplitude: 0.1, right_front_amplitude: 0.9; phase_difference: -0.07762151, amplitude: 0.97271806
right_rear_amplitude: 0.2, right_front_amplitude: 0.8; phase_difference: -0.17324567, amplitude: 0.9521576
right_rear_amplitude: 0.3, right_front_amplitude: 0.7; phase_difference: -0.29145682, amplitude: 0.9408214
right_rear_amplitude: 0.4, right_front_amplitude: 0.6; phase_difference: -0.43662715, amplitude: 0.9421178
right_rear_amplitude: 0.5, right_front_amplitude: 0.5; phase_difference: -0.610726, amplitude: 0.96032774
right_rear_amplitude: 0.6, right_front_amplitude: 0.4; phase_difference: -0.80978364, amplitude: 1
right_rear_amplitude: 0.7, right_front_amplitude: 0.3; phase_difference: -1.0214219, amplitude: 1.0645432
right_rear_amplitude: 0.8, right_front_amplitude: 0.2; phase_difference: -1.2277725, amplitude: 1.1546428
right_rear_amplitude: 0.9, right_front_amplitude: 0.1; phase_difference: -1.4133795, amplitude: 1.2678871
right_rear_amplitude: 1, right_front_amplitude: 0; phase_difference: -1.5707964, amplitude: 1.4

right rear to left rear
left_amplitude: 0, right_aplitude: 1; phase_difference: -1.5707964, amplitude: 1.4
left_amplitude: 0.1, right_aplitude: 0.9; phase_difference: -1.7921108, amplitude: 1.267754
left_amplitude: 0.2, right_aplitude: 0.8; phase_difference: -2.0607538, amplitude: 1.1544696
left_amplitude: 0.3, right_aplitude: 0.7; phase_difference: -2.38058, amplitude: 1.0662081
left_amplitude: 0.4, right_aplitude: 0.6; phase_difference: -2.7468014, amplitude: 1.0095544
left_amplitude: 0.5, right_aplitude: 0.5; phase_difference: -3.1415927, amplitude: 0.98994946
left_amplitude: 0.6, right_aplitude: 0.4; phase_difference: 2.7468016, amplitude: 1.0095544
left_amplitude: 0.7, right_aplitude: 0.3; phase_difference: 2.38058, amplitude: 1.0662081
left_amplitude: 0.8, right_aplitude: 0.2; phase_difference: 2.0607538, amplitude: 1.1544696
left_amplitude: 0.9, right_aplitude: 0.1; phase_difference: 1.7921109, amplitude: 1.2677538
left_amplitude: 1, right_aplitude: 0; phase_difference: 1.5707961, amplitude: 1.4

left front to left rear
left_rear_amplitude: 1, left_front_amplitude: 0; phase_difference: 1.5707961, amplitude: 1.4
left_rear_amplitude: 0.9, left_front_amplitude: 0.1; phase_difference: 1.728213, amplitude: 1.2678871
left_rear_amplitude: 0.8, left_front_amplitude: 0.2; phase_difference: 1.91382, amplitude: 1.1546428
left_rear_amplitude: 0.7, left_front_amplitude: 0.3; phase_difference: 2.1201706, amplitude: 1.0645432
left_rear_amplitude: 0.6, left_front_amplitude: 0.4; phase_difference: 2.3318088, amplitude: 1
left_rear_amplitude: 0.5, left_front_amplitude: 0.5; phase_difference: 2.5308666, amplitude: 0.96032774
left_rear_amplitude: 0.4, left_front_amplitude: 0.6; phase_difference: 2.7049654, amplitude: 0.9421178
left_rear_amplitude: 0.3, left_front_amplitude: 0.7; phase_difference: 2.8501358, amplitude: 0.9408214
left_rear_amplitude: 0.2, left_front_amplitude: 0.8; phase_difference: 2.9683468, amplitude: 0.9521576
left_rear_amplitude: 0.1, left_front_amplitude: 0.9; phase_difference: 3.063971, amplitude: 0.97271806
left_rear_amplitude: 0, left_front_amplitude: 1; phase_difference: 3.1415925, amplitude: 1

*/

        let amplitude_sum = left_amplitude + right_amplitude;
        let left_to_right = (left_amplitude / amplitude_sum) * 2.0 - 1.0;

        // Will range from -pi to pi
        // - 0-pi is front
        // - 1.5707961 is left rear
        // - -pi is rear center
        // - -1.5707961 is right rear
        // - 0-pi is front
        let phase_difference = left_phase - right_phase;

        if phase_difference == 0.0 && phase_difference == PI {
            // Tone is 100% in the front
            return FrequencyPans {
                left_to_right,
                back_to_front: 0.0
            };
        } else if phase_difference <= HALF_NPI {
            // Tone is 100% in the rear, steered to the right by phase
            /*
left_amplitude: 0, right_aplitude: 1; phase_difference: -1.5707964, amplitude: 1.4
left_amplitude: 0.1, right_aplitude: 0.9; phase_difference: -1.7921108, amplitude: 1.267754
left_amplitude: 0.2, right_aplitude: 0.8; phase_difference: -2.0607538, amplitude: 1.1544696
left_amplitude: 0.3, right_aplitude: 0.7; phase_difference: -2.38058, amplitude: 1.0662081
left_amplitude: 0.4, right_aplitude: 0.6; phase_difference: -2.7468014, amplitude: 1.0095544
left_amplitude: 0.5, right_aplitude: 0.5; phase_difference: -3.1415927, amplitude: 0.98994946
            */

            // -1.5707964 -> 1
            // -PI -> 0

            let positive_phase_difference = phase_difference * -1.0;
            let ppd_zeroed = positive_phase_difference - HALF_PI;

            return FrequencyPans {
                // Right to left panning: -1 is left, 1 is right
                left_to_right: (HALF_PI - ppd_zeroed) / HALF_PI,
                back_to_front: 1.0
            };
        } else if phase_difference >= HALF_PI {
            // Tone is 100% in the rear, steered to the left by phase
            /*
left_amplitude: 0.6, right_aplitude: 0.4; phase_difference: 2.7468016, amplitude: 1.0095544
left_amplitude: 0.7, right_aplitude: 0.3; phase_difference: 2.38058, amplitude: 1.0662081
left_amplitude: 0.8, right_aplitude: 0.2; phase_difference: 2.0607538, amplitude: 1.1544696
left_amplitude: 0.9, right_aplitude: 0.1; phase_difference: 1.7921109, amplitude: 1.2677538
left_amplitude: 1, right_aplitude: 0; phase_difference: 1.5707961, amplitude: 1.4
            */

            // 1.5707964 -> -1
            // PI -> 0

            let phase_difference_zeroed = phase_difference - HALF_PI;
            let right_to_left = (HALF_PI - phase_difference_zeroed) / HALF_PI;

            return FrequencyPans {
                // Right to left panning: -1 is left, 1 is right
                left_to_right: right_to_left * -1.0,
                back_to_front: 1.0
            };
        }

        panic!("Incomplete");

        /*
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

        let back_to_front_from_panning = (left_to_right.abs() - 1.0).max(0.0);

        left_to_right = left_to_right.min(1.0).max(-1.0);

        FrequencyPans {
            left_to_right,
            back_to_front: (back_to_front_from_panning + back_to_front_from_phase).min(1.0),
        }
        */
    }

    fn phase_shift(
        &self,
        _left_front_phase: &mut f32,
        _right_front_phase: &mut f32,
        left_rear_phase: &mut f32,
        right_rear_phase: &mut f32,
    ) {
        shift(left_rear_phase, SQ_LEFT_REAR_SHIFT);
        shift(right_rear_phase, SQ_RIGHT_REAR_SHIFT);
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
