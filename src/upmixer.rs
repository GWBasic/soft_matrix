use std::io::{stdout, Error, ErrorKind, Read, Result, Seek, Write};
use std::sync::Arc;
use std::thread;
use std::thread::available_parallelism;
use std::time::Duration;

use rustfft::{num_complex::Complex, FftPlanner};
use wave_stream::open_wav::OpenWav;
use wave_stream::wave_reader::{OpenWavReader, StreamOpenWavReader};
use wave_stream::wave_writer::OpenWavWriter;

use crate::logger::Logger;
use crate::options::Options;
use crate::panner_and_writer::PannerAndWriter;
use crate::panning_averager::PanningAverager;
use crate::reader::Reader;
use crate::structs::ThreadState;
use crate::window_sizes::get_ideal_window_size;

pub struct Upmixer {
    pub options: Options,
    pub window_size: usize,
    pub window_midpoint: usize,
    pub total_samples_to_write: usize,
    pub scale: f32,

    // Handles periodic logging to the console
    pub logger: Logger,

    // Reads from the source wav file, keeps a queue of samples, groups samples into windows
    pub reader: Reader,

    // Averages panning locations to keep the sound smooth
    pub panning_averager: PanningAverager,

    // Performs final panning within a transform, transforms backwards, and writes the results to the wav file
    pub panner_and_writer: PannerAndWriter,
}

unsafe impl Send for Upmixer {}
unsafe impl Sync for Upmixer {}

pub fn upmix<TReader: 'static + Read + Seek>(
    options: Options,
    source_wav_reader: OpenWavReader<TReader>,
    target_wav_writer: OpenWavWriter,
) -> Result<()> {
    let max_low_frequency = (source_wav_reader.sample_rate() / 8) as f32;
    if options.low_frequency >= max_low_frequency {
        let error = format!(
            "Lowest steered frequency {}hz is too high. Maximum lowest frequency for {} samples / second is {}",
            options.low_frequency,
            source_wav_reader.sample_rate(),
            max_low_frequency);
        return Err(Error::new(ErrorKind::InvalidInput, error));
    }

    let min_window_size = ((source_wav_reader.sample_rate() as f32) / options.low_frequency).ceil() as usize;
    let mut window_size = get_ideal_window_size(min_window_size)?;

    println!(
        "Lowest frequency: {}hz. With input at {} samples / second, using an optimized window size of {} samples",
        options.low_frequency,
        source_wav_reader.sample_rate(),
        window_size);

    if source_wav_reader.len_samples() < window_size {
        window_size = min_window_size;
    }

    if source_wav_reader.len_samples() < window_size {        
        let error = format!(
            "Input is too short, {} samples; minimum window size {} samples. Consider raising the lowest frequency via -low {}",
            source_wav_reader.len_samples(),
            min_window_size,
            (source_wav_reader.sample_rate() as usize / source_wav_reader.len_samples()) + 1);
        return Err(Error::new(ErrorKind::InvalidInput, error));
    }

    let mut stdout = stdout();
    stdout.write(format!("Starting...").as_bytes())?;
    stdout.flush()?;

    let source_wav_reader = source_wav_reader.get_stream_f32_reader()?;
    let target_wav_writer = target_wav_writer.get_random_access_f32_writer()?;

    // rustfft states that the scale is 1/len()
    // See "noramlization": https://docs.rs/rustfft/latest/rustfft/#normalization
    let scale: f32 = 1.0 / (window_size as f32);

    let window_midpoint = window_size / 2;

    let total_samples_to_write = source_wav_reader.info().len_samples();
    let sample_rate = source_wav_reader.info().sample_rate() as usize;

    let mut planner = FftPlanner::new();
    let fft_forward = planner.plan_fft_forward(window_size);
    let fft_inverse = planner.plan_fft_inverse(window_size);

    let reader = Reader::open(&options, source_wav_reader, window_size, fft_forward)?;
    let panner_and_writer = PannerAndWriter::new(
        &options,
        window_size,
        sample_rate,
        target_wav_writer,
        fft_inverse,
    );

    let upmixer = Arc::new(Upmixer {
        options,
        total_samples_to_write,
        window_size,
        window_midpoint,
        scale,
        logger: Logger::new(Duration::from_secs_f32(1.0 / 10.0), total_samples_to_write),
        reader,
        panning_averager: PanningAverager::new(window_size),
        panner_and_writer,
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
            let transformed_window_and_pans_option = self
                .reader
                .read_transform_and_measure_pans(&mut thread_state)?;

            self.logger.log_status(&thread_state)?;

            // Break the loop if upmix_sample returned None
            let end_loop = match transformed_window_and_pans_option {
                Some(transformed_window_and_pans) => {
                    self.panning_averager
                        .enqueue_transformed_window_and_pans(transformed_window_and_pans);
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
            self.panning_averager.enqueue_and_average(&thread_state);
            self.panner_and_writer
                .perform_backwards_transform_and_write_samples(&mut thread_state)?;

            self.logger.log_status(&thread_state)?;

            if end_loop {
                // If the upmixed wav isn't completely written, we're probably stuck in averaging
                // Block on whatever thread is averaging
                if self.panning_averager.is_complete() {
                    break 'upmix_each_sample;
                }
            }
        }

        Ok(())
    }
}
