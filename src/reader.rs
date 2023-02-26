use std::{
    collections::VecDeque,
    f32::consts::{PI, TAU},
    io::Result,
    sync::{Arc, Mutex},
};

use rustfft::{num_complex::Complex, Fft};
use wave_stream::wave_reader::{StreamWavReader, StreamWavReaderIterator};

use crate::structs::{FrequencyPans, ThreadState, TransformedWindowAndPans};

pub struct Reader {
    open_wav_reader_and_buffer: Mutex<OpenWavReaderAndBuffer>,
    fft_forward: Arc<dyn Fft<f32>>,
}

// Allows wrapping information about reading the wav into a single mutex
struct OpenWavReaderAndBuffer {
    stream_wav_reader_iterator: StreamWavReaderIterator<f32>,
    total_samples_read: usize,
    left_buffer: VecDeque<Complex<f32>>,
    right_buffer: VecDeque<Complex<f32>>,
}

impl Reader {
    pub fn open(
        stream_wav_reader: StreamWavReader<f32>,
        window_size: usize,
        fft_forward: Arc<dyn Fft<f32>>,
    ) -> Result<Reader> {
        let mut open_wav_reader_and_buffer = OpenWavReaderAndBuffer {
            stream_wav_reader_iterator: stream_wav_reader.into_iter(),
            total_samples_read: window_size - 1,
            left_buffer: VecDeque::with_capacity(window_size),
            right_buffer: VecDeque::with_capacity(window_size),
        };

        for _sample_to_read in 0..(window_size - 1) {
            open_wav_reader_and_buffer.queue_next_sample()?;
        }

        Ok(Reader {
            open_wav_reader_and_buffer: Mutex::new(open_wav_reader_and_buffer),
            fft_forward,
        })
    }

    pub fn get_inplace_scratch_len(self: &Reader) -> usize {
        self.fft_forward.get_inplace_scratch_len()
    }

    pub fn read_transform_and_measure_pans(
        self: &Reader,
        thread_state: &mut ThreadState,
    ) -> Result<Option<TransformedWindowAndPans>> {
        let mut left_transformed: Vec<Complex<f32>>;
        let mut right_transformed: Vec<Complex<f32>>;
        let last_sample_ctr: usize;
        {
            let mut open_wav_reader_and_buffer = self
                .open_wav_reader_and_buffer
                .lock()
                .expect("Cannot aquire lock because a thread panicked");

            last_sample_ctr = open_wav_reader_and_buffer.total_samples_read;
            if last_sample_ctr >= thread_state.upmixer.total_samples_to_write {
                return Ok(None);
            } else {
                open_wav_reader_and_buffer.total_samples_read += 1;
            }

            open_wav_reader_and_buffer.queue_next_sample()?;

            // Read queues are copied so that there are windows for running FFTs
            // (At one point I had each thread read the entire window from the wav reader. That was much
            // slower and caused lock contention)
            left_transformed = Vec::from(open_wav_reader_and_buffer.left_buffer.make_contiguous());
            right_transformed =
                Vec::from(open_wav_reader_and_buffer.right_buffer.make_contiguous());

            // After the window is read, pop the unneeded samples (for the next read)
            open_wav_reader_and_buffer.left_buffer.pop_front();
            open_wav_reader_and_buffer.right_buffer.pop_front();
        }

        self.fft_forward
            .process_with_scratch(&mut left_transformed, &mut thread_state.scratch_forward);
        self.fft_forward
            .process_with_scratch(&mut right_transformed, &mut thread_state.scratch_forward);

        //let upmixer = self.upmixer.upgrade_and_unwrap();

        let mut frequency_pans = Vec::with_capacity(thread_state.upmixer.window_midpoint);
        for freq_ctr in 1..(thread_state.upmixer.window_midpoint + 1) {
            // Phase is offset from sine/cos in # of samples
            let left = left_transformed[freq_ctr];
            let (_left_amplitude, left_phase) = left.to_polar();
            let right = right_transformed[freq_ctr];
            let (_right_amplitude, right_phase) = right.to_polar();

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
            let back_to_front = phase_difference_pi / PI;

            frequency_pans.push(FrequencyPans { back_to_front });
        }

        let transformed_window_and_pans = TransformedWindowAndPans {
            last_sample_ctr,
            left_transformed: Some(left_transformed),
            right_transformed: Some(right_transformed),
            frequency_pans,
        };

        return Ok(Some(transformed_window_and_pans));
    }

    pub fn get_total_samples_read(&self) -> usize {
        self.open_wav_reader_and_buffer
            .lock()
            .expect("Cannot aquire lock because a thread panicked")
            .total_samples_read
    }
}

impl OpenWavReaderAndBuffer {
    fn queue_next_sample(&mut self) -> Result<()> {
        match self.stream_wav_reader_iterator.next() {
            Some(samples_result) => {
                let samples = samples_result?;

                self.left_buffer.push_back(Complex {
                    re: samples[0],
                    im: 0.0f32,
                });

                self.right_buffer.push_back(Complex {
                    re: samples[1],
                    im: 0.0f32,
                });
            }
            None => {
                // The read buffer needs to be padded with empty samples, this way there is a full window to
                // run an fft on the end of the wav

                // TODO: Is this really needed? Probably should just abort if the file is shorter than the window length
                // (Or just make the window length the entire length of the file?)
                // https://github.com/GWBasic/soft_matrix/issues/24

                self.left_buffer.push_back(Complex {
                    re: 0.0f32,
                    im: 0.0f32,
                });
                self.right_buffer.push_back(Complex {
                    re: 0.0f32,
                    im: 0.0f32,
                });
            }
        }
        Ok(())
    }
}
