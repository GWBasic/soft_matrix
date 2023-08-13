use std::{
    collections::VecDeque,
    f32::consts::PI,
    io::Result,
    sync::{Arc, Mutex},
};

pub const LFE_START: f32 = 40.0;
const LFE_FULL: f32 = 20.0;
const HALF_PI: f32 = PI / 2.0;

use rustfft::{num_complex::Complex, Fft};
use wave_stream::{samples_by_channel::SamplesByChannel, wave_writer::RandomAccessWavWriter};

use crate::{
    options::Options,
    structs::{ThreadState, TransformedWindowAndPans},
    upmixer::Upmixer,
};

pub struct PannerAndWriter {
    // A queue of transformed windows and all of the panned locations of each frequency, after averaging
    transformed_window_and_averaged_pans_queue: Mutex<VecDeque<TransformedWindowAndPans>>,

    // Wav writer and state used to communicate status
    writer_state: Mutex<WriterState>,

    fft_inverse: Arc<dyn Fft<f32>>,

    lfe_levels: Option<Vec<f32>>,
}

// Wraps types used during writing so they can be within a mutex
struct WriterState {
    pub target_wav_writer: RandomAccessWavWriter<f32>,
    pub total_samples_written: usize,
}

impl PannerAndWriter {
    pub fn new(
        options: &Options,
        window_size: usize,
        sample_rate: usize,
        target_wav_writer: RandomAccessWavWriter<f32>,
        fft_inverse: Arc<dyn Fft<f32>>,
    ) -> PannerAndWriter {
        let lfe_levels = if options.channels.low_frequency {
            let mut lfe_levels = vec![0.0f32; window_size];
            let window_midpoint = window_size / 2;

            let sample_rate_f32 = sample_rate as f32;
            let window_size_f32 = window_size as f32;

            lfe_levels[0] = 1.0;
            lfe_levels[window_midpoint] = 0.0;

            // Calculate ranges for averaging each sub frequency
            for transform_index in 1..(window_midpoint - 2) {
                let transform_index_f32 = transform_index as f32;
                // Out of 8
                // 1, 2, 3, 4
                // 8, 4, 2, 1
                let wavelength = window_size_f32 / transform_index_f32;
                let frequency = sample_rate_f32 / wavelength;

                let level = if frequency < LFE_FULL {
                    1.0
                } else if frequency < LFE_START {
                    let frequency_fraction = (frequency - LFE_FULL) / LFE_FULL;
                    (frequency_fraction * HALF_PI).cos()
                } else {
                    0.0
                };

                lfe_levels[transform_index] = level;
                lfe_levels[window_size - transform_index] = level;
            }

            Some(lfe_levels)
        } else {
            None
        };

        PannerAndWriter {
            transformed_window_and_averaged_pans_queue: Mutex::new(VecDeque::new()),
            writer_state: Mutex::new(WriterState {
                target_wav_writer,
                total_samples_written: 0,
            }),
            fft_inverse,
            lfe_levels,
        }
    }

    pub fn get_inplace_scratch_len(self: &PannerAndWriter) -> usize {
        self.fft_inverse.get_inplace_scratch_len()
    }

    pub fn get_total_samples_written(self: &PannerAndWriter) -> usize {
        self.writer_state
            .lock()
            .expect("Cannot aquire lock because a thread panicked")
            .total_samples_written
    }

    pub fn enqueue(self: &PannerAndWriter, transformed_window_and_pans: TransformedWindowAndPans) {
        self.transformed_window_and_averaged_pans_queue
            .lock()
            .expect("Cannot aquire lock because a thread panicked")
            .push_back(transformed_window_and_pans);
    }

