use std::collections::VecDeque;
use std::f32::consts::{PI, TAU};
use std::io::{Read, Result, Seek};
use std::sync::{Arc, Mutex};

use atomic_counter::{AtomicCounter, ConsistentCounter};
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

    // rustfft states that the scale is 1/len()
    // See "noramlization": https://docs.rs/rustfft/latest/rustfft/#normalization
    // However, going back and forth to polar coordinates appears to make this very quiet, so I swapped
    // to 2 / ...
    let scale: f32 = 2.0 / (window_size as f32);

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

    let atomic_counter = ConsistentCounter::new(0);
    let mut source_wav_reader = Mutex::new(source_wav_reader);
    // TODO: Make this run on separate threads
    run_upmix_thread(
        &mut source_wav_reader,
        &atomic_counter,
        &mut upmixed_queue,
        window_size,
        &mut target_wav_writer,
        scale,
    )?;

    pad_upmixed_queue(window_size, &mut upmixed_queue);
    write_samples_from_upmixed_queue(
        &mut upmixed_queue,
        window_size,
        &mut target_wav_writer,
        scale,
    )?;

    target_wav_writer.flush()?;

    Ok(())
}

fn pad_upmixed_queue(window_size: usize, upmixed_queue: &mut VecDeque<UpmixedWindow>) {
    for sample_ctr in (-1 * (window_size / 2) as i32)..0 {
        upmixed_queue.push_front(UpmixedWindow {
            sample_ctr,
            left_front: vec![Complex { re: 0f32, im: 0f32 }; window_size],
            right_front: vec![Complex { re: 0f32, im: 0f32 }; window_size],
            left_rear: vec![Complex { re: 0f32, im: 0f32 }; window_size],
            right_rear: vec![Complex { re: 0f32, im: 0f32 }; window_size],
        })
    }
}

fn run_upmix_thread<TAtomicCounter: AtomicCounter<PrimitiveType = usize>>(
    source_wav_reader: &mut Mutex<RandomAccessWavReader<f32>>,
    atomic_counter: &TAtomicCounter,
    upmixed_queue: &mut VecDeque<UpmixedWindow>,
    window_size: usize,
    target_wav_writer: &mut RandomAccessWavWriter<f32>,
    scale: f32,
) -> Result<()> {
    let mut planner = FftPlanner::new();
    let fft_forward = planner.plan_fft_forward(window_size);
    let fft_inverse = planner.plan_fft_inverse(window_size);

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

    let len_samples = source_wav_reader.lock().unwrap()
        .info()
        .len_samples() as usize;

    loop {
        let sample_ctr = atomic_counter.inc();
        if sample_ctr >= len_samples {
            return Ok(());
        }

        let upmixed_window = upmix_sample(
            source_wav_reader,
            &fft_forward,
            &fft_inverse,
            &mut scratch_forward,
            &mut scratch_inverse,
            sample_ctr as i32,
        )?;

        upmixed_queue.push_back(upmixed_window);

        write_samples_from_upmixed_queue(
            upmixed_queue,
            window_size,
            target_wav_writer,
            scale,
        )?;
    }
}

