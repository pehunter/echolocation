use anyhow::anyhow;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, SupportedStreamConfig};
use crossterm::event::{Event, KeyCode, KeyEvent, poll, read};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use sofar::render::Renderer;
use std::io;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, mpsc};

use crate::position::Position;
use crate::sofa::setup_hrtf;

mod position;
mod sofa;

//Gets the first bi-channel output configuration
fn get_cfg_out(device: &cpal::Device) -> Result<SupportedStreamConfig, anyhow::Error> {
    //Prefer default config
    let cfg = device.default_output_config()?;

    if cfg.channels() == 2 {
        return Ok(cfg);
    }

    //If default is not bi-channel, then look at other configs.
    for i_cfg in device.supported_output_configs()? {
        if i_cfg.channels() == 2 {
            return Ok(i_cfg.with_max_sample_rate());
        }
    }

    return Err(anyhow!("No bi-channel audio device found."));
}

//Gets the first mono-channel input configuration
fn get_cfg_in(device: &cpal::Device) -> Result<SupportedStreamConfig, anyhow::Error> {
    //Prefer default config
    let cfg = device.default_input_config()?;

    if cfg.channels() == 1 {
        return Ok(cfg);
    }

    //If default is not bi-channel, then look at other configs.
    for i_cfg in device.supported_input_configs()? {
        if i_cfg.channels() == 1
            && i_cfg.sample_format() == SampleFormat::F32
            && i_cfg.max_sample_rate() >= 44100
        {
            return Ok(i_cfg.with_sample_rate(44100));
        }
    }

    return Err(anyhow!("No mono-channel audio device found."));
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let host = cpal::default_host();
    let out_device = host
        .default_output_device()
        .expect("Failed to find a default output device");

    let in_device = host
        .default_input_device()
        .expect("Failed to find a default input device");

    let out_config = get_cfg_out(&out_device).unwrap();
    println!("{:?}", out_config);
    let in_config = get_cfg_in(&out_device).unwrap();
    println!("{:?}", in_config);

    run::<f32>(
        &out_device,
        &in_device,
        &out_config.into(),
        &in_config.into(),
    )
    .unwrap_or_else(|_| disable_raw_mode().unwrap());
    disable_raw_mode()?;
    Ok(())
}

fn run<T>(
    device: &cpal::Device,
    mic: &cpal::Device,
    config: &cpal::StreamConfig,
    mic_config: &cpal::StreamConfig,
) -> Result<(), anyhow::Error>
where
    T: SizedSample + FromSample<f64> + Send + 'static,
{
    let sample_rate = config.sample_rate as f64;

    //Read SOFA file and
    let (sofa, mut filter) = setup_hrtf(sample_rate as f32).unwrap();

    let cur_render = Arc::new(Mutex::new(
        Renderer::builder(sofa.filter_len())
            .with_sample_rate(44100.0)
            .with_partition_len(64)
            .build()
            .unwrap(),
    ));
    let render_clone = Arc::clone(&cur_render);

    let pos = Arc::new(Mutex::new(Position(0.0, 0.0, 0.0)));
    let pos_clone = Arc::clone(&pos);

    let (tx, rx): (Sender<f32>, Receiver<f32>) = mpsc::channel();

    let err_fn = |err| eprintln!("an error occurred on stream: {}", err);
    let stream = device.build_output_stream(
        config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            write_data(data, &rx, &render_clone, &pos_clone)
                .expect("An error occurred writing to the output device.");
        },
        err_fn,
        None,
    )?;

    let _input = mic.build_input_stream(
        mic_config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            read_data(data, &tx).expect("oops");
        },
        err_fn,
        None,
    )?;

    stream.play()?;

    loop {
        if poll(std::time::Duration::from_millis(10))? {
            if let Event::Key(KeyEvent {
                code,
                modifiers: _,
                kind: _,
                state: _,
            }) = read()?
            {
                if code == KeyCode::Esc {
                    break;
                }

                let mut position = pos.lock().expect("Could not retrieve position");
                //Update render when the position is changed
                if position.keycode(code) {
                    std::process::Command::new("clear").status().unwrap();
                    println!("{}, {}, {}\n", position.0, position.1, position.2);
                    sofa.filter(position.0, position.1, position.2, &mut filter);
                    let _ = cur_render.lock().unwrap().set_filter(&filter);
                }
            }
        }
    }

    Ok(())
}

//Read data from input device
fn read_data(input: &[f32], tx: &Sender<f32>) -> Result<(), anyhow::Error> {
    for dat in input {
        tx.send(dat.clone())
            .expect("There was a problem sending audio data to the channel");
    }
    Ok(())
}

//Writes data to the output stream
fn write_data<SampleType>(
    output: &mut [SampleType],
    rx: &Receiver<f32>,
    render: &Arc<Mutex<Renderer>>,
    position: &Arc<Mutex<Position>>,
) -> Result<(), anyhow::Error>
where
    SampleType: Sample + FromSample<f64>,
{
    //Create vector to hold the waveform. 1 chunk = data in each channel for 1.. frame
    let chunks = output.len() / 2;
    let chunks = chunks - (chunks % 64);
    let mut values = vec![0.0; chunks];

    //Get distance factor
    let pos = position.lock().expect("Could not retrieve position");

    let dist_factor = f32::max(0.0, 1.0 - (pos.distance() / pos.limit()));

    //Receive audio from microphone
    for i in (0..chunks) {
        let val = rx.recv().unwrap();
        values[i] = val * dist_factor;
    }

    //Filter waveform to get audio for left/right
    let mut left = vec![0.0; chunks];
    let mut right = vec![0.0; chunks];

    let (use_l, use_r) = if pos.0 == 0.0 && pos.1 == 0.0 && pos.2 == 0.0 {
        (&values, &values)
    } else {
        render
            .lock()
            .expect("An error occurred trying to retrieve the renderer")
            .process_block(values, &mut left, &mut right)?;
        (&left, &right)
    };

    for (n, frame) in output.chunks_mut(2).enumerate() {
        for (ch, sample) in frame.iter_mut().enumerate() {
            let from = if ch == 1 { &use_l } else { &use_r };
            *sample = SampleType::from_sample(from[n].into());
        }
    }

    Ok(())
}
