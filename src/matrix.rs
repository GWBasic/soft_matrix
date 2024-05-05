use std::{
    cell::Cell,
    f32::consts::{PI, TAU},
};

const HALF_PI: f32 = PI / 2.0;

use rustfft::num_complex::Complex;

use crate::structs::FrequencyPans;

// When derriving a center channel:
// An amplitude of 1 in the center is equivalent to 0.707 (square root of 0.5) in both speakers
// (Based on https://music.arts.uci.edu/dobrian/maxcookbook/constant-power-panning-using-square-root-intensity)
// Thus, if a tone has a 1.0 amplitude in both speakers, its real amplitude is 1.414213562373094
// Items panned to the center are usually lowered by 0.707106781186548 in order to be the same volume as when panned to the edge
pub const CENTER_AMPLITUDE_ADJUSTMENT: f32 = 0.707106781186548; // 2.0.sqrt() / 2.0;

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

    fn print_debugging_information(&self);

    fn amplitude_adjustment(&self) -> f32;

    fn steer_right_left(&self) -> bool;
}

pub struct DefaultMatrix {
    widen_factor: f32,
    left_rear_shift: f32,
    right_rear_shift: f32,
    rear_adjustment: f32,
}

// Note that it is intended that DefaultMatrix can be configured to support the old quad matrixes
impl DefaultMatrix {
    pub fn new() -> DefaultMatrix {
        DefaultMatrix {
            widen_factor: 1.0,
            left_rear_shift: -0.5 * PI,
            right_rear_shift: 0.5 * PI,
            rear_adjustment: 1.0,
        }
    }

    pub fn qs() -> DefaultMatrix {
        let largest_sum = 0.924 + 0.383;
        let largest_pan = (0.924 / largest_sum) * 2.0 - 1.0;

        DefaultMatrix {
            widen_factor: 1.0 / largest_pan,
            left_rear_shift: -0.5 * PI,
            right_rear_shift: 0.5 * PI,
            rear_adjustment: 1.0,
        }
    }

    pub fn horseshoe() -> DefaultMatrix {
        DefaultMatrix {
            widen_factor: 2.0,
            left_rear_shift: -0.5 * PI,
            right_rear_shift: 0.5 * PI,
            rear_adjustment: 1.0,
        }
    }

    pub fn dolby_stereo() -> DefaultMatrix {
        DefaultMatrix {
            widen_factor: 1.0,
            left_rear_shift: -0.5 * PI,
            right_rear_shift: 0.5 * PI,
            rear_adjustment: 2.0f32.sqrt(),
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
                amplitude: amplitude_sum,
                left_to_right: 0.0,
                back_to_front: 0.0,
            };
        }

        let mut left_to_right = (left_amplitude / amplitude_sum) * -2.0 + 1.0;

        // Uncomment to sset breakpoints
        //if amplitude_sum > 0.333 && left_to_right < 0.1 && left_to_right > -0.1 {
        //    println!("break");
        //}

        left_to_right *= self.widen_factor;

        let fraction_in_side = left_to_right.abs();
        let fraction_in_center = 1.0 - fraction_in_side;
        let back_to_front_from_panning = (left_to_right.abs() - 1.0).max(0.0);
        let back_to_front = (back_to_front_from_panning + back_to_front_from_phase).min(1.0);
        let front_to_back = 1.0 - back_to_front;

        let amplitude_front = ((fraction_in_side * amplitude_sum) +
            // Items panned to the center are usually lowered to .707 so they are the same volume as when panned to the side
            (fraction_in_center * amplitude_sum * CENTER_AMPLITUDE_ADJUSTMENT))
            * front_to_back;

        let amplitude_back = amplitude_sum * back_to_front * self.rear_adjustment;

        left_to_right = left_to_right.min(1.0).max(-1.0);

        FrequencyPans {
            amplitude: amplitude_back + amplitude_front,
            left_to_right,
            back_to_front,
        }
    }

    fn phase_shift(
        &self,
        _left_front_phase: &mut f32,
        _right_front_phase: &mut f32,
        left_rear_phase: &mut f32,
        right_rear_phase: &mut f32,
    ) {
        shift_in_place(left_rear_phase, self.left_rear_shift);
        shift_in_place(right_rear_phase, self.right_rear_shift);
    }

    fn print_debugging_information(&self) {}

    fn amplitude_adjustment(&self) -> f32 {
        CENTER_AMPLITUDE_ADJUSTMENT
    }

    fn steer_right_left(&self) -> bool {
        false
    }
}

// https://en.wikipedia.org/wiki/Stereo_Quadraphonic
//const SQ_LOWER: f32 = 0.7;
const SQ_RAISE: f32 = 1.0 / 0.7;
const SQ_LEFT_REAR_SHIFT: f32 = PI / 2.0;
const SQ_RIGHT_REAR_SHIFT: f32 = SQ_LEFT_REAR_SHIFT * -1.0;

// Uses the Soft Matrix approach of closely inspecting phase and amplitude, but it doesn't work very well
pub struct SQMatrix {}

