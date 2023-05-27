use std::env;
use std::path::Path;

use wave_stream::wave_header::Channels;

use crate::matrix::{ DefaultMatrix, Matrix };

pub struct Options {
    pub source_wav_path: Box<Path>,
    pub target_wav_path: Box<Path>,
    pub channel_layout: ChannelLayout,
    pub transform_mono: bool,
    pub channels: Channels,

    // Performs additional adjustments according to the specific chosen matrix
    // SQ, QS, RM, ect
    pub matrix: Box<dyn Matrix>,
}

pub enum ChannelLayout {
    Four,
    Five,
    FiveOne,
}

pub enum MatrixFormat {
    Default,
    SQ,
}

impl Options {
    pub fn parse() -> Option<Options> {
        let args: Vec<String> = env::args().collect();

        if args.len() < 3 {
            println!("Usage: soft_matrix [source] [destination]");
            return None;
        }

        let mut args_iter = args.into_iter();

        // ignore the executable name
        let _ = args_iter.next().unwrap();

        let source_wav_path = args_iter.next().unwrap();
        let source_wav_path = Path::new(source_wav_path.as_str());

        let target_wav_path = args_iter.next().unwrap();
        let target_wav_path = Path::new(target_wav_path.as_str());

        let mut channel_layout = ChannelLayout::FiveOne;
        let mut matrix_format = MatrixFormat::Default;

        // Iterate through the options
        // -channels
        // 4 or 5 or 5.1

        loop {
            match args_iter.next() {
                Some(flag) => {
                    // Parse a flag
                    if flag.eq("-channels") {
                        match args_iter.next() {
                            Some(channels_string) => {
                                if channels_string.eq("4") {
                                    channel_layout = ChannelLayout::Four
                                } else if channels_string.eq("5") {
                                    channel_layout = ChannelLayout::Five
                                } else if channels_string.eq("5.1") {
                                    channel_layout = ChannelLayout::FiveOne
                                } else {
                                    println!("Unknown channel configuration: {}", channels_string);
                                    return None;
                                }
                            }
                            None => {
                                println!("Channels unspecified");
                                return None;
                            }
                        }
                    } else if flag.eq("-matrix") {
                        match args_iter.next() {
                            Some(matrix_format_string) => {
                                if matrix_format_string.eq("default") {
                                    matrix_format = MatrixFormat::Default
                                } else if matrix_format_string.eq("sq") {
                                    matrix_format = MatrixFormat::SQ
                                } else {
                                    println!("Unknown matrix format: {}", matrix_format_string);
                                    return None;
                                }
                            }
                            None => {
                                println!("Matrix unspecified");
                                return None;
                            }
                        }
                    } else {
                        println!("Unknown flag: {}", flag);
                        return None;
                    }
                }
                None => {
                    // No more flags left, interpret the options and return them
                    let transform_mono: bool;
                    let channels: Channels;
                    let matrix: Box<dyn Matrix>;

                    match channel_layout {
                        ChannelLayout::Four => {
                            transform_mono = false;
                            channels = Channels::new()
                                .front_left()
                                .front_right()
                                .back_left()
                                .back_right();
                        }
                        ChannelLayout::Five => {
                            transform_mono = true;
                            channels = Channels::new()
                                .front_left()
                                .front_right()
                                .front_center()
                                .back_left()
                                .back_right();
                        }
                        ChannelLayout::FiveOne => {
                            transform_mono = true;
                            channels = Channels::new()
                                .front_left()
                                .front_right()
                                .front_center()
                                .low_frequency()
                                .back_left()
                                .back_right();
                        }
                    }

                    match matrix_format {
                        MatrixFormat::Default => matrix = Box::new(DefaultMatrix::new()),
                        MatrixFormat::SQ => matrix = Box::new(DefaultMatrix::sq()),
                    }

                    return Some(Options {
                        source_wav_path: source_wav_path.into(),
                        target_wav_path: target_wav_path.into(),
                        channel_layout,
                        transform_mono,
                        channels,
                        matrix,
                    });
                }
            }
        }
    }
}