    pub fn perform_backwards_transform_and_write_samples(
        self: &PannerAndWriter,
        thread_state: &mut ThreadState,
    ) -> Result<()> {
        'transform_and_write: loop {
            let transformed_window_and_pans = {
                let mut transformed_window_and_averaged_pans_queue = self
                    .transformed_window_and_averaged_pans_queue
                    .lock()
                    .expect("Cannot aquire lock because a thread panicked");

                match transformed_window_and_averaged_pans_queue.pop_front() {
                    Some(transformed_window_and_pans) => transformed_window_and_pans,
                    None => {
                        break 'transform_and_write;
                    }
                }
            };

            // The front channels are based on the original transforms
            let mut left_front = transformed_window_and_pans
                .left_transformed
                .expect("Transform expected, got a placeholder instead");
            let mut right_front = transformed_window_and_pans
                .right_transformed
                .expect("Transform expected, got a placeholder instead");

            // Rear channels start as copies of the front channels
            let mut left_rear = left_front.clone();
            let mut right_rear = right_front.clone();

            let lfe = if thread_state.upmixer.options.channels.low_frequency {
                transformed_window_and_pans.mono_transformed.clone()
            } else {
                None
            };

            let mut center = if thread_state.upmixer.options.channels.front_center {
                transformed_window_and_pans.mono_transformed
            } else {
                None
            };

            // Ultra-lows are not shitfted
            left_rear[0] = Complex { re: 0f32, im: 0f32 };
            right_rear[0] = Complex { re: 0f32, im: 0f32 };

            // Steer each frequency
            for freq_ctr in 1..(thread_state.upmixer.window_midpoint + 1) {
                // Phase is offset from sine/cos in # of samples
                let left = left_front[freq_ctr];
                let (left_amplitude, mut left_front_phase) = left.to_polar();
                let right = right_front[freq_ctr];
                let (right_amplitude, mut right_front_phase) = right.to_polar();

                let mut left_rear_phase = left_front_phase;
                let mut right_rear_phase = right_front_phase;

                let frequency_pans = &transformed_window_and_pans.frequency_pans[freq_ctr - 1];
                let left_to_right = frequency_pans.left_to_right;
                let back_to_front = frequency_pans.back_to_front;

                // Widening is currently disabled because it results in poor audio quality, and favors too
                // much steering to the rear
                //thread_state.upmixer.options.matrix.widen(&mut back_to_front, &mut left_to_right);

                let front_to_back = 1f32 - back_to_front;

                // Figure out the amplitudes for front and rear
                let mut left_front_amplitude = left_amplitude * front_to_back;
                let mut right_front_amplitude = right_amplitude * front_to_back;
                let left_rear_amplitude = left_amplitude * back_to_front;
                let right_rear_amplitude = right_amplitude * back_to_front;

                // Steer center
                center = match center {
                    Some(mut center) => {
                        let (_, phase) = center[freq_ctr].to_polar();
                        let center_amplitude = (1.0 - left_to_right.abs())
                            * (left_front_amplitude + right_front_amplitude);
                        let c = Complex::from_polar(center_amplitude, phase);

                        center[freq_ctr] = c;
                        if freq_ctr < thread_state.upmixer.window_midpoint {
                            center[thread_state.upmixer.window_size - freq_ctr] = Complex {
                                re: c.re,
                                im: -1.0 * c.im,
                            }
                        }

                        // Subtract the center from the right and left front channels
                        left_front_amplitude =
                            f32::max(0.0, left_front_amplitude - center_amplitude);
                        right_front_amplitude =
                            f32::max(0.0, right_front_amplitude - center_amplitude);

                        Some(center)
                    }
                    None => None,
                };

                // Phase shifts
                thread_state.upmixer.options.matrix.phase_shift(
                    &mut left_front_phase,
                    &mut right_front_phase,
                    &mut left_rear_phase,
                    &mut right_rear_phase,
                );

                // Assign to array
                left_front[freq_ctr] = Complex::from_polar(left_front_amplitude, left_front_phase);
                right_front[freq_ctr] =
                    Complex::from_polar(right_front_amplitude, right_front_phase);
                left_rear[freq_ctr] = Complex::from_polar(left_rear_amplitude, left_rear_phase);
                right_rear[freq_ctr] = Complex::from_polar(right_rear_amplitude, right_rear_phase);

                if freq_ctr < thread_state.upmixer.window_midpoint {
                    let inverse_freq_ctr = thread_state.upmixer.window_size - freq_ctr;
                    left_front[inverse_freq_ctr] = Complex {
                        re: left_front[freq_ctr].re,
                        im: -1.0 * left_front[freq_ctr].im,
                    };
                    right_front[inverse_freq_ctr] = Complex {
                        re: right_front[freq_ctr].re,
                        im: -1.0 * right_front[freq_ctr].im,
                    };
                    left_rear[inverse_freq_ctr] = Complex {
                        re: left_rear[freq_ctr].re,
                        im: -1.0 * left_rear[freq_ctr].im,
                    };
                    right_rear[inverse_freq_ctr] = Complex {
                        re: right_rear[freq_ctr].re,
                        im: -1.0 * right_rear[freq_ctr].im,
                    };
                }
            }

            self.fft_inverse
                .process_with_scratch(&mut left_front, &mut thread_state.scratch_inverse);
            self.fft_inverse
                .process_with_scratch(&mut right_front, &mut thread_state.scratch_inverse);
            self.fft_inverse
                .process_with_scratch(&mut left_rear, &mut thread_state.scratch_inverse);
            self.fft_inverse
                .process_with_scratch(&mut right_rear, &mut thread_state.scratch_inverse);

            center = match center {
                Some(mut center) => {
                    self.fft_inverse
                        .process_with_scratch(&mut center, &mut thread_state.scratch_inverse);

                    Some(center)
                }
                None => None,
            };

            // Filter LFE
            let lfe = match lfe {
                Some(mut lfe) => {
                    let lfe_levels = self.lfe_levels.as_ref().expect("lfe_levels not set");

                    for window_ctr in 1..thread_state.upmixer.window_midpoint {
                        let (amplitude, phase) = lfe[window_ctr].to_polar();
                        let c = Complex::from_polar(amplitude * lfe_levels[window_ctr], phase);

                        lfe[window_ctr] = c;
                        lfe[thread_state.upmixer.window_size - window_ctr] = Complex {
                            re: c.re,
                            im: -1.0 * c.im,
                        }
                    }

                    self.fft_inverse
                        .process_with_scratch(&mut lfe, &mut thread_state.scratch_inverse);

                    Some(lfe)
                }
                None => None,
            };

            let sample_ctr =
                transformed_window_and_pans.last_sample_ctr - thread_state.upmixer.window_midpoint;

            if sample_ctr == thread_state.upmixer.window_midpoint {
                // Special case for the beginning of the file
                for sample_ctr in 0..sample_ctr {
                    self.write_samples_in_window(
                        &thread_state.upmixer,
                        sample_ctr,
                        sample_ctr as usize,
                        &left_front,
                        &right_front,
                        &left_rear,
                        &right_rear,
                        &lfe,
                        &center,
                    )?;
                }
            } else if transformed_window_and_pans.last_sample_ctr
                == thread_state.upmixer.total_samples_to_write - 1
            {
                // Special case for the end of the file
                let first_sample_in_transform =
                    thread_state.upmixer.total_samples_to_write - thread_state.upmixer.window_size;
                for sample_in_transform in
                    thread_state.upmixer.window_midpoint..thread_state.upmixer.window_size
                {
                    self.write_samples_in_window(
                        &thread_state.upmixer,
                        first_sample_in_transform + sample_in_transform,
                        sample_in_transform as usize,
                        &left_front,
                        &right_front,
                        &left_rear,
                        &right_rear,
                        &lfe,
                        &center,
                    )?;
                }
            } else {
                self.write_samples_in_window(
                    &thread_state.upmixer,
                    sample_ctr,
                    thread_state.upmixer.window_midpoint,
                    &left_front,
                    &right_front,
                    &left_rear,
                    &right_rear,
                    &lfe,
                    &center,
                )?;
            }

            thread_state.upmixer.logger.log_status(thread_state)?;
        }

