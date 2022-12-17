use std::env;
use std::path::Path;

use wave_stream::open_wav::OpenWav;
// These will be used
//use wave_stream::open_wav::OpenWav;
use wave_stream::wave_header::{SampleFormat, WavHeader};
//use wave_stream::wave_reader::{RandomAccessOpenWavReader, StreamOpenWavReader};
use wave_stream::{read_wav_from_file_path, write_wav_to_file_path};

mod structs;
mod upmixer;
mod window_sizes;

use upmixer::upmix;

fn main() {
    println!("Soft Matrix: Upmixes stereo wav files to surround");

    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        println!("Usage: soft_matrix [source] [destination]");
        return;
    }

    let source_wav_path = Path::new(args[1].as_str());
    let target_wav_path = Path::new(args[2].as_str());

    let open_source_wav_result = read_wav_from_file_path(source_wav_path);

    let source_wav = match open_source_wav_result {
        Err(error) => {
            println!("Can not open {}: {:?}", source_wav_path.display(), error);
            return;
        }
        Ok(source_wav) => source_wav,
    };

    // Check that source is 2 channels
    if source_wav.channels() != 2 {
        println!(
            "Upmixing can only happen from a 2-channel wav. {} has {} channel(s)",
            source_wav_path.display(),
            source_wav.channels()
        );

        return;
    }

    let header = WavHeader {
        sample_format: SampleFormat::Float,
        channels: 4, // Currently starting with quad
        sample_rate: source_wav.sample_rate(),
    };

    let open_target_wav_result = write_wav_to_file_path(target_wav_path, header);

    let target_wav = match open_target_wav_result {
        Err(error) => {
            println!("Can not open {}: {:?}", target_wav_path.display(), error);
            return;
        }
        Ok(target_wav) => target_wav,
    };

    match upmix(source_wav, target_wav) {
        Err(error) => {
            println!("Error upmixing: {:?}", error);
        }
        _ => {
            println!("Upmixing completed successfully");
        }
    }
}
