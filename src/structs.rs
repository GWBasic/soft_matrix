use std::{collections::VecDeque, sync::Arc};

use rustfft::num_complex::Complex;

use crate::upmixer::Upmixer;

// State that is local to a thread
pub struct ThreadState {
    pub upmixer: Arc<Upmixer>,

    // Each thread has a separate FFT scratch space
    pub scratch_forward: Vec<Complex<f32>>,
    pub scratch_inverse: Vec<Complex<f32>>,
}

// A window, transformed forward via fft; and all of the positions of each frequency
#[derive(Debug)]
pub struct TransformedWindowAndPans {
    // The index of the last sample in the transforms
    pub last_sample_ctr: usize,
    pub left_transformed: Option<Vec<Complex<f32>>>,
    pub right_transformed: Option<Vec<Complex<f32>>>,
    pub frequency_pans: Vec<FrequencyPans>,
}

// The position of a frequency at a specific moment in time
#[derive(Debug, Clone)]
pub struct FrequencyPans {
    // Comment todo (probably 0 is left, 1 is right)
    //pub left_to_right: f32,
    // Front to back panning, 0 is front, 1 is back
    pub back_to_front: f32,
}
