use std::f32::consts::{PI, TAU};

const HALF_PI: f32 = PI / 2.0;

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

        if amplitude_sum == 0.0 {
            return FrequencyPans {
                left_to_right: 0.0,
                back_to_front: 0.0,
            };
        }

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
        left_total_amplitude: f32,
        left_phase: f32,
        right_total_amplitude: f32,
        right_phase: f32,
    ) -> FrequencyPans {
        /*

        right front to right rear
        left_front: 0; right_front: 1; left_rear: 0; right_rear: 0.1; phase_difference: -0.06988599896430969238, amplitude: 1.0724471, left_total_amplitude: 0.07, right_total_amplitude: 1.002447
        left_front: 0; right_front: 1; left_rear: 0; right_rear: 0.2; phase_difference: -0.13909594714641571045, amplitude: 1.1497524, left_total_amplitude: 0.14, right_total_amplitude: 1.0097524
        left_front: 0; right_front: 1; left_rear: 0; right_rear: 0.3; phase_difference: -0.20699220895767211914, amplitude: 1.2318121, left_total_amplitude: 0.21000001, right_total_amplitude: 1.0218121
        left_front: 0; right_front: 1; left_rear: 0; right_rear: 0.4; phase_difference: -0.27300870418548583984, amplitude: 1.3184603, left_total_amplitude: 0.28, right_total_amplitude: 1.0384604
        left_front: 0; right_front: 1; left_rear: 0; right_rear: 0.5; phase_difference: -0.33667480945587158203, amplitude: 1.409481, left_total_amplitude: 0.35, right_total_amplitude: 1.059481
        left_front: 0; right_front: 1; left_rear: 0; right_rear: 0.6; phase_difference: -0.39762800931930541992, amplitude: 1.5046198, left_total_amplitude: 0.42000002, right_total_amplitude: 1.0846198
        left_front: 0; right_front: 1; left_rear: 0; right_rear: 0.7; phase_difference: -0.45561563968658447266, amplitude: 1.6035978, left_total_amplitude: 0.48999998, right_total_amplitude: 1.1135978
        left_front: 0; right_front: 1; left_rear: 0; right_rear: 0.8; phase_difference: -0.51048833131790161133, amplitude: 1.7061238, left_total_amplitude: 0.56, right_total_amplitude: 1.1461239
        left_front: 0; right_front: 1; left_rear: 0; right_rear: 0.9; phase_difference: -0.56218671798706054688, amplitude: 1.8119053, left_total_amplitude: 0.63, right_total_amplitude: 1.1819053
        left_front: 0; right_front: 1; left_rear: 0; right_rear: 1; phase_difference: -0.61072599887847900391, amplitude: 1.9206555, left_total_amplitude: 0.7, right_total_amplitude: 1.2206556
        left_front: 0; right_front: 0.9; left_rear: 0; right_rear: 1; phase_difference: -0.66104322671890258789, amplitude: 1.8401754, left_total_amplitude: 0.7, right_total_amplitude: 1.1401753
        left_front: 0; right_front: 0.8; left_rear: 0; right_rear: 1; phase_difference: -0.71882998943328857422, amplitude: 1.7630146, left_total_amplitude: 0.7, right_total_amplitude: 1.0630145
        left_front: 0; right_front: 0.7; left_rear: 0; right_rear: 1; phase_difference: -0.78539818525314331055, amplitude: 1.6899494, left_total_amplitude: 0.7, right_total_amplitude: 0.9899494
        left_front: 0; right_front: 0.6; left_rear: 0; right_rear: 1; phase_difference: -0.86217010021209716797, amplitude: 1.6219544, left_total_amplitude: 0.7, right_total_amplitude: 0.9219544
        left_front: 0; right_front: 0.5; left_rear: 0; right_rear: 1; phase_difference: -0.95054686069488525391, amplitude: 1.5602324, left_total_amplitude: 0.7, right_total_amplitude: 0.8602325
        left_front: 0; right_front: 0.4; left_rear: 0; right_rear: 1; phase_difference: -1.05165028572082519531, amplitude: 1.5062258, left_total_amplitude: 0.7, right_total_amplitude: 0.8062258
        left_front: 0; right_front: 0.3; left_rear: 0; right_rear: 1; phase_difference: -1.16590452194213867188, amplitude: 1.4615773, left_total_amplitude: 0.7, right_total_amplitude: 0.7615773
        left_front: 0; right_front: 0.2; left_rear: 0; right_rear: 1; phase_difference: -1.29249668121337890625, amplitude: 1.428011, left_total_amplitude: 0.7, right_total_amplitude: 0.72801095
        left_front: 0; right_front: 0.1; left_rear: 0; right_rear: 1; phase_difference: -1.42889928817749023438, amplitude: 1.4071068, left_total_amplitude: 0.7, right_total_amplitude: 0.70710677
        left_front: 0; right_front: 0; left_rear: 0; right_rear: 1; phase_difference: -1.57079637050628662109, amplitude: 1.4, left_total_amplitude: 0.7, right_total_amplitude: 0.7

        right rear to left rear
        left_front: 0; right_front: 0; left_rear: 0; right_rear: 1; phase_difference: -1.57079637050628662109, amplitude: 1.4, left_total_amplitude: 0.7, right_total_amplitude: 0.7
        left_front: 0; right_front: 0; left_rear: 0.1; right_rear: 1; phase_difference: -1.77013361454010009766, amplitude: 1.4069825, left_total_amplitude: 0.7034913, right_total_amplitude: 0.7034913
        left_front: 0; right_front: 0; left_rear: 0.2; right_rear: 1; phase_difference: -1.96558749675750732422, amplitude: 1.4277254, left_total_amplitude: 0.7138627, right_total_amplitude: 0.7138627
        left_front: 0; right_front: 0; left_rear: 0.3; right_rear: 1; phase_difference: -2.15370988845825195312, amplitude: 1.4616429, left_total_amplitude: 0.73082143, right_total_amplitude: 0.73082143
        left_front: 0; right_front: 0; left_rear: 0.4; right_rear: 1; phase_difference: -2.33180904388427734375, amplitude: 1.5078461, left_total_amplitude: 0.75392306, right_total_amplitude: 0.75392306
        left_front: 0; right_front: 0; left_rear: 0.5; right_rear: 1; phase_difference: -2.49809169769287109375, amplitude: 1.5652475, left_total_amplitude: 0.78262377, right_total_amplitude: 0.78262377
        left_front: 0; right_front: 0; left_rear: 0.6; right_rear: 1; phase_difference: -2.65163540840148925781, amplitude: 1.6326665, left_total_amplitude: 0.81633323, right_total_amplitude: 0.81633323
        left_front: 0; right_front: 0; left_rear: 0.7; right_rear: 1; phase_difference: -2.79224824905395507812, amplitude: 1.7089177, left_total_amplitude: 0.85445887, right_total_amplitude: 0.85445887
        left_front: 0; right_front: 0; left_rear: 0.8; right_rear: 1; phase_difference: -2.92027831077575683594, amplitude: 1.7928748, left_total_amplitude: 0.8964374, right_total_amplitude: 0.89643735
        left_front: 0; right_front: 0; left_rear: 0.9; right_rear: 1; phase_difference: -3.03642654418945312500, amplitude: 1.8835074, left_total_amplitude: 0.9417537, right_total_amplitude: 0.9417537
        left_front: 0; right_front: 0; left_rear: 1; right_rear: 1; phase_difference: -3.14159274101257324219, amplitude: 1.9798989, left_total_amplitude: 0.9899494, right_total_amplitude: 0.98994946
        left_front: 0; right_front: 0; left_rear: 1; right_rear: 0.9; phase_difference: 3.03642654418945312500, amplitude: 1.8835073, left_total_amplitude: 0.9417536, right_total_amplitude: 0.9417536
        left_front: 0; right_front: 0; left_rear: 1; right_rear: 0.8; phase_difference: 2.92027831077575683594, amplitude: 1.7928747, left_total_amplitude: 0.89643735, right_total_amplitude: 0.89643735
        left_front: 0; right_front: 0; left_rear: 1; right_rear: 0.7; phase_difference: 2.79224824905395507812, amplitude: 1.7089176, left_total_amplitude: 0.85445887, right_total_amplitude: 0.8544588
        left_front: 0; right_front: 0; left_rear: 1; right_rear: 0.6; phase_difference: 2.65163564682006835938, amplitude: 1.6326665, left_total_amplitude: 0.81633323, right_total_amplitude: 0.81633323
        left_front: 0; right_front: 0; left_rear: 1; right_rear: 0.5; phase_difference: 2.49809169769287109375, amplitude: 1.5652475, left_total_amplitude: 0.78262377, right_total_amplitude: 0.78262377
        left_front: 0; right_front: 0; left_rear: 1; right_rear: 0.4; phase_difference: 2.33180928230285644531, amplitude: 1.5078461, left_total_amplitude: 0.75392306, right_total_amplitude: 0.75392306
        left_front: 0; right_front: 0; left_rear: 1; right_rear: 0.3; phase_difference: 2.15370988845825195312, amplitude: 1.4616429, left_total_amplitude: 0.73082143, right_total_amplitude: 0.73082143
        left_front: 0; right_front: 0; left_rear: 1; right_rear: 0.2; phase_difference: 1.96558761596679687500, amplitude: 1.4277254, left_total_amplitude: 0.7138627, right_total_amplitude: 0.7138627
        left_front: 0; right_front: 0; left_rear: 1; right_rear: 0.1; phase_difference: 1.77013349533081054688, amplitude: 1.4069825, left_total_amplitude: 0.7034913, right_total_amplitude: 0.7034913
        left_front: 0; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 1.57079613208770751953, amplitude: 1.4, left_total_amplitude: 0.7, right_total_amplitude: 0.7

        left rear to left front
        left_front: 0; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 1.57079613208770751953, amplitude: 1.4, left_total_amplitude: 0.7, right_total_amplitude: 0.7
        left_front: 0.1; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 1.71269321441650390625, amplitude: 1.4071068, left_total_amplitude: 0.70710677, right_total_amplitude: 0.7
        left_front: 0.2; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 1.84909582138061523438, amplitude: 1.428011, left_total_amplitude: 0.72801095, right_total_amplitude: 0.7
        left_front: 0.3; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 1.97568798065185546875, amplitude: 1.4615773, left_total_amplitude: 0.7615773, right_total_amplitude: 0.7
        left_front: 0.4; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 2.08994221687316894531, amplitude: 1.5062258, left_total_amplitude: 0.8062258, right_total_amplitude: 0.7
        left_front: 0.5; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 2.19104576110839843750, amplitude: 1.5602324, left_total_amplitude: 0.8602325, right_total_amplitude: 0.7
        left_front: 0.6; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 2.27942228317260742188, amplitude: 1.6219544, left_total_amplitude: 0.9219544, right_total_amplitude: 0.7
        left_front: 0.7; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 2.35619425773620605469, amplitude: 1.6899494, left_total_amplitude: 0.9899494, right_total_amplitude: 0.7
        left_front: 0.8; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 2.42276239395141601562, amplitude: 1.7630146, left_total_amplitude: 1.0630145, right_total_amplitude: 0.7
        left_front: 0.9; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 2.48054933547973632812, amplitude: 1.8401754, left_total_amplitude: 1.1401753, right_total_amplitude: 0.7
        left_front: 1; right_front: 0; left_rear: 1; right_rear: 0; phase_difference: 2.53086662292480468750, amplitude: 1.9206555, left_total_amplitude: 1.2206556, right_total_amplitude: 0.7
        left_front: 1; right_front: 0; left_rear: 0.9; right_rear: 0; phase_difference: 2.57940578460693359375, amplitude: 1.8119053, left_total_amplitude: 1.1819053, right_total_amplitude: 0.63
        left_front: 1; right_front: 0; left_rear: 0.8; right_rear: 0; phase_difference: 2.63110423088073730469, amplitude: 1.7061238, left_total_amplitude: 1.1461239, right_total_amplitude: 0.56
        left_front: 1; right_front: 0; left_rear: 0.7; right_rear: 0; phase_difference: 2.68597698211669921875, amplitude: 1.6035978, left_total_amplitude: 1.1135978, right_total_amplitude: 0.48999998
        left_front: 1; right_front: 0; left_rear: 0.6; right_rear: 0; phase_difference: 2.74396443367004394531, amplitude: 1.5046198, left_total_amplitude: 1.0846198, right_total_amplitude: 0.42000002
        left_front: 1; right_front: 0; left_rear: 0.5; right_rear: 0; phase_difference: 2.80491781234741210938, amplitude: 1.409481, left_total_amplitude: 1.059481, right_total_amplitude: 0.35
        left_front: 1; right_front: 0; left_rear: 0.4; right_rear: 0; phase_difference: 2.86858367919921875000, amplitude: 1.3184603, left_total_amplitude: 1.0384604, right_total_amplitude: 0.28
        left_front: 1; right_front: 0; left_rear: 0.3; right_rear: 0; phase_difference: 2.93460035324096679688, amplitude: 1.2318121, left_total_amplitude: 1.0218121, right_total_amplitude: 0.21000001
        left_front: 1; right_front: 0; left_rear: 0.2; right_rear: 0; phase_difference: 3.00249648094177246094, amplitude: 1.1497524, left_total_amplitude: 1.0097524, right_total_amplitude: 0.14
        left_front: 1; right_front: 0; left_rear: 0.1; right_rear: 0; phase_difference: 3.07170653343200683594, amplitude: 1.0724471, left_total_amplitude: 1.002447, right_total_amplitude: 0.07
        left_front: 1; right_front: 0; left_rear: 0; right_rear: 0; phase_difference: 0.00000000000000000000, amplitude: 1, left_total_amplitude: 1, right_total_amplitude: 0
        */

        // Phase differences
        // Right front isolated -> Right rear isolated: 0 -> -1.42889928817749023438
        // Right rear isolated -> rear center isolated: -1.42889928817749023438 -> -pi
        // Rear center isolated -> left rear isolated: pi -> 1.77013349533081054688 (Amplitudes are generally equal in input channels)
        // Left rear isolated -> left front isolated: 1.71269321441650390625 -> 3.07170653343200683594 (Amplitude louder in left total)
        // Front channels: > 3.0717065, 0,
        let amplitude_sum = left_total_amplitude + right_total_amplitude;

        let mut phase_difference = left_phase - right_phase;
        bring_phase_in_range(&mut phase_difference);

        if amplitude_sum == 0.0 {
            return FrequencyPans {
                left_to_right: 0.0,
                back_to_front: 0.0,
            };
        } else if phase_difference.abs() < 0.01
            || left_total_amplitude < 0.01
            || right_total_amplitude < 0.01
        {
            // Sound is in phase: Front isolated
            let left_to_right = (left_total_amplitude / amplitude_sum) * -2.0 + 1.0;
            return FrequencyPans {
                left_to_right,
                back_to_front: 0.0,
            };
        } else {
            let amplitude_difference = left_total_amplitude - right_total_amplitude;

            if amplitude_difference > 0.001 {
                // Left-isolated, front -> back pan comes from phase
                return FrequencyPans {
                    left_to_right: -1.0,
                    back_to_front: if phase_difference >= 0.0 {
                        0.0
                    } else {
                        1.0 - ((phase_difference - HALF_PI) / HALF_PI).max(1.0).min(0.0)
                    },
                };
            } else if amplitude_difference < -0.001 {
                // Right-isolated, front -> back pan comes from phase
                return FrequencyPans {
                    left_to_right: 1.0,
                    back_to_front: if phase_difference >= 0.0 {
                        0.0
                    } else {
                        ((-1.0 * phase_difference) / HALF_PI).max(1.0).min(0.0)
                    },
                };
            } else {
                // Sound is out-of-phase, but amplitude is the same: Rear isolated, right -> left pan comes from phase
                return FrequencyPans {
                    left_to_right: -1.0 * phase_difference / HALF_PI,
                    back_to_front: 1.0,
                };
            }
        }

        /*
        if phase_difference < 0.0 && phase_difference >= -1.57079637050628662109 {
            // Right front -> Right Rear
            return FrequencyPans {
                left_to_right: 1.0,
                back_to_front: phase_difference / -1.57079637050628662109,
            };
        } else if phase_difference < -1.57079637050628662109 {
            // Right rear -> rear center
            return FrequencyPans {
                left_to_right: (phase_difference - PI) / 1.57079613208770751953,
                back_to_front: 1.0,
            };
        } else if phase_difference >= 1.57079613208770751953 && amplitude_difference < 0.0001 {
            // Rear center -> left rear
            return FrequencyPans {
                left_to_right: (phase_difference + PI) / 1.57079637050628662109,
                back_to_front: 1.0,
            };
        } else if phase_difference >= 1.57079613208770751953 && left_amplitude > right_amplitude {
            // Left rear -> left front
            return FrequencyPans {
                left_to_right: -1.0,
                back_to_front: 1.0
                    - ((phase_difference - 1.57079613208770751953) / 1.57079613208770751953),
            };
        } else {
            // else front isolated
            return FrequencyPans {
                left_to_right: (left_amplitude / amplitude_sum) * 2.0 - 1.0,
                back_to_front: 0.0,
            };
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
    bring_phase_in_range(phase);
}

fn bring_phase_in_range(phase: &mut f32) {
    if *phase > PI {
        *phase -= TAU;
    } else if *phase < -PI {
        *phase += TAU;
    }
}
