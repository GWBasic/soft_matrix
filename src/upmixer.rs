use std::collections::{HashMap, VecDeque};
use std::io::{stdout, Read, Result, Seek, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::available_parallelism;
use std::time::Duration;

use rustfft::{num_complex::Complex, FftPlanner};
use wave_stream::open_wav::OpenWav;
use wave_stream::wave_reader::{OpenWavReader, RandomAccessOpenWavReader};
use wave_stream::wave_writer::OpenWavWriter;

use crate::logger::Logger;
use crate::panner_and_writer::PannerAndWriter;
use crate::reader::Reader;
use crate::structs::{
    EnqueueAndAverageState, FrequencyPans, ThreadState, TransformedWindowAndPans,
};
use crate::window_sizes::get_ideal_window_size;

pub struct Upmixer {
    pub window_size: usize,
    pub window_midpoint: usize,
    pub total_samples_to_write: usize,
    pub scale: f32,

    // Handles periodic logging to the console
    pub logger: Logger,

    // Reads from the source wav file, keeps a queue of samples, groups samples into windows
    pub reader: Reader,

    // Performs final panning within a transform, transforms backwards, and writes the results to the wav file
    pub panner_and_writer: PannerAndWriter,

    // TODO: Remove after this
    // =====================

    // Temporary location for transformed windows and pans so that they can be finished out-of-order
    transformed_window_and_pans_by_sample: Mutex<HashMap<usize, TransformedWindowAndPans>>,

    // State enqueueing and averaging
    enqueue_and_average_state: Mutex<EnqueueAndAverageState>,
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

    let total_samples_to_write = source_wav_reader.info().len_samples();

    // Calculate ranges for averaging each sub frequency
    let mut average_last_sample_ctr_lower_bounds = Vec::with_capacity(window_midpoint - 1);
    let mut average_last_sample_ctr_upper_bounds = Vec::with_capacity(window_midpoint - 1);
    let mut pan_fraction_per_frequencys = Vec::with_capacity(window_midpoint - 1);
    for sub_freq_ctr in 0..window_midpoint {
        // Out of 8
        // 1, 2, 3, 4
        let transform_index = sub_freq_ctr + 1;
        // 8, 4, 2, 1
        let wavelength = window_size / transform_index;

        let extra_samples = window_size - wavelength;

        let average_last_sample_ctr_lower_bound = extra_samples / 2;
        let average_last_sample_ctr_upper_bound =
            average_last_sample_ctr_lower_bound + wavelength - 1;
        let pan_fraction_per_frequency = 1.0 / (wavelength as f32);

        average_last_sample_ctr_lower_bounds.push(average_last_sample_ctr_lower_bound);
        average_last_sample_ctr_upper_bounds.push(average_last_sample_ctr_upper_bound);
        pan_fraction_per_frequencys.push(pan_fraction_per_frequency);
    }

    let mut planner = FftPlanner::new();
    let fft_forward = planner.plan_fft_forward(window_size);
    let fft_inverse = planner.plan_fft_inverse(window_size);

    let upmixer = Arc::new(Upmixer {
        total_samples_to_write,
        window_size,
        window_midpoint,
        scale,
        logger: Logger::new(Duration::from_secs_f32(1.0 / 10.0), total_samples_to_write),
        reader: Reader::open(source_wav_reader, window_size, fft_forward)?,
        panner_and_writer: PannerAndWriter::new(target_wav_writer, fft_inverse),
        transformed_window_and_pans_by_sample: Mutex::new(HashMap::new()),
        enqueue_and_average_state: Mutex::new(EnqueueAndAverageState {
            average_last_sample_ctr_lower_bounds,
            average_last_sample_ctr_upper_bounds,
            pan_fraction_per_frequencys,
            next_last_sample_ctr_to_enqueue: window_size - 1,
            transformed_window_and_pans_queue: VecDeque::new(),
            pan_averages: Vec::with_capacity(window_size - 1),
            complete: false,
        }),
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

    Ok(())
}

impl Upmixer {
    // Runs the upmix thread. Aborts the process if there is an error
    fn run_upmix_thread(self: &Arc<Upmixer>) {
        match self.run_upmix_thread_int() {
            Err(error) => {
                println!("Error upmixing: {:?}", error);
                std::process::exit(-1);
            }
            _ => {}
        }
    }

    fn run_upmix_thread_int(self: &Arc<Upmixer>) -> Result<()> {
        // Each thread has a separate FFT scratch space
        let scratch_forward = vec![
            Complex {
                re: 0.0f32,
                im: 0.0f32
            };
            self.reader.get_inplace_scratch_len()
        ];
        let scratch_inverse = vec![
            Complex {
                re: 0.0f32,
                im: 0.0f32
            };
            self.panner_and_writer.get_inplace_scratch_len()
        ];

        let mut thread_state = ThreadState {
            upmixer: self.clone(),
            scratch_forward,
            scratch_inverse,
        };

        // Initial log
        self.logger.log_status(&thread_state)?;

        'upmix_each_sample: loop {
            let transformed_window_and_pans_option =
                self.reader.read_transform_and_measure_pans(&mut thread_state)?;

            self.logger.log_status(&thread_state)?;

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
            self.panner_and_writer
                .perform_backwards_transform_and_write_samples(&mut thread_state)?;

            self.logger.log_status(&thread_state)?;

            if end_loop {
                // If the upmixed wav isn't completely written, we're probably stuck in averaging
                // Block on whatever thread is averaging
                let enqueue_and_average_state = self
                    .enqueue_and_average_state
                    .lock()
                    .expect("Cannot aquire lock because a thread panicked");

                if enqueue_and_average_state.complete {
                    break 'upmix_each_sample;
                }
            }
        }

        Ok(())
    }

    // Enqueues the transformed_window_and_pans and averages pans if possible
    fn enqueue_and_average(self: &Upmixer) {
        // The thread that can lock self.transformed_window_and_pans_queue will keep writing samples are long as there
        // are samples to write
        // All other threads will skip this logic and continue performing FFTs while a thread has this lock
        let mut enqueue_and_average_state = match self.enqueue_and_average_state.try_lock() {
            Ok(enqueue_and_average_state) => enqueue_and_average_state,
            _ => return,
        };

        // Get all transformed windows in order
        {
            let mut transformed_window_and_pans_by_sample = self
                .transformed_window_and_pans_by_sample
                .lock()
                .expect("Cannot aquire lock because a thread panicked");

            'enqueue: loop {
                match transformed_window_and_pans_by_sample
                    .remove(&enqueue_and_average_state.next_last_sample_ctr_to_enqueue)
                {
                    Some(mut last_transformed_window_and_pans) => {
                        // Special case: First transform
                        // Pre-seed multiple copies of the first transform for averaging
                        if enqueue_and_average_state.next_last_sample_ctr_to_enqueue
                            == self.window_size - 1
                        {
                            while enqueue_and_average_state
                                .transformed_window_and_pans_queue
                                .len()
                                < self.window_midpoint - 1
                            {
                                enqueue_and_average_state
                                    .transformed_window_and_pans_queue
                                    .push_back(TransformedWindowAndPans {
                                        last_sample_ctr: 0,
                                        // The first transforms will never be used
                                        left_transformed: None,
                                        right_transformed: None,
                                        frequency_pans: last_transformed_window_and_pans
                                            .frequency_pans
                                            .clone(),
                                    });
                            }
                        }

                        // Special case: Last transform
                        // Seed multiple copies at the end so the last part of the file is written
                        if enqueue_and_average_state.next_last_sample_ctr_to_enqueue
                            == self.total_samples_to_write - 1
                        {
                            for _ in 0..self.window_midpoint {
                                let next_last_transformed_window_and_pans =
                                    TransformedWindowAndPans {
                                        last_sample_ctr: last_transformed_window_and_pans
                                            .last_sample_ctr
                                            + 1,
                                        left_transformed: None,
                                        right_transformed: None,
                                        frequency_pans: last_transformed_window_and_pans
                                            .frequency_pans
                                            .clone(),
                                    };

                                enqueue_and_average_state
                                    .transformed_window_and_pans_queue
                                    .push_back(last_transformed_window_and_pans);

                                last_transformed_window_and_pans =
                                    next_last_transformed_window_and_pans;
                            }

                            enqueue_and_average_state.complete = true;
                        }

                        enqueue_and_average_state
                            .transformed_window_and_pans_queue
                            .push_back(last_transformed_window_and_pans);

                        // Special case: Pre-seed averages
                        if enqueue_and_average_state.next_last_sample_ctr_to_enqueue
                            == self.window_size + self.window_midpoint
                        {
                            for freq_ctr in 0..self.window_midpoint {
                                let mut average_back_to_front = 0.0;
                                for sample_ctr in enqueue_and_average_state
                                    .average_last_sample_ctr_lower_bounds[freq_ctr]
                                    ..(enqueue_and_average_state
                                        .average_last_sample_ctr_upper_bounds[freq_ctr]
                                        - 1)
                                {
                                    let back_to_front = enqueue_and_average_state
                                        .transformed_window_and_pans_queue[sample_ctr]
                                        .frequency_pans[freq_ctr]
                                        .back_to_front;
                                    let fraction_per_frequency = enqueue_and_average_state
                                        .pan_fraction_per_frequencys[freq_ctr];
                                    average_back_to_front += back_to_front * fraction_per_frequency;
                                }

                                enqueue_and_average_state.pan_averages.push(FrequencyPans {
                                    back_to_front: average_back_to_front,
                                });
                            }
                        }

                        enqueue_and_average_state.next_last_sample_ctr_to_enqueue += 1;
                    }
                    None => break 'enqueue,
                };
            }
        }

        // Gaurd against no averaging
        if enqueue_and_average_state.pan_averages.len() == 0 {
            return;
        }

        // Calculate averages and enqueue for final transforms and writing
        while enqueue_and_average_state
            .transformed_window_and_pans_queue
            .len()
            >= self.window_size
        {
            // Add newly-added pans (in the queue) to the averages
            for freq_ctr in 0..self.window_midpoint {
                let sample_ctr =
                    enqueue_and_average_state.average_last_sample_ctr_upper_bounds[freq_ctr];
                let adjust_back_to_front = enqueue_and_average_state
                    .transformed_window_and_pans_queue[sample_ctr]
                    .frequency_pans[freq_ctr]
                    .back_to_front
                    * enqueue_and_average_state.pan_fraction_per_frequencys[freq_ctr];
                enqueue_and_average_state.pan_averages[freq_ctr].back_to_front +=
                    adjust_back_to_front;
            }

            // enqueue the averaged transformed window and pans
            let transformed_window_and_pans = enqueue_and_average_state
                .transformed_window_and_pans_queue
                .get_mut(self.window_midpoint)
                .unwrap();

            let last_transform =
                transformed_window_and_pans.last_sample_ctr == self.total_samples_to_write - 1;

            self.panner_and_writer.enqueue(TransformedWindowAndPans {
                last_sample_ctr: transformed_window_and_pans.last_sample_ctr,
                left_transformed: transformed_window_and_pans.left_transformed.take(),
                right_transformed: transformed_window_and_pans.right_transformed.take(),
                frequency_pans: enqueue_and_average_state.pan_averages.clone(),
            });

            // Special case to stop averaging
            if last_transform {
                enqueue_and_average_state
                    .transformed_window_and_pans_queue
                    .clear();
                return;
            }

            // Remove the unneeded pans
            for freq_ctr in 0..self.window_midpoint {
                let sample_ctr =
                    enqueue_and_average_state.average_last_sample_ctr_lower_bounds[freq_ctr];
                let adjust_back_to_front = enqueue_and_average_state
                    .transformed_window_and_pans_queue[sample_ctr]
                    .frequency_pans[freq_ctr]
                    .back_to_front
                    * enqueue_and_average_state.pan_fraction_per_frequencys[freq_ctr];
                enqueue_and_average_state.pan_averages[freq_ctr].back_to_front -=
                    adjust_back_to_front;
            }

            // dequeue
            enqueue_and_average_state
                .transformed_window_and_pans_queue
                .pop_front();
        }
    }
}
