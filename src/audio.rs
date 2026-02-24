use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Stream, StreamConfig};
use std::sync::{Arc, Mutex};

use crate::synth::Synth;

pub struct AudioEngine {
    _stream: Stream,
}

impl AudioEngine {
    pub fn new(synth: Arc<Mutex<Synth>>) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("No output device found")?;

        let config = device
            .default_output_config()
            .context("No default output config")?;

        let sample_rate = config.sample_rate().0 as f32;
        let channels = config.channels() as usize;

        // Update synth sample rate
        {
            let mut s = synth.lock().unwrap();
            s.sample_rate = sample_rate;
        }

        let synth_clone = Arc::clone(&synth);

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config.into(), synth_clone, channels)?,
            cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config.into(), synth_clone, channels)?,
            cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config.into(), synth_clone, channels)?,
            fmt => anyhow::bail!("Unsupported sample format: {:?}", fmt),
        };

        stream.play().context("Failed to start audio stream")?;

        Ok(Self { _stream: stream })
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    synth: Arc<Mutex<Synth>>,
    channels: usize,
) -> Result<Stream>
where
    T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>,
{
    let err_fn = |err| eprintln!("Audio stream error: {}", err);

    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            let mut synth = synth.lock().unwrap();
            let frame_count = data.len() / channels;
            for frame in 0..frame_count {
                let sample = synth.generate_sample();
                let value = T::from_sample(sample);
                for ch in 0..channels {
                    data[frame * channels + ch] = value;
                }
            }
        },
        err_fn,
        None,
    )?;

    Ok(stream)
}
