use std::collections::VecDeque;
use std::io::{Read, Result, Seek};
use std::sync::Arc;

use rustfft::Fft;
use rustfft::{num_complex::Complex, FftPlanner};
use wave_stream::open_wav::OpenWav;
use wave_stream::wave_reader::{OpenWavReader, RandomAccessOpenWavReader, RandomAccessWavReader};
use wave_stream::wave_writer::{OpenWavWriter, RandomAccessWavWriter};

use crate::structs::FrequenciesAndPositions;

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
    let position_samples_length = (source_wav_reader.sample_rate() / 20) as usize;

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

    let mut frequences_and_positions_queue = VecDeque::<FrequenciesAndPositions>::new();

    let read_offset = (window_size / 2) as u32;
    for sample_ctr in 0..source_wav_reader.info().len_samples() {
        upmix_sample(
            scale,
            &mut source_wav_reader,
            &mut target_wav_writer,
            position_samples_length,
            &mut frequences_and_positions_queue,
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
    position_samples_length: usize,
    frequences_and_positions_queue: &mut VecDeque<FrequenciesAndPositions>,
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

    let window_size = left_buffer.len();
    let midpoint = window_size / 2;

    let mut frequencies_and_positions = FrequenciesAndPositions {
        left_frequences: left_buffer.to_vec(),
        right_frequences: right_buffer.to_vec(),
        right_to_lefts: vec![0.0; midpoint],
        phase_ratios: vec![0.0; midpoint],
    };

    fft_forward.process_with_scratch(
        &mut frequencies_and_positions.left_frequences,
        scratch_forward,
    );
    fft_forward.process_with_scratch(
        &mut frequencies_and_positions.right_frequences,
        scratch_forward,
    );

    for freq_ctr in 1..(midpoint + 1) {
        let mut left = frequencies_and_positions.left_frequences[freq_ctr];
        let mut right = frequencies_and_positions.right_frequences[freq_ctr];

        let samples_in_freq = (window_size / freq_ctr) as f32;

        // Fix negative amplitudes
        if left.re < 0.0 {
            left = invert_phase(left, samples_in_freq);
            frequencies_and_positions.left_frequences[freq_ctr] = left;
        }
        if right.re < 0.0 {
            right = invert_phase(right, samples_in_freq);
            frequencies_and_positions.right_frequences[freq_ctr] = right;
        }

        // Phase is offset from sine/cos in # of samples
        let samples_shifted_left = normalize_samples_shifted(left.im, samples_in_freq);
        let samples_shifted_right = normalize_samples_shifted(right.im, samples_in_freq);

        let samples_shifted_difference = (samples_shifted_left - samples_shifted_right).abs();

        // phase ratio: 0 is in phase, 1 is out of phase
        let phase_ratio = samples_shifted_difference / samples_in_freq;
        frequencies_and_positions.phase_ratios[freq_ctr - 1] = phase_ratio;

        let louder_amplitude = left.re.max(right.re);
        let amplitude_ratio = if louder_amplitude > 0.0 {
            left.re.min(right.re) / louder_amplitude
        } else {
            0.5
        };

        // Right to left measurements, 0 is right, 1 is left
        let right_to_left = if left.re > right.re {
            amplitude_ratio
        } else {
            1.0 - amplitude_ratio
        };
        frequencies_and_positions.right_to_lefts[freq_ctr - 1] = right_to_left;
    }

    frequences_and_positions_queue.push_back(frequencies_and_positions);

    while frequences_and_positions_queue.len() >= position_samples_length {
        // Copy transforms
        let frequencies_and_positions =
            &frequences_and_positions_queue[position_samples_length / 2];
        let mut left_front = frequencies_and_positions.left_frequences.to_vec();
        let mut right_front = frequencies_and_positions.right_frequences.to_vec();
        let mut left_rear = left_front.to_vec();
        let mut right_rear = right_front.to_vec();

        for freq_ctr in 1..(midpoint + 1) {
            let phase_ratio_sum: f32 = frequences_and_positions_queue
                .iter()
                .map(|f| f.phase_ratios[freq_ctr - 1])
                .sum();
            let phase_ratio_rear = phase_ratio_sum / (frequences_and_positions_queue.len() as f32);
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
        target_wav_writer.write_sample(
            sample_ctr,
            0,
            scale * left_front[sample_ctr_in_buffer].re,
        )?;
        target_wav_writer.write_sample(
            sample_ctr,
            1,
            scale * right_front[sample_ctr_in_buffer].re,
        )?;
        target_wav_writer.write_sample(
            sample_ctr,
            2,
            scale * left_rear[sample_ctr_in_buffer].re,
        )?;
        target_wav_writer.write_sample(
            sample_ctr,
            3,
            scale * right_rear[sample_ctr_in_buffer].re,
        )?;

        frequences_and_positions_queue.pop_front();
    }

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

fn invert_phase(c: Complex<f32>, samples_in_freq: f32) -> Complex<f32> {
    let mut im = c.im - samples_in_freq;
    if im < 0.0 {
        im = im + samples_in_freq;
    }

    Complex {
        re: c.re * -1.0,
        im,
    }
}
