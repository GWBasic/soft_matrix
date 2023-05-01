use wave_stream::open_wav::OpenWav;
use wave_stream::wave_header::{Channels, SampleFormat, WavHeader};
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
        None => return,
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

    match upmix(options, source_wav, target_wav) {
        Err(error) => {
            println!("Error upmixing: {:?}", error);
        }
        _ => {
            println!("Upmixing completed successfully");
        }
    }
}