impl SQMatrix {
    pub fn sq() -> SQMatrix {
        SQMatrix {}
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
                amplitude: 0.0,
                left_to_right: 0.0,
                back_to_front: 0.0,
            };
        } else if phase_difference.abs() < 0.01
            || left_total_amplitude < 0.01
            || right_total_amplitude < 0.01
        {
            // Sound is in phase: Front isolated
            let left_to_right = (left_total_amplitude / amplitude_sum) * -2.0 + 1.0;

            let fraction_in_side = left_to_right.abs();
            let fraction_in_center = 1.0 - fraction_in_side;

            let amplitude_front = (fraction_in_side * amplitude_sum) +
                // Items panned to the center are usually lowered to .707 so they are the same volume as when panned to the side
                (fraction_in_center * amplitude_sum * CENTER_AMPLITUDE_ADJUSTMENT);

            return FrequencyPans {
                amplitude: amplitude_front,
                left_to_right,
                back_to_front: 0.0,
            };
        } else {
            let left_to_right: f32;
            let back_to_front: f32;

            if phase_difference < 0.0 && phase_difference > (-1.0 * HALF_PI) {
                // Right-isolated, front -> back pan comes from phase
                left_to_right = 1.0;
                back_to_front = (-1.0 * phase_difference) / HALF_PI;
            } else if phase_difference > HALF_PI
                && phase_difference <= PI
                && left_total_amplitude > right_total_amplitude
            {
                // Left-isolated, front -> back pan comes from phase
                left_to_right = -1.0;
                back_to_front = 1.0 - ((phase_difference - HALF_PI) / HALF_PI);
            } else if phase_difference <= (-1.0 * HALF_PI) {
                // Between right rear and rear center
                // right rear to rear center: -(pi/2) -> -pi
                // Sound is out-of-phase, but amplitude is the same: Rear isolated, right -> left pan comes from phase
                left_to_right = (-1.0 * phase_difference / HALF_PI).min(0.0).max(1.0);
                back_to_front = 1.0;
            } else {
                // Between left rear and rear center
                // rear center to left rear: pi -> (pi/2)
                // Sound is out-of-phase, but amplitude is the same: Rear isolated, right -> left pan comes from phase
                left_to_right = (-1.0 * (HALF_PI - (phase_difference - HALF_PI)) / HALF_PI)
                    .min(-1.0)
                    .max(0.0);
                back_to_front = 1.0;
            }

            let front_to_back = 1.0 - back_to_front;
            return FrequencyPans {
                amplitude: (amplitude_sum * front_to_back)
                    + (amplitude_sum * back_to_front * SQ_RAISE),
                left_to_right,
                back_to_front,
            };
        }
    }

    fn phase_shift(
        &self,
        _left_front_phase: &mut f32,
        _right_front_phase: &mut f32,
        left_rear_phase: &mut f32,
        right_rear_phase: &mut f32,
    ) {
        shift_in_place(left_rear_phase, SQ_LEFT_REAR_SHIFT);
        shift_in_place(right_rear_phase, SQ_RIGHT_REAR_SHIFT);
    }

    fn print_debugging_information(&self) {}

    fn amplitude_adjustment(&self) -> f32 {
        CENTER_AMPLITUDE_ADJUSTMENT
    }

    fn steer_right_left(&self) -> bool {
        true
    }
}

// Attempts to follow a "by the book" dematrixer, except for when something is in the front
// Doesn't work very well

pub struct SQMatrixExperimental {
    min_back_to_front: Cell<f32>,
    max_back_to_front: Cell<f32>,
    min_left_to_right: Cell<f32>,
    max_left_to_right: Cell<f32>,
}

impl SQMatrixExperimental {
    pub fn sq() -> SQMatrixExperimental {
        SQMatrixExperimental {
            min_back_to_front: Cell::new(f32::INFINITY),
            max_back_to_front: Cell::new(f32::NEG_INFINITY),
            min_left_to_right: Cell::new(f32::INFINITY),
            max_left_to_right: Cell::new(f32::NEG_INFINITY),
        }
    }
}

