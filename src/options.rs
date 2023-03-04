use std::env;
use std::path::Path;

pub struct Options {
    pub source_wav_path: Box<Path>,
    pub target_wav_path: Box<Path>,
    pub channels: Channels,
    pub num_channels_to_write: u16,
    pub transform_mono: bool,
    pub left_front_channel: u16,
    pub right_front_channel: u16,
    pub left_rear_channel: u16,
    pub right_rear_channel: u16,
    pub center_front_channel: Option<u16>,
    pub lfe_channel: Option<u16>,
}

pub enum Channels {
    Four,
    //Five,
    FiveOne,
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
                                //} else if channels_string.eq("5") {
                                //    channels = Channels::Five
                                } else if channels_string.eq("5.1") {
                                    channels = Channels::FiveOne
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
                    } else {
                        println!("Unknown flag: {}", flag);
                        return None;
                    }
                }
                None => {
                    // No more flags left, interpret the options and return them
                    let num_channels_to_write: u16;
                    let transform_mono: bool;
                    let left_front_channel: u16;
                    let right_front_channel: u16;
                    let left_rear_channel: u16;
                    let right_rear_channel: u16;
                    let center_front_channel: Option<u16>;
                    let lfe_channel: Option<u16>;

                    match channels {
                        Channels::Four => {
                            num_channels_to_write = 4;
                            transform_mono = false;
                            left_front_channel = 0;
                            right_front_channel = 1;
                            left_rear_channel = 2;
                            right_rear_channel = 3;
                            center_front_channel = None;
                            lfe_channel = None;
                        }
                        /*Channels::Five => {
                            transform_mono = true;
                            generate_center_channel = true;
                            generate_subwoofer_channel = false
                        },*/
                        Channels::FiveOne => {
                            num_channels_to_write = 6;
                            transform_mono = true;
                            left_front_channel = 0;
                            right_front_channel = 1;
                            center_front_channel = Some(2);
                            lfe_channel = Some(3);
                            left_rear_channel = 4;
                            right_rear_channel = 5;
                        }
                    }

                    return Some(Options {
                        source_wav_path: source_wav_path.into(),
                        target_wav_path: target_wav_path.into(),
                        channels,
                        num_channels_to_write,
                        transform_mono,
                        left_front_channel,
                        right_front_channel,
                        left_rear_channel,
                        right_rear_channel,
                        center_front_channel,
                        lfe_channel,
                    });
                }
            }
        }
    }
}
