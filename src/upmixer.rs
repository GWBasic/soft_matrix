use core::num;
use std::cell::Cell;
use std::collections::{HashMap, VecDeque};
use std::f32::consts::{PI, TAU};
use std::io::{stdout, Read, Result, Seek, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::available_parallelism;
use std::time::{Duration, Instant};

use rustfft::Fft;
use rustfft::{num_complex::Complex, FftPlanner};
use wave_stream::open_wav::OpenWav;
use wave_stream::wave_reader::{OpenWavReader, RandomAccessOpenWavReader};
use wave_stream::wave_writer::OpenWavWriter;

use crate::structs::{
    AveragedFrequencyPans, FrequencyPans, OpenWavReaderAndBuffer, TransformedWindowAndPans,
    WriterState,
};
use crate::window_sizes::get_ideal_window_size;

struct Upmixer {
    open_wav_reader_and_buffer: Mutex<OpenWavReaderAndBuffer>,
    window_size: usize,
    window_size_f32: f32,
    window_size_u32: u32,
    window_midpoint: usize,
    window_midpoint_u32: u32,
    scale: f32,
    total_samples_to_write: u32,

    // Temporary location for transformed windows and pans so that they can be finished out-of-order
    transformed_window_and_pans_by_sample: Mutex<HashMap<u32, TransformedWindowAndPans>>,

    // The next transformed window to average pans
    next_transformed_window_and_pans_cell: Mutex<Cell<u32>>,

    // Queue of AveragedFrequencyPans, used to keep track of ongoing averages
    //averaged_frequency_pans_queue: Mutex<VecDeque<AveragedFrequencyPans>>,

    // A queue of transformed windows and all of the panned locations of each frequency, after averaging
    transformed_window_and_averaged_pans_queue: Mutex<VecDeque<TransformedWindowAndPans>>,

    // Wav writer and state used to communicate status
    writer_state: Mutex<WriterState>,

    // Delete me (for debugging)
    delete_me_last_sample_queued: Cell<u32>,
}

unsafe impl Send for Upmixer {}
unsafe impl Sync for Upmixer {}

pub fn upmix<TReader: 'static + Read + Seek>(
    source_wav_reader: OpenWavReader<TReader>,
    target_wav_writer: OpenWavWriter,
) -> Result<()> {
    let mut stdout = stdout();
    stdout.write(format!("Starting...").as_bytes())?;
    stdout.flush()?;

    let min_window_size = source_wav_reader.sample_rate() / 10;
    let window_size = get_ideal_window_size(min_window_size as usize)?;

    let source_wav_reader = source_wav_reader.get_random_access_f32_reader()?;
    let target_wav_writer = target_wav_writer.get_random_access_f32_writer()?;

    // rustfft states that the scale is 1/len()
    // See "noramlization": https://docs.rs/rustfft/latest/rustfft/#normalization
    let scale: f32 = 1.0 / (window_size as f32);

    let window_midpoint = window_size / 2;

    let mut open_wav_reader_and_buffer = OpenWavReaderAndBuffer {
        source_wav_reader,
        next_read_sample: (window_size - 1) as u32,
        left_buffer: VecDeque::with_capacity(window_size),
        right_buffer: VecDeque::with_capacity(window_size),
    };

    for sample_to_read in 0..(window_size - 1) as u32 {
        read_samples(sample_to_read, &mut open_wav_reader_and_buffer)?;
    }

    let now = Instant::now();
    let total_samples_to_write = open_wav_reader_and_buffer
        .source_wav_reader
        .info()
        .len_samples() as f64;

    let upmixer = Arc::new(Upmixer {
        total_samples_to_write: open_wav_reader_and_buffer
            .source_wav_reader
            .info()
            .len_samples(),
        open_wav_reader_and_buffer: Mutex::new(open_wav_reader_and_buffer),
        window_size,
        window_size_f32: window_size as f32,
        window_size_u32: window_size as u32,
        window_midpoint,
        window_midpoint_u32: window_midpoint as u32,
        scale,
        transformed_window_and_pans_by_sample: Mutex::new(HashMap::new()),
        next_transformed_window_and_pans_cell: Mutex::new(Cell::new((window_size as u32) - 1)),
        //averaged_frequency_pans_queue: Mutex::new(VecDeque::new()),
        transformed_window_and_averaged_pans_queue: Mutex::new(VecDeque::new()),
        writer_state: Mutex::new(WriterState {
            target_wav_writer,
            started: now,
            next_log: now,
            total_samples_to_write,
        }),
        delete_me_last_sample_queued: Cell::new(0),
    });

    // Start threads
    let num_threads = available_parallelism()?.get();
    let mut join_handles = Vec::with_capacity(num_threads - 1);
    for _ in 1..num_threads {
        let upmixer_thread = upmixer.clone();
        let join_handle = thread::spawn(move || {
            upmixer_thread.run_upmix_thread();
        });

        join_handles.push(join_handle);
    }

    // Perform upmixing on this thread as well
    upmixer.run_upmix_thread();

    for join_handle in join_handles {
        // Note that threads will terminate the process if there is an unhandled error
        join_handle.join().expect("Could not join thread");
    }

    stdout.write(
        format!("\rFinishing...                                                                 ")
            .as_bytes(),
    )?;
    println!();
    stdout.flush()?;

    {
        let mut writer_state = upmixer
            .writer_state
            .lock()
            .expect("Cannot aquire lock because a thread panicked");

        writer_state.target_wav_writer.flush()?;
    }
    Ok(())
}

