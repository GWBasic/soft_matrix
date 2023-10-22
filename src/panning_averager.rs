use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
};

use crate::structs::{FrequencyPans, ThreadState, TransformedWindowAndPans};

pub struct PanningAverager {
    // Temporary location for transformed windows and pans so that they can be finished out-of-order
    transformed_window_and_pans_by_sample: Mutex<HashMap<usize, TransformedWindowAndPans>>,

    // State enqueueing and averaging
    enqueue_and_average_state: Mutex<EnqueueAndAverageState>,
}

struct EnqueueAndAverageState {
    // Precalculated indexes and fractions used to calculate rolling averages of samples
    pub average_last_sample_ctr_lower_bounds: Vec<usize>,
    pub average_last_sample_ctr_upper_bounds: Vec<usize>,
    pub pan_fraction_per_frequencys: Vec<f32>,
    // Indexes of samples to average
    pub next_last_sample_ctr_to_enqueue: usize,
    // A queue of transformed windows and all of the panned locations of each frequency, before averaging
    pub transformed_window_and_pans_queue: VecDeque<TransformedWindowAndPans>,
    // The current average pans
    pub pan_averages: Vec<FrequencyPans>,
}

impl PanningAverager {
    pub fn new(window_size: usize) -> PanningAverager {
        let window_midpoint = window_size / 2;

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

        PanningAverager {
            transformed_window_and_pans_by_sample: Mutex::new(HashMap::new()),
            enqueue_and_average_state: Mutex::new(EnqueueAndAverageState {
                average_last_sample_ctr_lower_bounds,
                average_last_sample_ctr_upper_bounds,
                pan_fraction_per_frequencys,
                next_last_sample_ctr_to_enqueue: window_size - 1,
                transformed_window_and_pans_queue: VecDeque::new(),
                pan_averages: Vec::with_capacity(window_size - 1),
            }),
        }
    }

    pub fn enqueue_transformed_window_and_pans(
        &self,
        transformed_window_and_pans: TransformedWindowAndPans,
    ) {
        let mut transformed_window_and_pans_by_sample = self
            .transformed_window_and_pans_by_sample
            .lock()
            .expect("Cannot aquire lock because a thread panicked");

        transformed_window_and_pans_by_sample.insert(
            transformed_window_and_pans.last_sample_ctr,
            transformed_window_and_pans,
        );
    }

    // Enqueues the transformed_window_and_pans and averages pans if possible
    pub fn enqueue_and_average(&self, thread_state: &ThreadState) {
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
                            == thread_state.upmixer.window_size - 1
                        {
                            while enqueue_and_average_state
                                .transformed_window_and_pans_queue
                                .len()
                                < thread_state.upmixer.window_midpoint - 1
                            {
                                enqueue_and_average_state
                                    .transformed_window_and_pans_queue
                                    .push_back(TransformedWindowAndPans {
                                        last_sample_ctr: 0,
                                        // The first transforms will never be used
                                        left_transformed: None,
                                        right_transformed: None,
                                        mono_transformed: None,
                                        frequency_pans: last_transformed_window_and_pans
                                            .frequency_pans
                                            .clone(),
                                    });
                            }
                        }

                        // Special case: Last transform
                        // Seed multiple copies at the end so the last part of the file is written
                        if enqueue_and_average_state.next_last_sample_ctr_to_enqueue
                            == thread_state.upmixer.total_samples_to_write - 1
                        {
                            for _ in 0..thread_state.upmixer.window_midpoint {
                                let next_last_transformed_window_and_pans =
                                    TransformedWindowAndPans {
                                        last_sample_ctr: last_transformed_window_and_pans
                                            .last_sample_ctr
                                            + 1,
                                        left_transformed: None,
                                        right_transformed: None,
                                        mono_transformed: None,
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
                        }

                        enqueue_and_average_state
                            .transformed_window_and_pans_queue
                            .push_back(last_transformed_window_and_pans);

                        // Special case: Pre-seed averages
                        if enqueue_and_average_state.next_last_sample_ctr_to_enqueue
                            == thread_state.upmixer.window_size
                                + thread_state.upmixer.window_midpoint
                        {
                            for freq_ctr in 0..thread_state.upmixer.window_midpoint {
                                let mut average_left_to_right = 0.0;
                                let mut average_back_to_front = 0.0;
                                for sample_ctr in enqueue_and_average_state
                                    .average_last_sample_ctr_lower_bounds[freq_ctr]
                                    ..(enqueue_and_average_state
                                        .average_last_sample_ctr_upper_bounds[freq_ctr]
                                        - 1)
                                {
                                    let fraction_per_frequency = enqueue_and_average_state
                                        .pan_fraction_per_frequencys[freq_ctr];

                                    let frequency_pans = &enqueue_and_average_state
                                        .transformed_window_and_pans_queue[sample_ctr]
                                        .frequency_pans[freq_ctr];

                                    average_left_to_right +=
                                        frequency_pans.left_to_right * fraction_per_frequency;
                                    average_back_to_front +=
                                        frequency_pans.back_to_front * fraction_per_frequency;
                                }

                                enqueue_and_average_state.pan_averages.push(FrequencyPans {
                                    left_to_right: average_left_to_right,
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
            >= thread_state.upmixer.window_size
        {
            // Add newly-added pans (in the queue) to the averages
            for freq_ctr in 0..thread_state.upmixer.window_midpoint {
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
                .get_mut(thread_state.upmixer.window_midpoint)
                .unwrap();

            let last_transform = transformed_window_and_pans.last_sample_ctr
                == thread_state.upmixer.total_samples_to_write - 1;

            thread_state
                .upmixer
                .panner_and_writer
                .enqueue(TransformedWindowAndPans {
                    last_sample_ctr: transformed_window_and_pans.last_sample_ctr,
                    left_transformed: transformed_window_and_pans.left_transformed.take(),
                    right_transformed: transformed_window_and_pans.right_transformed.take(),
                    mono_transformed: transformed_window_and_pans.mono_transformed.take(),
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
            for freq_ctr in 0..thread_state.upmixer.window_midpoint {
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
