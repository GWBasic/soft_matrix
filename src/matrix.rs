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
left_front: 0; right_front: 1; left_rear: 0; right_rear: 0.1; phase_difference: -0.069886, amplitude: 1.0724471, left_total_amplitude: 0.07, right_total_amplitude: 1.002447
left_front: 0; right_front: 1; left_rear: 0; right_rear: 0.2; phase_difference: -0.13909595, amplitude: 1.1497524, left_total_amplitude: 0.14, right_total_amplitude: 1.0097524
left_front: 0; right_front: 1; left_rear: 0; right_rear: 0.3; phase_difference: -0.20699221, amplitude: 1.2318121, left_total_amplitude: 0.21000001, right_total_amplitude: 1.0218121
left_front: 0; right_front: 1; left_rear: 0; right_rear: 0.4; phase_difference: -0.2730087, amplitude: 1.3184603, left_total_amplitude: 0.28, right_total_amplitude: 1.0384604
left_front: 0; right_front: 1; left_rear: 0; right_rear: 0.5; phase_difference: -0.3366748, amplitude: 1.409481, left_total_amplitude: 0.35, right_total_amplitude: 1.059481
left_front: 0; right_front: 1; left_rear: 0; right_rear: 0.6; phase_difference: -0.397628, amplitude: 1.5046198, left_total_amplitude: 0.42000002, right_total_amplitude: 1.0846198
left_front: 0; right_front: 1; left_rear: 0; right_rear: 0.7; phase_difference: -0.45561564, amplitude: 1.6035978, left_total_amplitude: 0.48999998, right_total_amplitude: 1.1135978
left_front: 0; right_front: 1; left_rear: 0; right_rear: 0.8; phase_difference: -0.51048833, amplitude: 1.7061238, left_total_amplitude: 0.56, right_total_amplitude: 1.1461239
left_front: 0; right_front: 1; left_rear: 0; right_rear: 0.9; phase_difference: -0.5621867, amplitude: 1.8119053, left_total_amplitude: 0.63, right_total_amplitude: 1.1819053
left_front: 0; right_front: 1; left_rear: 0; right_rear: 1; phase_difference: -0.610726, amplitude: 1.9206555, left_total_amplitude: 0.7, right_total_amplitude: 1.2206556
left_front: 0; right_front: 0.9; left_rear: 0; right_rear: 1; phase_difference: -0.6610432, amplitude: 1.8401754, left_total_amplitude: 0.7, right_total_amplitude: 1.1401753
left_front: 0; right_front: 0.8; left_rear: 0; right_rear: 1; phase_difference: -0.71883, amplitude: 1.7630146, left_total_amplitude: 0.7, right_total_amplitude: 1.0630145
left_front: 0; right_front: 0.7; left_rear: 0; right_rear: 1; phase_difference: -0.7853982, amplitude: 1.6899494, left_total_amplitude: 0.7, right_total_amplitude: 0.9899494
left_front: 0; right_front: 0.6; left_rear: 0; right_rear: 1; phase_difference: -0.8621701, amplitude: 1.6219544, left_total_amplitude: 0.7, right_total_amplitude: 0.9219544
left_front: 0; right_front: 0.5; left_rear: 0; right_rear: 1; phase_difference: -0.95054686, amplitude: 1.5602324, left_total_amplitude: 0.7, right_total_amplitude: 0.8602325
left_front: 0; right_front: 0.4; left_rear: 0; right_rear: 1; phase_difference: -1.0516503, amplitude: 1.5062258, left_total_amplitude: 0.7, right_total_amplitude: 0.8062258
left_front: 0; right_front: 0.3; left_rear: 0; right_rear: 1; phase_difference: -1.1659045, amplitude: 1.4615773, left_total_amplitude: 0.7, right_total_amplitude: 0.7615773
left_front: 0; right_front: 0.2; left_rear: 0; right_rear: 1; phase_difference: -1.2924967, amplitude: 1.428011, left_total_amplitude: 0.7, right_total_amplitude: 0.72801095
left_front: 0; right_front: 0.1; left_rear: 0; right_rear: 1; phase_difference: -1.4288993, amplitude: 1.4071068, left_total_amplitude: 0.7, right_total_amplitude: 0.70710677