fn read_samples(
    sample_to_read: u32,
    open_wav_reader_and_buffer: &mut OpenWavReaderAndBuffer,
) -> Result<()> {
    let len_samples = open_wav_reader_and_buffer
        .source_wav_reader
        .info()
        .len_samples();

    if sample_to_read < len_samples {
        let left = open_wav_reader_and_buffer
            .source_wav_reader
            .read_sample(sample_to_read, 0)?;
        open_wav_reader_and_buffer.left_buffer.push_back(Complex {
            re: left,
            im: 0.0f32,
        });

        let right = open_wav_reader_and_buffer
            .source_wav_reader
            .read_sample(sample_to_read, 1)?;
        open_wav_reader_and_buffer.right_buffer.push_back(Complex {
            re: right,
            im: 0.0f32,
        });
    } else {
        // The read buffer needs to be padded with empty samples, this way there is a full window to
        // run an fft on the end of the wav

        // TODO: Is this really needed? Probably should just abort if the file is shorter than the window length
        // (Or just make the window length the entire length of the file?)
        // https://github.com/GWBasic/soft_matrix/issues/24

        open_wav_reader_and_buffer.left_buffer.push_back(Complex {
            re: 0.0f32,
            im: 0.0f32,
        });
        open_wav_reader_and_buffer.right_buffer.push_back(Complex {
            re: 0.0f32,
            im: 0.0f32,
        });
    }

    Ok(())
}

impl Upmixer {
    // Runs the upmix thread. Aborts the process if there is an error
    fn run_upmix_thread(self: &Upmixer) {
        match self.run_upmix_thread_int() {
            Err(error) => {
                println!("Error upmixing: {:?}", error);
                std::process::exit(-1);
            }
            _ => {}
        }
    }

    fn run_upmix_thread_int(self: &Upmixer) -> Result<()> {
        // Each thread has its own separate FFT calculator
        let mut planner = FftPlanner::new();
        let fft_forward = planner.plan_fft_forward(self.window_size);
        let fft_inverse = planner.plan_fft_inverse(self.window_size);

        // Each thread has a separate FFT scratch space
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

        'upmix_each_sample: loop {
            let transformed_window_and_pans_option =
                self.read_transform_and_measure_pans(&fft_forward, &mut scratch_forward)?;

            // Break the loop if upmix_sample returned None
            let end_loop = match transformed_window_and_pans_option {
                Some(transformed_window_and_pans) => {
                    let mut transformed_window_and_pans_by_sample = self
                        .transformed_window_and_pans_by_sample
                        .lock()
                        .expect("Cannot aquire lock because a thread panicked");

                    transformed_window_and_pans_by_sample.insert(
                        transformed_window_and_pans.last_sample_ctr,
                        transformed_window_and_pans,
                    );

                    false
                }
                None => true,
            };

            // If a lock can be aquired
            // - Enqueues completed transformed_window_and_pans
            // - Performs averaging
            //
            // The conditional lock is because these calculations require global state and can not be
            // performed in parallel
            //
            // It's possible that there are dangling samples on the queue
            // Because write_samples_from_upmixed_queue doesn't wait for the lock, this should be called
            // one more time to drain the queue of upmixed samples
            self.enqueue_and_average();
            self.perform_backwards_transform_and_write_samples(&fft_inverse, &mut scratch_inverse)?;

            if end_loop {
                break 'upmix_each_sample;
            }
        }

