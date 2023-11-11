use std::ffi::OsStr;
use std::path::Path;

use wave_stream::open_wav::OpenWav;
use wave_stream::wave_header::{Channels, SampleFormat, WavHeader};
use wave_stream::wave_writer::OpenWavWriter;
use wave_stream::{read_wav_from_file_path, write_wav_to_file_path};

mod logger;
mod matrix;
mod options;
mod panner_and_writer;
mod panning_averager;
mod reader;
mod structs;
mod upmixer;
mod window_sizes;

use upmixer::upmix;

use crate::options::Options;

fn main() {
    println!("Soft Matrix: Upmixes stereo wav files to surround");

    // See https://en.wikipedia.org/wiki/Matrix_decoder for information about all the different matrixes

    let options = match Options::parse() {
        Some(options) => options,
        None => {
            println!("See https://github.com/GWBasic/soft_matrix/blob/{}/options.md for more information about options", env!("GIT_HASH"));
            return;
        }
    };

    let open_source_wav_result = read_wav_from_file_path(&options.source_wav_path);

    let source_wav = match open_source_wav_result {
        Err(error) => {
            println!(
                "Can not open {}: {:?}",
                &options.source_wav_path.display(),
                error
            );
            return;
        }
        Ok(source_wav) => source_wav,
    };

    // Check that source is 2 channels
    let expected_channels = Channels::new().front_left().front_right();

    if source_wav.channels() != &expected_channels {
        println!(
            "Upmixing can only happen from a 2-channel wav. {} has {} channel(s). (Extended format wavs must specify front_left and front_right",
            &options.source_wav_path.display(),
            source_wav.num_channels()
        );

        return;
    }

    let header = WavHeader {
        sample_format: SampleFormat::Float,
        channels: options.channels.clone(),
        sample_rate: source_wav.sample_rate(),
    };

    // Wave files have a max size of 4GB. (Due to RIFF using 32 bits to track its size.) It's very easy to exceed this length
    // when upmixing a file over (approximately) 58 minutes in length. 6 channels @ 32 bits / sample (float) adds up quickly

    let max_samples_in_file = header.max_samples();
    let mut num_target_files = source_wav.len_samples() / max_samples_in_file;
    if source_wav.len_samples() % max_samples_in_file > 0 {
        num_target_files += 1;
    }

    let mut target_open_wav_writers: Vec<OpenWavWriter> = Vec::with_capacity(num_target_files);

    if num_target_files > 1 {
        // Need to update the path if there are multiple targets
        let file_stem = match options.target_wav_path.file_stem() {
            Some(file_stem) => file_stem,
            None => {
                println!(
                    "Not a valid filename: {}",
                    options.target_wav_path.display()
                );
                return;
            }
        };
        let extension = options
            .target_wav_path
            .extension()
            .unwrap_or(OsStr::new("wav"));
        let folder = options.target_wav_path.parent().unwrap_or(&Path::new("/"));

        for file_ctr in 1..(num_target_files + 1) {
            let target_wav_filename_string = format!(
                "{} - {} of {}.{}",
                file_stem.to_string_lossy(),
                file_ctr,
                num_target_files,
                extension.to_string_lossy()
            );

            let target_wav_path = folder.join(target_wav_filename_string);

            let open_target_wav_result = write_wav_to_file_path(&target_wav_path, header);

            let target_wav = match open_target_wav_result {
                Err(error) => {
                    println!("Can not open {}: {:?}", &target_wav_path.display(), error);
                    return;
                }
                Ok(target_wav) => target_wav,
            };

            target_open_wav_writers.push(target_wav)
        }
    } else {
        let open_target_wav_result = write_wav_to_file_path(&options.target_wav_path, header);

        let target_wav = match open_target_wav_result {
            Err(error) => {
                println!(
                    "Can not open {}: {:?}",
                    &options.target_wav_path.display(),
                    error
                );
                return;
            }
            Ok(target_wav) => target_wav,
        };

        target_open_wav_writers.push(target_wav)
    }

    let length_seconds = (source_wav.len_samples() as f64) / (source_wav.sample_rate() as f64);
    println!(
        "\tSource: {}, {} seconds long",
        &options.source_wav_path.display(),
        length_seconds
    );
    println!("\tTarget: {}", &options.target_wav_path.display());

    let mut _keepawake = if options.keep_awake {
        let reason = format!(
            "De-matrixing {} to {}",
            &options.source_wav_path.display(),
            &options.target_wav_path.display()
        );
        Some(
            keepawake::Builder::new()
                .display(false)
                .idle(true)
                .app_name("soft_matrix")
                .reason(reason)
                .app_reverse_domain("io.github.gwbasic.soft_matrix"),
        )
    } else {
        None
    };

    match upmix(options, source_wav, target_open_wav_writers) {
        Err(error) => {
            println!("Error upmixing: {:?}", error);
        }
        _ => {
            println!("Upmixing completed successfully");
        }
    }

    _keepawake = None;
}
