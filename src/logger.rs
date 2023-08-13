use std::{
    io::{stdout, Result, Write},
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::structs::ThreadState;

pub struct Logger {
    total_samples_to_write_f64: f64,
    logging_state: Mutex<LoggingState>,
}

pub struct LoggingState {
    pub started: Instant,
    pub next_log: Instant,
    pub logging_frequency: Duration,
}

impl Logger {
    pub fn new(logging_frequency: Duration, total_samples_to_write: usize) -> Logger {
        let now = Instant::now();

        Logger {
            total_samples_to_write_f64: total_samples_to_write as f64,
            logging_state: Mutex::new(LoggingState {
                started: now,
                next_log: now,
                logging_frequency: logging_frequency,
            }),
        }
    }

    pub fn log_status(self: &Logger, thread_state: &ThreadState) -> Result<()> {
        let mut logging_state = match self.logging_state.try_lock() {
            Ok(logging_state) => logging_state,
            _ => return Ok(()),
        };

        // Log current progess
        let now = Instant::now();
        if now >= logging_state.next_log {
            let elapsed_seconds = (now - logging_state.started).as_secs_f64();

            let total_samples_read = thread_state.upmixer.reader.get_total_samples_read();
            let total_samples_written = thread_state
                .upmixer
                .panner_and_writer
                .get_total_samples_written();

            let fraction_read = (total_samples_read as f64) / self.total_samples_to_write_f64;
            let fraction_written = (total_samples_written as f64) / self.total_samples_to_write_f64;

            let fraction_complete = (fraction_read + fraction_written) / 2.0;
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

            let logging_frequency = logging_state.logging_frequency;
            logging_state.next_log += logging_frequency;
        }

        return Ok(());
    }

    pub fn finish_logging(self: &Logger) -> Result<()> {
        let logging_state = self
            .logging_state
            .lock()
            .expect("Logging state broken on another thread");

        let now = Instant::now();
        let elapsed_seconds = (now - logging_state.started).as_secs_f64();

        let mut stdout = stdout();
        stdout.write(
            format!(
                "\rTotal time to complete: {:.0} seconds                                                             ",
                elapsed_seconds,
            )
            .as_bytes(),
        )?;
        stdout.flush()?;

        println!();

        Ok(())
    }
}