        Ok(())
    }

    fn read_transform_and_measure_pans(
        self: &Upmixer,
        fft_forward: &Arc<dyn Fft<f32>>,
        scratch_forward: &mut Vec<Complex<f32>>,
    ) -> Result<Option<TransformedWindowAndPans>> {
        let mut left_transformed: Vec<Complex<f32>>;
        let mut right_transformed: Vec<Complex<f32>>;
        let last_sample_ctr: u32;

        {
            let mut open_wav_reader_and_buffer = self
                .open_wav_reader_and_buffer
                .lock()
                .expect("Cannot aquire lock because a thread panicked");

            let source_len = open_wav_reader_and_buffer
                .source_wav_reader
                .info()
                .len_samples() as u32;

            last_sample_ctr = open_wav_reader_and_buffer.next_read_sample;
            if last_sample_ctr >= source_len {
                return Ok(None);
            } else {
                open_wav_reader_and_buffer.next_read_sample += 1;
            }

            read_samples(last_sample_ctr, &mut open_wav_reader_and_buffer)?;

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

        fft_forward.process_with_scratch(&mut left_transformed, scratch_forward);
        fft_forward.process_with_scratch(&mut right_transformed, scratch_forward);

        let mut frequency_pans = Vec::with_capacity(self.window_midpoint);
        for freq_ctr in 1..(self.window_midpoint + 1) {
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
            left_transformed,
            right_transformed,
            frequency_pans,
        };

        return Ok(Some(transformed_window_and_pans));
    }

    // Enqueues the transformed_window_and_pans and averages pans if possible
    // Returns true if the transformed_window_and_pans_queue is small and more reads should happen
    fn enqueue_and_average(self: &Upmixer) {
        // The thread that can lock self.next_transformed_window_and_pans will keep writing samples are long as there
        // are samples to write
        // All other threads will skip this logic and continue performing FFTs while a thread has this lock
        let mut next_transformed_window_and_pans_cell =
            match self.next_transformed_window_and_pans_cell.try_lock() {
                Ok(next_transformed_window_and_pans_cell) => next_transformed_window_and_pans_cell,
                _ => return,
            };

        'enqueue: loop {
            // Unlock transformed_window_and_pans_by_sample once per loop to allow other threads to fill it
            let mut transformed_window_and_pans_by_sample = self
                .transformed_window_and_pans_by_sample
                .lock()
                .expect("Cannot aquire lock because a thread panicked");

            // Make sure the transform that's next to average is present
            let last_sample_ctr = next_transformed_window_and_pans_cell.get();
            if !transformed_window_and_pans_by_sample.contains_key(&last_sample_ctr) {
                break 'enqueue;
            }

            // Make sure all needed transforms to average with are present
            let oldest_last_sample = last_sample_ctr - self.window_midpoint_u32;
            let newest_last_sample = last_sample_ctr + self.window_midpoint_u32;

            for check_sample in oldest_last_sample..newest_last_sample {
                if check_sample >= (self.window_size_u32 - 1) {
                    if check_sample < self.total_samples_to_write {
                        // TODO: Micro-Optimization: Place these into an array for more direct lookup
                        if !transformed_window_and_pans_by_sample.contains_key(&check_sample) {
                            break 'enqueue;
                        }
                    }
                }
            }

            // Average each sub frequency
            let mut frequency_pans = Vec::with_capacity(self.window_midpoint - 1);
            for sub_freq_ctr in 0..self.window_midpoint {
                // Out of 8
                // 1, 2, 3, 4
                let transform_index = (sub_freq_ctr as u32) + 1;
                // 8, 4, 2, 1
                let wavelength = self.window_size_u32 / transform_index;

                let oldest_last_sample = last_sample_ctr - (wavelength / 2);
                let newest_last_sample = last_sample_ctr + (wavelength / 2);
                let mut back_to_front = 0.0;
                let mut num_items = 0;
                for check_sample in oldest_last_sample..newest_last_sample {
                    if check_sample >= (self.window_size_u32 - 1) {
                        if check_sample < self.total_samples_to_write {
                            let other_transformed_window_and_pans =
                                transformed_window_and_pans_by_sample
                                    .get(&check_sample)
                                    .expect("Checked that transform was present, now it's not");

                            back_to_front += other_transformed_window_and_pans.frequency_pans
                                [sub_freq_ctr]
                                .back_to_front;
                            num_items += 1;
                        }
                    }
                }

                frequency_pans.push(FrequencyPans {
                    back_to_front: back_to_front / (num_items as f32),
                });
            }

            let transformed_window_and_pans = transformed_window_and_pans_by_sample
                .get(&last_sample_ctr)
                .expect("Checked that transform was present, now it's not");

            self.transformed_window_and_averaged_pans_queue
                .lock()
                .expect("Cannot aquire lock because a thread panicked")
                .push_back(TransformedWindowAndPans {
                    last_sample_ctr,
                    // TODO: Optimize by using a RefCell and swapping
                    // https://doc.rust-lang.org/std/cell/struct.RefCell.html#method.replace
                    left_transformed: transformed_window_and_pans.left_transformed.to_vec(),
                    right_transformed: transformed_window_and_pans.right_transformed.to_vec(),
                    frequency_pans,
                });

            // Remove the no-longer-needed set of transforms
            transformed_window_and_pans_by_sample.remove(&oldest_last_sample);

            next_transformed_window_and_pans_cell.set(last_sample_ctr + 1);
        }

        /*
        // Special case for first transform
        // Seed the first pans for a half a window
        if averaged_frequency_pans_queue.len() == 0 {
            match transformed_window_and_pans_by_sample.get(&((self.window_size - 1) as u32)) {
                Some(transformed_window_and_pans) => {
                    for last_sample_ctr in
                        self.window_midpoint..(self.window_size + self.window_midpoint)
                    {
                        averaged_frequency_pans_queue.push_back(AveragedFrequencyPans {
                            last_sample_ctr: last_sample_ctr as u32,
                            frequency_pans: transformed_window_and_pans.frequency_pans.to_vec(),
                            averaged_frequency_pans: transformed_window_and_pans
                                .frequency_pans
                                .to_vec(),
                        });
                    }
                }
                _ => {}
            };
        }

        // Add sequential transforms
        'enqueue: loop {
            match averaged_frequency_pans_queue.back() {
                Some(back_averaged_frequency_pans) => {
                    let added_last_sample_ctr = back_averaged_frequency_pans.last_sample_ctr + 1;
                    let apply_last_sample_ctr =
                        added_last_sample_ctr - (self.window_midpoint as u32);

                    let mut insert_transformed_window_and_pans = None;

                    // Doing the remove after the get keeps the borrow-checker happy
                    // (Otherwise, the get holds a mutable borrow on transformed_window_and_pans_by_sample)
                    let mut added_averaged_frequency_pans;
                    match transformed_window_and_pans_by_sample.get(&apply_last_sample_ctr) {
                        Some(_apply_transformed_window_and_pans) => {
                            match transformed_window_and_pans_by_sample.get(&added_last_sample_ctr)
                            {
                                Some(added_transformed_window_and_pans) => {
                                    // Special case for the last windows
                                    // The pans will be copied for the last few windows
                                    if added_last_sample_ctr
                                        >= self.total_samples_to_write
                                            - self.window_midpoint_u32
                                            - 1
                                        && added_last_sample_ctr
                                            < (self.total_samples_to_write
                                                + self.window_midpoint_u32)
                                    {
                                        insert_transformed_window_and_pans =
                                            Some(TransformedWindowAndPans {
                                                last_sample_ctr: added_last_sample_ctr + 1,
                                                left_transformed: added_transformed_window_and_pans
                                                    .left_transformed
                                                    .to_vec(),
                                                right_transformed:
                                                    added_transformed_window_and_pans
                                                        .right_transformed
                                                        .to_vec(),
                                                frequency_pans: added_transformed_window_and_pans
                                                    .frequency_pans
                                                    .to_vec(),
                                            });
                                    }

                                    // Calculate averages and enqueue
                                    let removed_averaged_frequency_pans =
                                        averaged_frequency_pans_queue.front().unwrap();

                                    // Rolling average is kept by copying the previous-calculated average
                                    added_averaged_frequency_pans = back_averaged_frequency_pans
                                        .averaged_frequency_pans
                                        .to_vec();

                                    for freq_ctr in 0..self.window_midpoint {
                                        added_averaged_frequency_pans[freq_ctr].back_to_front +=
                                            added_transformed_window_and_pans.frequency_pans
                                                [freq_ctr]
                                                .back_to_front
                                                / self.window_size_f32;
                                        added_averaged_frequency_pans[freq_ctr].back_to_front -=
                                            removed_averaged_frequency_pans.frequency_pans
                                                [freq_ctr]
                                                .back_to_front
                                                / self.window_size_f32;
                                    }

                                    averaged_frequency_pans_queue.push_back(
                                        AveragedFrequencyPans {
                                            last_sample_ctr: added_last_sample_ctr,
                                            frequency_pans: added_transformed_window_and_pans
                                                .frequency_pans
                                                .to_vec(),
                                            averaged_frequency_pans: added_averaged_frequency_pans
                                                .to_vec(),
                                        },
                                    );

                                    averaged_frequency_pans_queue.pop_front();
                                }
                                None => break 'enqueue,
                            };
                        }
                        None => break 'enqueue,
                    };

                    let mut apply_transformed_window_and_pans =
                        transformed_window_and_pans_by_sample
                            .remove(&apply_last_sample_ctr)
                            .unwrap();

                    apply_transformed_window_and_pans.frequency_pans =
                        added_averaged_frequency_pans;

                    let mut transformed_window_and_averaged_pans_queue = self
                        .transformed_window_and_averaged_pans_queue
                        .lock()
                        .expect("Cannot aquire lock because a thread panicked");

                    self.delete_me_last_sample_queued
                        .set(apply_transformed_window_and_pans.last_sample_ctr);

                    transformed_window_and_averaged_pans_queue
                        .push_back(apply_transformed_window_and_pans);

                    // Special case insert
                    match insert_transformed_window_and_pans {
                        Some(insert_transformed_window_and_pans) => {
                            transformed_window_and_pans_by_sample.insert(
                                insert_transformed_window_and_pans.last_sample_ctr,
                                insert_transformed_window_and_pans,
                            );
                        }
                        _ => {}
                    }
                }
                None => break 'enqueue,
            }
        }
        */
    }

    fn perform_backwards_transform_and_write_samples(
        self: &Upmixer,
        fft_inverse: &Arc<dyn Fft<f32>>,
        scratch_inverse: &mut Vec<Complex<f32>>,
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

            let mut left_front = transformed_window_and_pans.left_transformed;
            let mut right_front = transformed_window_and_pans.right_transformed;

            // Rear channels start as copies of the front channels
            let mut left_rear = left_front.to_vec();
            let mut right_rear = right_front.to_vec();

            // Ultra-lows are not shitfted
            left_rear[0] = Complex { re: 0f32, im: 0f32 };
            right_rear[0] = Complex { re: 0f32, im: 0f32 };

            // Steer each frequency
            for freq_ctr in 1..(self.window_midpoint + 1) {
                // Phase is offset from sine/cos in # of samples
                let left = left_front[freq_ctr];
                let (left_amplitude, left_phase) = left.to_polar();
                let right = right_front[freq_ctr];
                let (right_amplitude, right_phase) = right.to_polar();

                let back_to_front =
                    transformed_window_and_pans.frequency_pans[freq_ctr - 1].back_to_front;
                let front_to_back = 1f32 - back_to_front;

                // Figure out the amplitudes for front and rear
                let left_front_amplitude = left_amplitude * front_to_back;
                let right_front_amplitude = right_amplitude * front_to_back;
                let left_rear_amplitude = left_amplitude * back_to_front;
                let right_rear_amplitude = right_amplitude * back_to_front;

                // Assign to array
                left_front[freq_ctr] = Complex::from_polar(left_front_amplitude, left_phase);
                right_front[freq_ctr] = Complex::from_polar(right_front_amplitude, right_phase);
                left_rear[freq_ctr] = Complex::from_polar(left_rear_amplitude, left_phase);
                right_rear[freq_ctr] = Complex::from_polar(right_rear_amplitude, right_phase);

                if freq_ctr < self.window_midpoint {
                    let inverse_freq_ctr = self.window_size - freq_ctr;
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

            fft_inverse.process_with_scratch(&mut left_front, scratch_inverse);
            fft_inverse.process_with_scratch(&mut right_front, scratch_inverse);
            fft_inverse.process_with_scratch(&mut left_rear, scratch_inverse);
            fft_inverse.process_with_scratch(&mut right_rear, scratch_inverse);

            let sample_ctr = transformed_window_and_pans.last_sample_ctr - self.window_midpoint_u32;

            if sample_ctr == self.window_midpoint as u32 {
                // Special case for the beginning of the file
                for sample_ctr in 0..sample_ctr {
                    self.write_samples_in_window(
                        sample_ctr,
                        sample_ctr as usize,
                        &left_front,
                        &right_front,
                        &left_rear,
                        &right_rear,
                    )?;
                }
            } else if transformed_window_and_pans.last_sample_ctr == self.total_samples_to_write - 1
            {
                // Special case for the end of the file
                for sample_in_transform in (self.window_midpoint_u32 + 1)..self.window_size_u32 {
                    self.write_samples_in_window(
                        sample_in_transform - self.window_midpoint_u32 + 1 + sample_ctr + 1,
                        sample_in_transform as usize,
                        &left_front,
                        &right_front,
                        &left_rear,
                        &right_rear,
                    )?;
                }
            }

            self.write_samples_in_window(
                sample_ctr,
                self.window_midpoint,
                &left_front,
                &right_front,
                &left_rear,
                &right_rear,
            )?;
        }

        Ok(())
    }

    fn write_samples_in_window(
        self: &Upmixer,
        sample_ctr: u32,
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
            0,
            self.scale * left_front_sample,
        )?;
        writer_state.target_wav_writer.write_sample(
            sample_ctr,
            1,
            self.scale * right_front_sample,
        )?;
        writer_state.target_wav_writer.write_sample(
            sample_ctr,
            2,
            self.scale * left_rear_sample,
        )?;
        writer_state.target_wav_writer.write_sample(
            sample_ctr,
            3,
            self.scale * right_rear_sample,
        )?;

        // Log current progess
        let now = Instant::now();
        if now >= writer_state.next_log {
            let elapsed_seconds = (now - writer_state.started).as_secs_f64();
            let fraction_complete = (sample_ctr as f64) / writer_state.total_samples_to_write;
            let estimated_seconds = elapsed_seconds / fraction_complete;

            let mut stdout = stdout();
            stdout.write(
                format!(
                    "\rWriting: {:.2}% complete, {:.0} elapsed seconds, {:.2} estimated total seconds         ",
                    100.0 * fraction_complete,
                    elapsed_seconds,
                    estimated_seconds,
                )
                .as_bytes(),
            )?;
            stdout.flush()?;

            writer_state.next_log += Duration::from_secs(1);
        }

        Ok(())
    }
}
