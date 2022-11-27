use std::collections::VecDeque;
use std::io::{Read, Result, Seek};
use std::sync::Arc;

use rustfft::Fft;
use rustfft::{num_complex::Complex, FftPlanner};
use wave_stream::open_wav::OpenWav;
use wave_stream::wave_reader::{OpenWavReader, RandomAccessOpenWavReader, RandomAccessWavReader};
use wave_stream::wave_writer::{OpenWavWriter, RandomAccessWavWriter};

use crate::structs::UpmixedWindow;
use crate::window_sizes::get_ideal_window_size;

pub fn upmix<TReader: 'static + Read + Seek>(
    source_wav_reader: OpenWavReader<TReader>,
    target_wav_writer: OpenWavWriter,
) -> Result<()> {
    let min_window_size = source_wav_reader.sample_rate() / 40; // TODO: This really should be 10, or even 5, but that's super-slow
    let window_size = get_ideal_window_size(min_window_size as usize)?;

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

    let mut upmixed_queue = VecDeque::<UpmixedWindow>::new();
    pad_upmixed_queue(window_size, &mut upmixed_queue);

    let read_offset = (window_size / 2) as u32;
    for sample_ctr in 0..source_wav_reader.info().len_samples() as i32 {
        upmix_sample(
            &mut source_wav_reader,
            &fft_forward,
            &fft_inverse,
            &mut left_buffer,
            &mut right_buffer,
            &mut scratch_forward,
            &mut scratch_inverse,
            sample_ctr,
            read_offset,
            &mut upmixed_queue
        )?;

        write_samples_from_upmixed_queue(&mut upmixed_queue, window_size, &mut target_wav_writer, scale)?;
    }

    pad_upmixed_queue(window_size, &mut upmixed_queue);
    write_samples_from_upmixed_queue(&mut upmixed_queue, window_size, &mut target_wav_writer, scale)?;

    target_wav_writer.flush()?;

    Ok(())
}

fn pad_upmixed_queue(window_size: usize, upmixed_queue: &mut VecDeque<UpmixedWindow>) {
    for sample_ctr in (-1 * (window_size / 2) as i32)..0 {
        upmixed_queue.push_front(
            UpmixedWindow {
                sample_ctr,
                left_front: vec![Complex {re: 0f32, im: 0f32}; window_size],
                right_front: vec![Complex {re: 0f32, im: 0f32}; window_size],
                left_rear: vec![Complex {re: 0f32, im: 0f32}; window_size],
                right_rear: vec![Complex {re: 0f32, im: 0f32}; window_size]
            }
        )
    }
}