right rear to left rear
left_front: 0; right_front: 0; left_rear: 0.1; right_rear: 1; phase_difference: -1.7701336, amplitude: 1.4069825, left_total_amplitude: 0.7034913, right_total_amplitude: 0.7034913
left_front: 0; right_front: 0; left_rear: 0.2; right_rear: 1; phase_difference: -1.9655875, amplitude: 1.4277254, left_total_amplitude: 0.7138627, right_total_amplitude: 0.7138627
left_front: 0; right_front: 0; left_rear: 0.3; right_rear: 1; phase_difference: -2.15371, amplitude: 1.4616429, left_total_amplitude: 0.73082143, right_total_amplitude: 0.73082143
left_front: 0; right_front: 0; left_rear: 0.4; right_rear: 1; phase_difference: -2.331809, amplitude: 1.5078461, left_total_amplitude: 0.75392306, right_total_amplitude: 0.75392306
left_front: 0; right_front: 0; left_rear: 0.5; right_rear: 1; phase_difference: -2.4980917, amplitude: 1.5652475, left_total_amplitude: 0.78262377, right_total_amplitude: 0.78262377
left_front: 0; right_front: 0; left_rear: 0.6; right_rear: 1; phase_difference: -2.6516354, amplitude: 1.6326665, left_total_amplitude: 0.81633323, right_total_amplitude: 0.81633323
left_front: 0; right_front: 0; left_rear: 0.7; right_rear: 1; phase_difference: -2.7922482, amplitude: 1.7089177, left_total_amplitude: 0.85445887, right_total_amplitude: 0.85445887
left_front: 0; right_front: 0; left_rear: 0.8; right_rear: 1; phase_difference: -2.9202783, amplitude: 1.7928748, left_total_amplitude: 0.8964374, right_total_amplitude: 0.89643735
left_front: 0; right_front: 0; left_rear: 0.9; right_rear: 1; phase_difference: -3.0364265, amplitude: 1.8835074, left_total_amplitude: 0.9417537, right_total_amplitude: 0.9417537
left_front: 0; right_front: 0; left_rear: 1; right_rear: 1; phase_difference: -3.1415927, amplitude: 1.9798989, left_total_amplitude: 0.9899494, right_total_amplitude: 0.98994946
left_front: 0; right_front: 0; left_rear: 1; right_rear: 0.9; phase_difference: 3.0364265, amplitude: 1.8835073, left_total_amplitude: 0.9417536, right_total_amplitude: 0.9417536
left_front: 0; right_front: 0; left_rear: 1; right_rear: 0.8; phase_difference: 2.9202783, amplitude: 1.7928747, left_total_amplitude: 0.89643735, right_total_amplitude: 0.89643735
left_front: 0; right_front: 0; left_rear: 1; right_rear: 0.7; phase_difference: 2.7922482, amplitude: 1.7089176, left_total_amplitude: 0.85445887, right_total_amplitude: 0.8544588
left_front: 0; right_front: 0; left_rear: 1; right_rear: 0.6; phase_difference: 2.6516356, amplitude: 1.6326665, left_total_amplitude: 0.81633323, right_total_amplitude: 0.81633323
left_front: 0; right_front: 0; left_rear: 1; right_rear: 0.5; phase_difference: 2.4980917, amplitude: 1.5652475, left_total_amplitude: 0.78262377, right_total_amplitude: 0.78262377
left_front: 0; right_front: 0; left_rear: 1; right_rear: 0.4; phase_difference: 2.3318093, amplitude: 1.5078461, left_total_amplitude: 0.75392306, right_total_amplitude: 0.75392306
left_front: 0; right_front: 0; left_rear: 1; right_rear: 0.3; phase_difference: 2.15371, amplitude: 1.4616429, left_total_amplitude: 0.73082143, right_total_amplitude: 0.73082143
left_front: 0; right_front: 0; left_rear: 1; right_rear: 0.2; phase_difference: 1.9655876, amplitude: 1.4277254, left_total_amplitude: 0.7138627, right_total_amplitude: 0.7138627
left_front: 0; right_front: 0; left_rear: 1; right_rear: 0.1; phase_difference: 1.7701335, amplitude: 1.4069825, left_total_amplitude: 0.7034913, right_total_amplitude: 0.7034913

