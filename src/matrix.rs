use std::f32::consts::{PI, TAU};

use rustfft::num_complex::Complex;

use crate::{structs::ThreadState, window_sizes};

pub trait Matrix {
    fn phase_shift(
        &self,
        thread_state: &ThreadState,
        freq_ctr: usize,
        left_front_phase: &mut f32,
        right_front_phase: &mut f32,
        left_rear_phase: &mut f32,
        right_rear_phase: &mut f32,
    );
}

pub struct PhaseMatrix {
    wavelengths: Vec<f32>,
    left_rear_phase_shifts: Vec<f32>,
    right_rear_phase_shifts: Vec<f32>,
}

impl PhaseMatrix {
    pub fn default(window_size: usize) -> PhaseMatrix {
        Self::new(window_size, -0.5 * PI, 0.5 * PI)
    }

    // Note that it is intended that PhaseMatrix can be configured to support the old quad matrixes
    fn new(window_size: usize, left_rear_shift: f32, right_rear_shift: f32) -> PhaseMatrix {
        let left_rear_shift_fraction = left_rear_shift / TAU;
        let right_rear_shift_fraction = right_rear_shift / TAU;
        let window_midpoint = window_size / 2;
        let window_size_f32 = window_size as f32;

        let mut wavelengths = Vec::with_capacity(window_midpoint - 1);
        let mut left_rear_phase_shifts = Vec::with_capacity(window_midpoint - 1);
        let mut right_rear_phase_shifts = Vec::with_capacity(window_midpoint - 1);

        // Out of 8
        // 1, 2, 3, 4
        for transform_index in 1..(window_midpoint + 1) {
            // 8, 4, 2, 1
            let wavelength = window_size_f32 / (transform_index as f32);
            wavelengths.push(wavelength);

            let mut left_rear_shift_wavelength = wavelength * left_rear_shift_fraction;
            let mut right_rear_shift_wavelength = wavelength * right_rear_shift_fraction;

            if left_rear_shift_wavelength < 0.0 {
                left_rear_shift_wavelength += wavelength;
            }
            if right_rear_shift_wavelength < 0.0 {
                right_rear_shift_wavelength += wavelength;
            }

            left_rear_phase_shifts.push(left_rear_shift_wavelength);
            right_rear_phase_shifts.push(right_rear_shift_wavelength);
        }

        PhaseMatrix {
            wavelengths,
            left_rear_phase_shifts,
            right_rear_phase_shifts,
        }
    }
}

impl Matrix for PhaseMatrix {
    fn phase_shift(
        &self,
        _thread_state: &ThreadState,
        freq_ctr: usize,
        _left_front_phase: &mut f32,
        _right_front_phase: &mut f32,
        left_rear_phase: &mut f32,
        right_rear_phase: &mut f32,
    ) {
        let index = freq_ctr - 1;
        let wavelength = self.wavelengths[index];
        let left_rear_shift_wavelength = self.left_rear_phase_shifts[index];
        let right_rear_shift_wavelength = self.right_rear_phase_shifts[index];

        *left_rear_phase += left_rear_shift_wavelength;
        if *left_rear_phase > wavelength {
            *left_rear_phase -= wavelength
        }

        *right_rear_phase += right_rear_shift_wavelength;
        if *right_rear_phase > wavelength {
            *right_rear_phase -= wavelength
        }
    }
}