fn upmix_sample(
    source_wav_reader: &mut RandomAccessWavReader<f32>,
    fft_forward: &Arc<dyn Fft<f32>>,
    fft_inverse: &Arc<dyn Fft<f32>>,
    left_buffer: &mut Vec<Complex<f32>>,
    right_buffer: &mut Vec<Complex<f32>>,
    scratch_forward: &mut Vec<Complex<f32>>,
    scratch_inverse: &mut Vec<Complex<f32>>,
    sample_ctr: i32,
    read_offset: u32,
    upmixed_queue: &mut VecDeque<UpmixedWindow>
) -> Result<()> {
    left_buffer.remove(0);
    right_buffer.remove(0);

    let sample_to_read = (sample_ctr as u32) + read_offset;

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

    // Ultra-lows are not shitfted
    left_rear[0] = Complex { re: 0f32, im: 0f32 };
    right_rear[0] = Complex { re: 0f32, im: 0f32 };

    let window_size = left_buffer.len();
    let midpoint = window_size / 2;
    for freq_ctr in 1..(midpoint + 1) {
        // Phase is offset from sine/cos in # of samples
        let mut left = left_front[freq_ctr];
        let mut right = right_front[freq_ctr];

        let samples_in_freq = (window_size / freq_ctr) as f32;

        // Fix negative amplitudes
        if left.re < 0.0 {
            left = invert_phase(left, samples_in_freq);
        }
        if right.re < 0.0 {
            right = invert_phase(right, samples_in_freq);
        }

        // Phase is offset from sine/cos in # of samples
        let samples_shifted_left = normalize_samples_shifted(left.im, samples_in_freq);
        let samples_shifted_right = normalize_samples_shifted(right.im, samples_in_freq);

        let samples_shifted_difference = (samples_shifted_left - samples_shifted_right).abs();

        // phase ratio: 0 is in phase, 1 is out of phase
        let phase_ratio_rear = samples_shifted_difference / samples_in_freq;
        let phase_ratio_front = 1f32 - phase_ratio_rear;

        let mut left_front_component = left;
        let mut left_rear_component = left;
        let mut right_front_component = right;
        let mut right_rear_component = right;

        // Shift balance to front or rear
        left_front_component.re *= phase_ratio_front;
        right_front_component.re *= phase_ratio_front;
        left_rear_component.re *= phase_ratio_rear;
        right_rear_component.re *= phase_ratio_rear;

        // Assign to array
        left_front[freq_ctr] = left_front_component;
        right_front[freq_ctr] = right_front_component;
        left_rear[freq_ctr] = left_rear_component;
        right_rear[freq_ctr] = right_rear_component;

        if freq_ctr < midpoint {
            let inverse_freq_ctr = window_size - freq_ctr;
            left_front[inverse_freq_ctr] = Complex {
                re: left_front_component.re,
                im: left_front_component.im * -1f32,
            };
            right_front[inverse_freq_ctr] = Complex {
                re: right_front_component.re,
                im: right_front_component.im * -1f32,
            };
            left_rear[inverse_freq_ctr] = Complex {
                re: left_rear_component.re,
                im: left_rear_component.im * -1f32,
            };
            right_rear[inverse_freq_ctr] = Complex {
                re: right_rear_component.re,
                im: right_rear_component.im * -1f32,
            };
        }
    }

    fft_inverse.process_with_scratch(&mut left_front, scratch_inverse);
    fft_inverse.process_with_scratch(&mut right_front, scratch_inverse);
    fft_inverse.process_with_scratch(&mut left_rear, scratch_inverse);
    fft_inverse.process_with_scratch(&mut right_rear, scratch_inverse);

    upmixed_queue.push_back(UpmixedWindow {
        sample_ctr,
        left_front,
        right_front,
        left_rear,
        right_rear
    });

    Ok(())
}

fn write_samples_from_upmixed_queue(
    upmixed_queue: &mut VecDeque<UpmixedWindow>,
    window_size: usize,target_wav_writer:
    &mut RandomAccessWavWriter<f32>, scale: f32) -> Result<()> {
    while upmixed_queue.len() >= window_size {
        let mut left_front_sample = 0f32;
        let mut right_front_sample = 0f32;
        let mut left_rear_sample = 0f32;
        let mut right_rear_sample = 0f32;

        for queue_ctr in 0..window_size {
            let upmixed_window = &upmixed_queue[queue_ctr];
            left_front_sample += upmixed_window.left_front[queue_ctr].re;
            right_front_sample += upmixed_window.right_front[queue_ctr].re;
            left_rear_sample += upmixed_window.left_rear[queue_ctr].re;
            right_rear_sample += upmixed_window.right_rear[queue_ctr].re;
        }

        let sample_ctr_to_write = upmixed_queue[window_size / 2].sample_ctr as u32; 

        let window_size_f32 = window_size as f32;
        left_front_sample /= window_size_f32;
        right_front_sample /= window_size_f32;
        left_rear_sample /= window_size_f32;
        right_rear_sample /= window_size_f32;

        target_wav_writer.write_sample(sample_ctr_to_write, 0, scale * left_front_sample)?;
        target_wav_writer.write_sample(sample_ctr_to_write, 1, scale * right_front_sample)?;
        target_wav_writer.write_sample(sample_ctr_to_write, 2, scale * left_rear_sample)?;
        target_wav_writer.write_sample(sample_ctr_to_write, 3, scale * right_rear_sample)?;

        upmixed_queue.pop_front();
    }

    Ok(())
}

fn invert_phase(c: Complex<f32>, samples_in_freq: f32) -> Complex<f32> {
    let mut im = c.im - samples_in_freq;
    if im < 0.0 {
        im = im + samples_in_freq;
    }

    Complex {
        re: c.re * -1.0,
        im
    }
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
