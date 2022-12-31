use std::collections::VecDeque;

use rustfft::num_complex::Complex;
use wave_stream::{wave_reader::RandomAccessWavReader, wave_writer::RandomAccessWavWriter};

// Allows wrapping information about reading the wav into a single mutex
pub struct OpenWavReaderAndBuffer {
    pub source_wav_reader: RandomAccessWavReader<f32>,
    pub num_threads: usize,
    pub next_read_sample: u32,
    pub left_buffer: VecDeque<Complex<f32>>,
    pub right_buffer: VecDeque<Complex<f32>>,
}

// An upmixed window, in the time domain
#[derive(Debug)]
pub struct UpmixedWindow {
    pub sample_ctr: u32,
    pub left_front: Vec<Complex<f32>>,
    pub right_front: Vec<Complex<f32>>,
    pub left_rear: Vec<Complex<f32>>,
    pub right_rear: Vec<Complex<f32>>,
}

// Wraps types used during writing so they can be within a mutex
pub struct QueueAndWriter {
    pub upmixed_queue: VecDeque<UpmixedWindow>,
    pub target_wav_writer: RandomAccessWavWriter<f32>,
}
