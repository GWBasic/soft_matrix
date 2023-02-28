use std::env;
use std::path::Path;

pub struct Options {
    pub source_wav_path: Box<Path>,
    pub target_wav_path: Box<Path>,
    pub channels: Channels,
    pub transform_mono: bool,
    pub generate_center_channel: bool,
    pub generate_subwoofer_channel: bool,
}

pub enum Channels {
    Four,
    Five,
    FiveOne
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

        let mut channels = Channels::FiveOne;

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
                                    channels = Channels::Four
                                } else if channels_string.eq("5") {
                                    channels = Channels::Five
                                } else if channels_string.eq("5.1") {
                                    channels = Channels::FiveOne
                                } else {
                                    println!("Unknown channel configuration: {}", channels_string);
                                    return None;
                                }
                            },
                            None => {
                                println!("Channels unspecified");
                                return None;
                            }
                        }
                    } else {
                        println!("Unknown flag: {}", flag);
                        return None;
                    }
                },
                None => {
                    // No more flags left, interpret the options and return them
                    let transform_mono: bool;
                    let generate_center_channel: bool;
                    let generate_subwoofer_channel: bool;
                
                    match channels {
                        Channels::Four => {
                            transform_mono = false;
                            generate_center_channel = false;
                            generate_subwoofer_channel = false
                        },
                        Channels::Five => {
                            transform_mono = true;
                            generate_center_channel = true;
                            generate_subwoofer_channel = false
                        },
                        Channels::FiveOne => {
                            transform_mono = true;
                            generate_center_channel = true;
                            generate_subwoofer_channel = true
                        },
                    }

                    return Some(Options {
                        source_wav_path: source_wav_path.into(),
                        target_wav_path: target_wav_path.into(),
                        channels,
                        transform_mono,
                        generate_center_channel,
                        generate_subwoofer_channel
                    });
                }
            }
        }
    }
}