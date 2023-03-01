use std::{
    collections::VecDeque,
    io::Result,
    sync::{Arc, Mutex},
};

use rustfft::{num_complex::Complex, Fft};
use wave_stream::wave_writer::RandomAccessWavWriter;

use crate::{
    structs::{ThreadState, TransformedWindowAndPans},
    upmixer::Upmixer,
};

pub struct PannerAndWriter {
    // A queue of transformed windows and all of the panned locations of each frequency, after averaging
    transformed_window_and_averaged_pans_queue: Mutex<VecDeque<TransformedWindowAndPans>>,

    // Wav writer and state used to communicate status
    writer_state: Mutex<WriterState>,

    fft_inverse: Arc<dyn Fft<f32>>,
}

// Wraps types used during writing so they can be within a mutex
struct WriterState {
    pub target_wav_writer: RandomAccessWavWriter<f32>,
    pub total_samples_written: usize,
}

impl PannerAndWriter {
    pub fn new(
        target_wav_writer: RandomAccessWavWriter<f32>,
        fft_inverse: Arc<dyn Fft<f32>>,
    ) -> PannerAndWriter {
        PannerAndWriter {
            transformed_window_and_averaged_pans_queue: Mutex::new(VecDeque::new()),
            writer_state: Mutex::new(WriterState {
                target_wav_writer,
                total_samples_written: 0,
            }),
            fft_inverse,
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

                let back_to_front =
                    transformed_window_and_pans.frequency_pans[freq_ctr - 1].back_to_front;
                let front_to_back = 1f32 - back_to_front;

                // Figure out the amplitudes for front and rear
                let left_front_amplitude = left_amplitude * front_to_back;
                let right_front_amplitude = right_amplitude * front_to_back;
                let left_rear_amplitude = left_amplitude * back_to_front;
                let right_rear_amplitude = right_amplitude * back_to_front;

                // Phase shifts
                thread_state.upmixer.matrix.phase_shift(
                    &thread_state,
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
    ) -> Result<()> {
        let mut writer_state = self
            .writer_state
            .lock()
            .expect("Cannot aquire lock because a thread panicked");

        let left_front_sample = left_front[sample_in_transform].re;
        let right_front_sample = right_front[sample_in_transform].re;
        let left_rear_sample = left_rear[sample_in_transform].re;
        let right_rear_sample = right_rear[sample_in_transform].re;

        writer_state.target_wav_writer.write_sample(
            sample_ctr,
            upmixer.options.left_front_channel,
            upmixer.scale * left_front_sample,
        )?;
        writer_state.target_wav_writer.write_sample(
            sample_ctr,
            upmixer.options.right_front_channel,
            upmixer.scale * right_front_sample,
        )?;
        writer_state.target_wav_writer.write_sample(
            sample_ctr,
            upmixer.options.right_rear_channel,
            upmixer.scale * left_rear_sample,
        )?;
        writer_state.target_wav_writer.write_sample(
            sample_ctr,
            upmixer.options.right_rear_channel,
            upmixer.scale * right_rear_sample,
        )?;

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