impl Matrix for SQMatrixExperimental {
    fn steer(
        &self,
        left_total_amplitude: f32,
        left_phase: f32,
        right_total_amplitude: f32,
        right_phase: f32,
    ) -> FrequencyPans {
        let amplitude_sum = left_total_amplitude + right_total_amplitude;

        let mut phase_difference = left_phase - right_phase;
        bring_phase_in_range(&mut phase_difference);

        if amplitude_sum == 0.0 {
            return FrequencyPans {
                amplitude: 0.0,
                left_to_right: 0.0,
                back_to_front: 0.0,
            };
        } else if phase_difference.abs() < 0.01
            || left_total_amplitude < 0.01
            || right_total_amplitude < 0.01
        {
            // Sound is in phase: Front isolated
            let left_to_right = (left_total_amplitude / amplitude_sum) * -2.0 + 1.0;

            let fraction_in_side = left_to_right.abs();
            let fraction_in_center = 1.0 - fraction_in_side;

            let amplitude_front = (fraction_in_side * amplitude_sum) +
                // Items panned to the center are usually lowered to .707 so they are the same volume as when panned to the side
                (fraction_in_center * amplitude_sum * CENTER_AMPLITUDE_ADJUSTMENT);

            return FrequencyPans {
                amplitude: amplitude_front,
                left_to_right,
                back_to_front: 0.0,
            };
        } else {
            // http://www.hi-ho.ne.jp/odaka/quad/index-e.html
            /*
            LF =        L
            RF =        R
            LR = -0.5 * L * ( 1 – i ) - 0.5 * R * ( 1 + i )
            RR =  0.5 * L * ( 1 + i ) + 0.5 * R * ( 1 – i )
            */

            // It appears that i is a 90 degree phase shift
            // Re-interpreting (for readability)
            /*
            LF =        L
            RF =        R
            LR = (-0.5 * L * –i ) - (0.5 * R *  i )
            RR =  (0.5 * L *  i ) + (0.5 * R * –i )
            */

            //let left_total = Complex::from_polar(left_total_amplitude, left_phase);
            //let right_total = Complex::from_polar(right_total_amplitude, right_phase);

            /*
            let left_back = Complex::from_polar(left_total_amplitude * SQ_RAISE / 2.0, shift(left_phase, HALF_PI)) +
                Complex::from_polar(right_total_amplitude * SQ_RAISE / 2.0, shift(right_phase, PI));

            let right_back = Complex::from_polar(left_total_amplitude * SQ_RAISE / 2.0, left_phase) +
                Complex::from_polar(right_total_amplitude * SQ_RAISE / 2.0, shift(right_phase, HALF_PI * -1.0));
            */
            let left_back =
                Complex::from_polar(
                    left_total_amplitude / 2.0,
                    shift(left_phase, -1.0 * HALF_PI),
                ) + Complex::from_polar(right_total_amplitude / 2.0, shift(right_phase, HALF_PI));

            let right_back =
                Complex::from_polar(left_total_amplitude / 2.0, shift(left_phase, HALF_PI))
                    + Complex::from_polar(
                        right_total_amplitude / 2.0,
                        shift(right_phase, HALF_PI * -1.0),
                    );

            let (left_back_amplitude, _) = left_back.to_polar();
            let (right_back_amplitude, _) = right_back.to_polar();

            let total_amplitude = left_total_amplitude + right_total_amplitude;
            let back_amplitude = left_back_amplitude + right_back_amplitude;
            //let left_front_amplitude = left_total_amplitude - left_back_amplitude;
            //let right_front_amplitude = right_total_amplitude - right_back_amplitude;

            let back_to_front = back_amplitude / total_amplitude; //((back_amplitude / total_amplitude) - 0.5) * 2.0;
                                                                  //let front_to_back = 1.0 - back_to_front;

            let left_to_right = (2.0 * (right_total_amplitude / total_amplitude)) - 1.0;
            /*
            let left_to_right_front = ((2.0 * (right_total_amplitude / total_amplitude)) - 1.0) * 4.0;
            let left_to_right_rear = ((2.0 * (right_back_amplitude / back_amplitude)) - 1.0) * 4.0;
            let left_to_right =
                (left_to_right_front * front_to_back) + (left_to_right_rear * back_to_front);
            */

            //let amplitude = (total_amplitude * front_to_back) + (total_amplitude * back_to_front * SQ_RAISE);

            self.min_back_to_front
                .replace(back_to_front.min(self.min_back_to_front.get()));
            self.max_back_to_front
                .replace(back_to_front.max(self.max_back_to_front.get()));
            self.min_left_to_right
                .replace(left_to_right.min(self.min_left_to_right.get()));
            self.max_left_to_right
                .replace(left_to_right.max(self.max_left_to_right.get()));

            FrequencyPans {
                amplitude: total_amplitude,
                left_to_right,
                back_to_front,
            }
        }
    }

    fn phase_shift(
        &self,
        _left_front_phase: &mut f32,
        _right_front_phase: &mut f32,
        left_rear_phase: &mut f32,
        right_rear_phase: &mut f32,
    ) {
        shift_in_place(left_rear_phase, SQ_LEFT_REAR_SHIFT);
        shift_in_place(right_rear_phase, SQ_RIGHT_REAR_SHIFT);
    }

    fn print_debugging_information(&self) {
        /*
        println!();

        println!("min_back_to_front: {}", self.min_back_to_front.get());
        println!("max_back_to_front: {}", self.max_back_to_front.get());
        println!("min_left_to_right: {}", self.min_left_to_right.get());
        println!("max_left_to_right: {}", self.max_left_to_right.get());

        println!();
        */
    }

    fn amplitude_adjustment(&self) -> f32 {
        CENTER_AMPLITUDE_ADJUSTMENT
    }

    fn steer_right_left(&self) -> bool {
        true
    }
}

fn shift(phase: f32, shift: f32) -> f32 {
    let mut phase_mut = phase;
    shift_in_place(&mut phase_mut, shift);
    phase_mut
}

fn shift_in_place(phase: &mut f32, shift: f32) {
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
