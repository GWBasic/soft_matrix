use std::env;
use std::path::Path;

pub struct Options {
    pub source_wav_path: Box<Path>,
    pub target_wav_path: Box<Path>,
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

        // Iterate through the options
        // -channels
        // 4 or 5 or 5.1

        Some(Options {
            source_wav_path: source_wav_path.into(),
            target_wav_path: target_wav_path.into(),
        })
    }
}