left front to left rear
left_front: 0.1; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 1.7126932, amplitude: 1.4071068, left_total_amplitude: 0.70710677, right_total_amplitude: 0.7
left_front: 0.2; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 1.8490958, amplitude: 1.428011, left_total_amplitude: 0.72801095, right_total_amplitude: 0.7
left_front: 0.3; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 1.975688, amplitude: 1.4615773, left_total_amplitude: 0.7615773, right_total_amplitude: 0.7
left_front: 0.4; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 2.0899422, amplitude: 1.5062258, left_total_amplitude: 0.8062258, right_total_amplitude: 0.7
left_front: 0.5; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 2.1910458, amplitude: 1.5602324, left_total_amplitude: 0.8602325, right_total_amplitude: 0.7
left_front: 0.6; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 2.2794223, amplitude: 1.6219544, left_total_amplitude: 0.9219544, right_total_amplitude: 0.7
left_front: 0.7; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 2.3561943, amplitude: 1.6899494, left_total_amplitude: 0.9899494, right_total_amplitude: 0.7
left_front: 0.8; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 2.4227624, amplitude: 1.7630146, left_total_amplitude: 1.0630145, right_total_amplitude: 0.7
left_front: 0.9; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 2.4805493, amplitude: 1.8401754, left_total_amplitude: 1.1401753, right_total_amplitude: 0.7
left_front: 1; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 2.5308666, amplitude: 1.9206555, left_total_amplitude: 1.2206556, right_total_amplitude: 0.7
left_front: 1; right_front: 0; left_rear: 0.9; right_rear: 0; phase_difference: 2.5794058, amplitude: 1.8119053, left_total_amplitude: 1.1819053, right_total_amplitude: 0.63
left_front: 1; right_front: 0; left_rear: 0.8; right_rear: 0; phase_difference: 2.6311042, amplitude: 1.7061238, left_total_amplitude: 1.1461239, right_total_amplitude: 0.56
left_front: 1; right_front: 0; left_rear: 0.7; right_rear: 0; phase_difference: 2.685977, amplitude: 1.6035978, left_total_amplitude: 1.1135978, right_total_amplitude: 0.48999998
left_front: 1; right_front: 0; left_rear: 0.6; right_rear: 0; phase_difference: 2.7439644, amplitude: 1.5046198, left_total_amplitude: 1.0846198, right_total_amplitude: 0.42000002
left_front: 1; right_front: 0; left_rear: 0.5; right_rear: 0; phase_difference: 2.8049178, amplitude: 1.409481, left_total_amplitude: 1.059481, right_total_amplitude: 0.35
left_front: 1; right_front: 0; left_rear: 0.4; right_rear: 0; phase_difference: 2.8685837, amplitude: 1.3184603, left_total_amplitude: 1.0384604, right_total_amplitude: 0.28
left_front: 1; right_front: 0; left_rear: 0.3; right_rear: 0; phase_difference: 2.9346004, amplitude: 1.2318121, left_total_amplitude: 1.0218121, right_total_amplitude: 0.21000001
left_front: 1; right_front: 0; left_rear: 0.2; right_rear: 0; phase_difference: 3.0024965, amplitude: 1.1497524, left_total_amplitude: 1.0097524, right_total_amplitude: 0.14
left_front: 1; right_front: 0; left_rear: 0.1; right_rear: 0; phase_difference: 3.0717065, amplitude: 1.0724471, left_total_amplitude: 1.002447, right_total_amplitude: 0.07
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
                back_to_front: 0.0,
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
                back_to_front: 1.0,
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
                back_to_front: 1.0,
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