fn upmix_sample(
    source_wav_reader: &mut Mutex<RandomAccessWavReader<f32>>,
    fft_forward: &Arc<dyn Fft<f32>>,
    fft_inverse: &Arc<dyn Fft<f32>>,
    scratch_forward: &mut Vec<Complex<f32>>,
    scratch_inverse: &mut Vec<Complex<f32>>,
    sample_ctr: i32,
) -> Result<UpmixedWindow> {

    let mut left_front = Vec::with_capacity(fft_forward.len());
    let mut right_front = Vec::with_capacity(fft_forward.len());

    {
        let mut source_wav_reader = source_wav_reader.lock().unwrap();

        let source_len = source_wav_reader.info().len_samples() as i32;

        let half_window_size = (fft_forward.len() / 2) as i32;
        let sample_to_start = sample_ctr - half_window_size;
        let sample_to_end = sample_ctr + half_window_size;
        for sample_to_read in sample_to_start..sample_to_end {
            if sample_to_read < 0 || sample_to_read >= source_len {
                left_front.push(Complex {
                    re: 0.0f32,
                    im: 0.0f32,
                });
                right_front.push(Complex {
                    re: 0.0f32,
                    im: 0.0f32,
                });
            } else {
                let sample_to_read = sample_to_read as u32;

                let left_sample = source_wav_reader.read_sample(sample_to_read, 0)?;
                left_front.push(Complex {
                    re: left_sample,
                    im: 0.0f32,
                });
        
                let right_sample = source_wav_reader.read_sample(sample_to_read, 1)?;
                right_front.push(Complex {
                    re: right_sample,
                    im: 0.0f32,
                });
            }    
        }
    }

    fft_forward.process_with_scratch(&mut left_front, scratch_forward);
    fft_forward.process_with_scratch(&mut right_front, scratch_forward);
    // Rear channels start as copies of the front channels
    let mut left_rear = left_front.to_vec();
    let mut right_rear = right_front.to_vec();

    // Ultra-lows are not shitfted
    left_rear[0] = Complex { re: 0f32, im: 0f32 };
    right_rear[0] = Complex { re: 0f32, im: 0f32 };

    let window_size = left_front.len();
    //let window_size_f32 = window_size as f32;
    let midpoint = window_size / 2;
    for freq_ctr in 1..(midpoint + 1) {
        // Phase is offset from sine/cos in # of samples
        let left = left_front[freq_ctr];
        let (left_amplitude, left_phase) = left.to_polar();
        //let left_phase_abs = left_phase.abs();
        let right = right_front[freq_ctr];
        let (right_amplitude, right_phase) = right.to_polar();
        //let right_phase_abs = right_phase.abs();

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
        let phase_ratio_rear = phase_difference_pi / PI;
        let phase_ratio_front = 1f32 - phase_ratio_rear;

        // Figure out the amplitudes for front and rear
        let left_front_amplitude = left_amplitude * phase_ratio_front;
        let right_front_amplitude = right_amplitude * phase_ratio_front;
        let left_rear_amplitude = left_amplitude * phase_ratio_rear;
        let right_rear_amplitude = right_amplitude * phase_ratio_rear;

        // Assign to array
        left_front[freq_ctr] = Complex::from_polar(left_front_amplitude, left_phase);
        right_front[freq_ctr] = Complex::from_polar(right_front_amplitude, right_phase);
        left_rear[freq_ctr] = Complex::from_polar(left_rear_amplitude, left_phase);
        right_rear[freq_ctr] = Complex::from_polar(right_rear_amplitude, right_phase);

        if freq_ctr < midpoint {
            let inverse_freq_ctr = window_size - freq_ctr;
            left_front[inverse_freq_ctr] = left_front[freq_ctr];
            right_front[inverse_freq_ctr] = right_front[freq_ctr];
            left_rear[inverse_freq_ctr] = left_rear[freq_ctr];
            right_rear[inverse_freq_ctr] = right_rear[freq_ctr];
        }
    }

    fft_inverse.process_with_scratch(&mut left_front, scratch_inverse);
    fft_inverse.process_with_scratch(&mut right_front, scratch_inverse);
    fft_inverse.process_with_scratch(&mut left_rear, scratch_inverse);
    fft_inverse.process_with_scratch(&mut right_rear, scratch_inverse);

    return Ok(UpmixedWindow {
        sample_ctr,
        left_front,
        right_front,
        left_rear,
        right_rear,
    });
}

/*
fn invert_phase(c: Complex<f32>, window_size: f32) -> Complex<f32> {
    Complex {
        re: c.re * -1.0,
        im: c.im + (window_size / 2f32),
    }
}
*/

fn write_samples_from_upmixed_queue(
    upmixed_queue: &mut VecDeque<UpmixedWindow>,
    window_size: usize,
    target_wav_writer: &mut RandomAccessWavWriter<f32>,
    scale: f32,
) -> Result<()> {
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

/*
fn normalize_phase(mut samples_shifted: f32, window_size: f32) -> f32 {
    while samples_shifted < 0f32 {
        samples_shifted += window_size;
    }

    while samples_shifted > window_size {
        samples_shifted -= window_size;
    }

    samples_shifted
}
*/
