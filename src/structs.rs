use std::{collections::VecDeque, time::Instant};

use rustfft::num_complex::Complex;
use wave_stream::{wave_reader::RandomAccessWavReader, wave_writer::RandomAccessWavWriter};

// Allows wrapping information about reading the wav into a single mutex
pub struct OpenWavReaderAndBuffer {
    pub source_wav_reader: RandomAccessWavReader<f32>,
    pub next_read_sample: u32,
    pub left_buffer: VecDeque<Complex<f32>>,
    pub right_buffer: VecDeque<Complex<f32>>,
}

// A window, transformed forward via fft; and all of the positions of each frequency
#[derive(Debug)]
pub struct TransformedWindowAndPans {
    // The index of the last sample in the transforms
    pub last_sample_ctr: u32,
    pub left_transformed: Vec<Complex<f32>>,
    pub right_transformed: Vec<Complex<f32>>,
    pub frequency_pans: Vec<FrequencyPans>,
}

/*
pub struct PansForSample {
    pub sample_ctr: u32,
    // This sample's transformations and pans
    pub transformed_window_and_pans: TransformedWindowAndPans,
    // Other pans for samples that are in the window
    pub other_pans: Vec<Vec<FrequencyPans>>,
    pub expected_other_pans_count: usize,
    pub first_sample: u32,
    pub last_sample: u32,
}
*/

// The position of a frequency at a specific moment in time
#[derive(Debug, Clone)]
pub struct FrequencyPans {
    // Comment todo (probably 0 is left, 1 is right)
    //pub left_to_right: f32,
    // Front to back panning, 0 is front, 1 is back
    pub back_to_front: f32,
}

pub struct AveragedFrequencyPans {
    // The index of the last sample in the transforms that these pans apply to
    pub last_sample_ctr: u32,
    pub frequency_pans: Vec<FrequencyPans>,
    pub averaged_frequency_pans: Vec<FrequencyPans>,
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
pub struct WriterState {
    //pub upmixed_queue: VecDeque<UpmixedWindow>,
    pub target_wav_writer: RandomAccessWavWriter<f32>,

    // Used for logging
    pub started: Instant,
    pub next_log: Instant,
    pub total_samples_to_write: f64,
}
