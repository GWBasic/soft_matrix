use std::io::{Read, Result, Seek};
use std::sync::Arc;

use rustfft::Fft;
use rustfft::{num_complex::Complex, FftPlanner};
use wave_stream::open_wav::OpenWav;
use wave_stream::wave_reader::{OpenWavReader, RandomAccessOpenWavReader, RandomAccessWavReader};
use wave_stream::wave_writer::{OpenWavWriter, RandomAccessWavWriter};

pub fn upmix<TReader: 'static + Read + Seek>(
    source_wav_reader: OpenWavReader<TReader>,
    target_wav_writer: OpenWavWriter,
) -> Result<()> {
    let min_window_size = source_wav_reader.sample_rate() / 20;
    let mut window_size = 2;
    while window_size < min_window_size {
        window_size *= 2;
    }
    let window_size = window_size as usize;

    let mut source_wav_reader = source_wav_reader.get_random_access_f32_reader()?;
    let mut target_wav_writer = target_wav_writer.get_random_access_f32_writer()?;

    let mut planner = FftPlanner::new();
    let fft_forward = planner.plan_fft_forward(window_size);
    let fft_inverse = planner.plan_fft_inverse(window_size);

    let scale: f32 = 1.0 / (window_size as f32);

    let mut scratch_forward = vec![
        Complex {
            re: 0.0f32,
            im: 0.0f32
        };
        fft_forward.get_inplace_scratch_len()
    ];
    let mut scratch_inverse = vec![
        Complex {
            re: 0.0f32,
            im: 0.0f32
        };
        fft_inverse.get_inplace_scratch_len()
    ];
    let mut right_buffer = vec![
        Complex {
            re: 0.0f32,
            im: 0.0f32
        };
        (window_size / 2) + 1
    ];
    let mut left_buffer = vec![
        Complex {
            re: 0.0f32,
            im: 0.0f32
        };
        (window_size / 2) + 1
    ];

    for sample_ctr in 0..(((window_size / 2) - 1) as u32) {
        let left_sample = source_wav_reader.read_sample(sample_ctr, 0)?;
        left_buffer.push(Complex {
            re: left_sample,
            im: 0.0f32,
        });

        let right_sample = source_wav_reader.read_sample(sample_ctr, 1)?;
        right_buffer.push(Complex {
            re: right_sample,
            im: 0.0f32,
        });
    }

    let read_offset = (window_size / 2) as u32;
    for sample_ctr in 0..source_wav_reader.info().len_samples() {
        upmix_sample(
            scale,
            &mut source_wav_reader,
            &mut target_wav_writer,
            &fft_forward,
            &fft_inverse,
            &mut left_buffer,
            &mut right_buffer,
            &mut scratch_forward,
            &mut scratch_inverse,
            sample_ctr,
            read_offset,
        )?;
    }

    target_wav_writer.flush()?;

    Ok(())
}

fn upmix_sample(
    scale: f32,
    source_wav_reader: &mut RandomAccessWavReader<f32>,
    target_wav_writer: &mut RandomAccessWavWriter<f32>,
    fft_forward: &Arc<dyn Fft<f32>>,
    fft_inverse: &Arc<dyn Fft<f32>>,
    left_buffer: &mut Vec<Complex<f32>>,
    right_buffer: &mut Vec<Complex<f32>>,
    scratch_forward: &mut Vec<Complex<f32>>,
    scratch_inverse: &mut Vec<Complex<f32>>,
    sample_ctr: u32,
    read_offset: u32,
) -> Result<()> {
    left_buffer.remove(0);
    right_buffer.remove(0);

    let sample_to_read = sample_ctr + read_offset;

    if sample_to_read < source_wav_reader.info().len_samples() {
        let left_sample = source_wav_reader.read_sample(sample_to_read, 0)?;
        left_buffer.push(Complex {
            re: left_sample,
            im: 0.0f32,
        });

        let right_sample = source_wav_reader.read_sample(sample_to_read, 1)?;
        right_buffer.push(Complex {
            re: right_sample,
            im: 0.0f32,
        });
    } else {
        left_buffer.push(Complex {
            re: 0.0f32,
            im: 0.0f32,
        });
        right_buffer.push(Complex {
            re: 0.0f32,
            im: 0.0f32,
        });
    }

    let mut left_front = left_buffer.to_vec();
    let mut right_front = right_buffer.to_vec();

    fft_forward.process_with_scratch(&mut left_front, scratch_forward);
    fft_forward.process_with_scratch(&mut right_front, scratch_forward);
    let mut left_rear = left_front.to_vec();
    let mut right_rear = right_front.to_vec();

    let window_size = left_buffer.len();
    let midpoint = window_size / 2;
    for freq_ctr in 1..(midpoint + 1) {
        // Phase is offset from sine/cos in # of samples
        let samples_in_freq = (window_size / freq_ctr) as f32;
        let samples_shifted_left = normalize_samples_shifted(left_front[freq_ctr].im, samples_in_freq);
        let samples_shifted_right = normalize_samples_shifted(right_front[freq_ctr].im, samples_in_freq);

        let samples_shifted_difference = (samples_shifted_left - samples_shifted_right).abs();
        
        // phase ratio: 0 is in phase, 1 is out of phase
        let phase_ratio_rear = samples_shifted_difference / samples_in_freq;
        let phase_ratio_front = 1f32 - phase_ratio_rear;

        // Shift balance to front or rear
        left_front[freq_ctr].re *= phase_ratio_front;
        right_front[freq_ctr].re *= phase_ratio_front;
        left_rear[freq_ctr].re *= phase_ratio_rear;
        right_rear[freq_ctr].re *= phase_ratio_rear;

        if freq_ctr < midpoint {
            let inverse_freq_ctr = midpoint + (midpoint - freq_ctr);
            left_front[inverse_freq_ctr].re *= phase_ratio_front;
            right_front[inverse_freq_ctr].re *= phase_ratio_front;
            left_rear[inverse_freq_ctr].re *= phase_ratio_rear;
            right_rear[inverse_freq_ctr].re *= phase_ratio_rear;
        }
    }

    fft_inverse.process_with_scratch(&mut left_front, scratch_inverse);
    fft_inverse.process_with_scratch(&mut right_front, scratch_inverse);
    fft_inverse.process_with_scratch(&mut left_rear, scratch_inverse);
    fft_inverse.process_with_scratch(&mut right_rear, scratch_inverse);

    let sample_ctr_in_buffer = right_buffer.len() / 2;
    // TODO: Scale
    target_wav_writer.write_sample(sample_ctr, 0, scale * left_front[sample_ctr_in_buffer].re)?;
    target_wav_writer.write_sample(sample_ctr, 1, scale * right_front[sample_ctr_in_buffer].re)?;
    target_wav_writer.write_sample(sample_ctr, 2, scale * left_rear[sample_ctr_in_buffer].re)?;
    target_wav_writer.write_sample(sample_ctr, 3, scale * right_rear[sample_ctr_in_buffer].re)?;

    Ok(())
}

fn normalize_samples_shifted(mut samples_shifted: f32, samples_in_freq: f32) -> f32 {
    while samples_shifted < 0f32 {
        samples_shifted += samples_in_freq;
    }

    while samples_shifted > samples_in_freq {
        samples_shifted -= samples_in_freq;
    }

    samples_shifted
}