        Ok(())
    }

    fn write_samples_in_window(
        self: &PannerAndWriter,
        upmixer: &Upmixer,
        sample_ctr: usize,
        sample_in_transform: usize,
        left_front: &Vec<Complex<f32>>,
        right_front: &Vec<Complex<f32>>,
        left_rear: &Vec<Complex<f32>>,
        right_rear: &Vec<Complex<f32>>,
        lfe: &Option<Vec<Complex<f32>>>,
        center: &Option<Vec<Complex<f32>>>,
    ) -> Result<()> {
        let mut writer_state = self
            .writer_state
            .lock()
            .expect("Cannot aquire lock because a thread panicked");

        let mut left_front_sample = left_front[sample_in_transform].re;
        let mut right_front_sample = right_front[sample_in_transform].re;
        let mut left_rear_sample = left_rear[sample_in_transform].re;
        let mut right_rear_sample = right_rear[sample_in_transform].re;

        let mut lfe_sample = match lfe {
            Some(lfe) => Some(lfe[sample_in_transform].re),
            None => None,
        };

        let mut center_sample = match center {
            Some(center) => Some(center[sample_in_transform].re),
            None => None,
        };

        upmixer.options.matrix.adjust_levels(
            &mut left_front_sample,
            &mut right_front_sample,
            &mut left_rear_sample,
            &mut right_rear_sample,
            &mut lfe_sample,
            &mut center_sample,
        );

        let mut samples_by_channel = SamplesByChannel::new()
            .front_left(upmixer.scale * left_front_sample)
            .front_right(upmixer.scale * right_front_sample)
            .back_left(upmixer.scale * left_rear_sample)
            .back_right(upmixer.scale * right_rear_sample);

        match lfe_sample {
            Some(lfe_sample) => {
                samples_by_channel = samples_by_channel.low_frequency(upmixer.scale * lfe_sample);
            }
            None => {}
        }

        match center_sample {
            Some(center_sample) => {
                samples_by_channel = samples_by_channel.front_center(upmixer.scale * center_sample);
            }
            None => {}
        }

        writer_state
            .target_wav_writer
            .write_samples(sample_ctr, samples_by_channel)?;

        writer_state.total_samples_written += 1;

        Ok(())
    }
}

// Perform final flush implicitly
impl Drop for PannerAndWriter {
    fn drop(&mut self) {
        self.writer_state
            .lock()
            .expect("Cannot aquire lock because a thread panicked")
            .target_wav_writer
            .flush()
            .expect("Can not flush writer");
    }
}
