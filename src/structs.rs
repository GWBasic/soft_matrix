use std::collections::{HashMap, VecDeque};

use rustfft::num_complex::Complex;
use wave_stream::wave_writer::RandomAccessWavWriter;

// An upmixed window, in the time domain
#[derive(Debug)]
pub struct UpmixedWindow {
    pub sample_ctr: i32,
    pub left_front: Vec<Complex<f32>>,
    pub right_front: Vec<Complex<f32>>,
    pub left_rear: Vec<Complex<f32>>,
    pub right_rear: Vec<Complex<f32>>,
}

// Wraps types used during writing so they can be within a mutex
pub struct QueueAndWriter {
    pub upmixed_windows: HashMap<i32, UpmixedWindow>,
    pub upmixed_queue: VecDeque<UpmixedWindow>,
    pub target_wav_writer: RandomAccessWavWriter<f32>,
}
