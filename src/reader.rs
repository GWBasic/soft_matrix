use std::{
    collections::VecDeque,
    io::Result,
    sync::{Arc, Mutex},
};

use rustfft::{num_complex::Complex, Fft};
use wave_stream::wave_reader::{StreamWavReader, StreamWavReaderIterator};

use crate::{
    options::Options,
    structs::{ThreadState, TransformedWindowAndPans},
};

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
    mono_buffer: VecDeque<Complex<f32>>,
}

impl Reader {
    pub fn open(
        options: &Options,
        stream_wav_reader: StreamWavReader<f32>,
        window_size: usize,
        fft_forward: Arc<dyn Fft<f32>>,
    ) -> Result<Reader> {
        let mut open_wav_reader_and_buffer = OpenWavReaderAndBuffer {
            stream_wav_reader_iterator: stream_wav_reader.into_iter(),
            total_samples_read: window_size - 1,
            left_buffer: VecDeque::with_capacity(window_size),
            right_buffer: VecDeque::with_capacity(window_size),
            mono_buffer: VecDeque::with_capacity(window_size),
        };

        for _sample_to_read in 0..(window_size - 1) {
            open_wav_reader_and_buffer.queue_next_sample(options)?;
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
        let mut mono_transformed: Option<Vec<Complex<f32>>>;
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

            open_wav_reader_and_buffer.queue_next_sample(&thread_state.upmixer.options)?;

            // Read queues are copied so that there are windows for running FFTs
            // (At one point I had each thread read the entire window from the wav reader. That was much
            // slower and caused lock contention)
            left_transformed = Vec::from(open_wav_reader_and_buffer.left_buffer.make_contiguous());
            right_transformed =
                Vec::from(open_wav_reader_and_buffer.right_buffer.make_contiguous());

            // After the window is read, pop the unneeded samples (for the next read)
            open_wav_reader_and_buffer.left_buffer.pop_front();
            open_wav_reader_and_buffer.right_buffer.pop_front();

            // The middle transform is only processed if the middle channel is needed
            if thread_state.upmixer.options.transform_mono {
                mono_transformed = Some(Vec::from(
                    open_wav_reader_and_buffer.mono_buffer.make_contiguous(),
                ));
                open_wav_reader_and_buffer.mono_buffer.pop_front();
            } else {
                mono_transformed = None;
            }
        }

        self.fft_forward
            .process_with_scratch(&mut left_transformed, &mut thread_state.scratch_forward);
        self.fft_forward
            .process_with_scratch(&mut right_transformed, &mut thread_state.scratch_forward);
        if thread_state.upmixer.options.transform_mono {
            let mut mono_transformed_value =
                mono_transformed.expect("mono_transform never initialized");
            self.fft_forward.process_with_scratch(
                &mut mono_transformed_value,
                &mut thread_state.scratch_forward,
            );
            mono_transformed = Some(mono_transformed_value);
        }

        let mut frequency_pans = Vec::with_capacity(thread_state.upmixer.window_midpoint);
        for freq_ctr in 1..(thread_state.upmixer.window_midpoint + 1) {
            // Phase ranges from -PI to +PI
            let (left_amplitude, mut left_phase) = left_transformed[freq_ctr].to_polar();
            let (right_amplitude, mut right_phase) = right_transformed[freq_ctr].to_polar();

            if left_amplitude < thread_state.upmixer.options.minimum_steered_amplitude
                && right_amplitude >= thread_state.upmixer.options.minimum_steered_amplitude
            {
                left_phase = right_phase;
            } else if left_amplitude >= thread_state.upmixer.options.minimum_steered_amplitude
                && right_amplitude < thread_state.upmixer.options.minimum_steered_amplitude
            {
                right_phase = left_phase
            }

            // Uncomment to set breakpoints
            /*if last_sample_ctr == 17640 && freq_ctr == 46 {
                print!("");
            }*/

            frequency_pans.push(thread_state.upmixer.options.matrix.steer(
                left_amplitude,
                left_phase,
                right_amplitude,
                right_phase,
            ));
        }

        let transformed_window_and_pans = TransformedWindowAndPans {
            last_sample_ctr,
            left_transformed: Some(left_transformed),
            right_transformed: Some(right_transformed),
            mono_transformed,
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
    fn queue_next_sample(&mut self, options: &Options) -> Result<()> {
        match self.stream_wav_reader_iterator.next() {
            Some(samples_result) => {
                let samples = samples_result?;

                let front_left = samples.front_left.expect("front_left missing when reading");
                let front_right = samples
                    .front_right
                    .expect("front_right missing when reading");

                self.left_buffer.push_back(Complex {
                    re: front_left,
                    im: 0.0f32,
                });

                self.right_buffer.push_back(Complex {
                    re: front_right,
                    im: 0.0f32,
                });

                if options.transform_mono {
                    self.mono_buffer.push_back(Complex {
                        re: (front_left + front_right) / 2.0,
                        im: 0.0f32,
                    });
                }
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

                if options.transform_mono {
                    self.mono_buffer.push_back(Complex {
                        re: 0.0f32,
                        im: 0.0f32,
                    });
                }
            }
        }
        Ok(())
    }
}
