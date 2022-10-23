use std::io::{Read, Result, Seek};

use wave_stream::open_wav::OpenWav;
use wave_stream::wave_reader::{OpenWavReader, RandomAccessOpenWavReader, RandomAccessWavReader};
use wave_stream::wave_writer::{OpenWavWriter, RandomAccessWavWriter};

pub fn upmix<TReader: 'static + Read + Seek>(
    source_wav_reader: OpenWavReader<TReader>,
    target_wav_writer: OpenWavWriter,
) -> Result<()> {
    let min_window_size = source_wav_reader.sample_rate() / 20;
    let mut window_size = 2;
    while window_size < min_window_size {
        window_size *= 2;
    }
    let window_size = window_size as usize;

    let mut source_wav_reader = source_wav_reader.get_random_access_f32_reader()?;
    let mut target_wav_writer = target_wav_writer.get_random_access_f32_writer()?;

    let mut right_buffer = vec![0f32; (window_size / 2) + 1];
    let mut left_buffer = vec![0f32; (window_size / 2) + 1];

    for sample_ctr in 0..(((window_size / 2) - 1) as u32) {
        left_buffer.push(source_wav_reader.read_sample(sample_ctr, 0)?);
        right_buffer.push(source_wav_reader.read_sample(sample_ctr, 1)?);
    }

    let read_offset = (window_size / 2) as u32;
    for sample_ctr in 0..source_wav_reader.info().len_samples() {
        upmix_sample(
            &mut source_wav_reader,
            &mut target_wav_writer,
            &mut right_buffer,
            &mut left_buffer,
            sample_ctr,
            read_offset,
        )?;
    }

    target_wav_writer.flush()?;

    Ok(())
}

fn upmix_sample(
    source_wav_reader: &mut RandomAccessWavReader<f32>,
    target_wav_writer: &mut RandomAccessWavWriter<f32>,
    left_buffer: &mut Vec<f32>,
    right_buffer: &mut Vec<f32>,
    sample_ctr: u32,
    read_offset: u32,
) -> Result<()> {
    left_buffer.remove(0);
    right_buffer.remove(0);

    let sample_to_read = sample_ctr + read_offset;

    if sample_to_read < source_wav_reader.info().len_samples() {
        left_buffer.push(source_wav_reader.read_sample(sample_to_read, 0)?);
        right_buffer.push(source_wav_reader.read_sample(sample_to_read, 1)?);
    } else {
        left_buffer.push(0f32);
        right_buffer.push(0f32);
    }

    let sample_ctr_in_buffer = right_buffer.len() / 2;
    target_wav_writer.write_sample(sample_ctr, 0, left_buffer[sample_ctr_in_buffer])?;
    target_wav_writer.write_sample(sample_ctr, 1, right_buffer[sample_ctr_in_buffer])?;
    target_wav_writer.write_sample(sample_ctr, 2, left_buffer[sample_ctr_in_buffer])?;
    target_wav_writer.write_sample(sample_ctr, 3, right_buffer[sample_ctr_in_buffer])?;

    Ok(())
}
