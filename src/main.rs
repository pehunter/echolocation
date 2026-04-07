use anyhow::anyhow;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream, SupportedStreamConfig};
use crossterm::event::{Event, KeyCode, KeyEvent, poll, read};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use sofar::render::{Renderer, RendererBuilder};
use std::io;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, mpsc};

use crate::position::Position;
use crate::sofa::setup_hrtf;
use crate::ui::Controller;

mod position;
mod sofa;
mod ui;

//Gets the first bi-channel output configuration
fn get_cfg_out(device: &cpal::Device) -> Result<SupportedStreamConfig, anyhow::Error> {
    //Prefer default config
    let cfg = device.default_output_config();
    if (cfg.is_err()) {
        println!("No freaking default");
        return Err(anyhow!("WRF"));
    }
    let cfg = cfg.unwrap();

    if cfg.channels() == 2 && cfg.sample_format() == SampleFormat::F32 {
        return Ok(cfg);
    }

    //If default is not bi-channel, then look at other configs.
    for i_cfg in device.supported_output_configs()? {
        if i_cfg.channels() == 2 && cfg.sample_format() == SampleFormat::F32 {
            return Ok(i_cfg.with_max_sample_rate());
        }
    }

    return Err(anyhow!("No bi-channel audio device found."));
}

//Gets the first mono-channel input configuration
fn get_cfg_in(device: &cpal::Device) -> Result<SupportedStreamConfig, anyhow::Error> {
    //Prefer default config
    let cfg = device.default_input_config()?;

    if cfg.channels() == 1 && cfg.sample_format() == SampleFormat::F32 && cfg.sample_rate() == 44100
    {
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
    //Get default input/output devices and configs
    let host = cpal::default_host();
    let out_device = host
        .default_output_device()
        .expect("Failed to find a default output device");
    println!("{}", out_device.description().unwrap());

    let in_device = host
        .default_input_device()
        .expect("Failed to find a default input device");

    let out_config = get_cfg_out(&out_device).expect("Huh??");
    println!("{:?}", out_config);
    let in_config = get_cfg_in(&out_device).unwrap();
    println!("{:?}", in_config);

    //Setup SOFA renderer and position
    let sample_rate = out_config.sample_rate() as f64;
    let (sofa, mut filter, renderer) = setup_hrtf(sample_rate as f32).unwrap();

    let pos = Arc::new(Mutex::new(Position(0.0, 0.0, 0.0)));

    //Setup audio capture and output
    let (output, _input) = setup_audio::<f32>(
        &out_device,
        &in_device,
        &out_config.into(),
        &in_config.into(),
        pos.clone(),
        renderer.clone(),
    )
    .unwrap();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_resizable(true)
            .with_inner_size([320.0, 480.0]),
        ..Default::default()
    };

    output.play().unwrap();

    let pos_renderer = renderer.clone();
    let onPositionUpdate = move |position: &Position| {
        println!("{}, {}, {}\n", position.0, position.1, position.2);
        sofa.filter(position.0, position.1, position.2, &mut filter);
        let _ = pos_renderer.lock().unwrap().set_filter(&filter);
    };
    eframe::run_native(
        "Echolocation",
        options,
        Box::new(|_| Ok(Box::new(Controller::new(pos.clone(), onPositionUpdate)))),
    )
    .unwrap();
    Ok(())
}

fn setup_audio<T>(
    device: &cpal::Device,
    mic: &cpal::Device,
    config: &cpal::StreamConfig,
    mic_config: &cpal::StreamConfig,
    pos: Arc<Mutex<Position>>,
    renderer: Arc<Mutex<Renderer>>,
) -> Result<(Stream, Stream), anyhow::Error>
where
    T: SizedSample + FromSample<f64> + Send + 'static,
{
    let (tx, rx): (Sender<f32>, Receiver<f32>) = mpsc::channel();

    let err_fn = |err| eprintln!("an error occurred on stream: {}", err);
    let output = device.build_output_stream(
        config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            write_data(data, &rx, &renderer, &pos)
                .expect("An error occurred writing to the output device.");
        },
        err_fn,
        None,
    )?;

    let input = mic.build_input_stream(
        mic_config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            read_data(data, &tx).expect("oops");
        },
        err_fn,
        None,
    )?;

    // loop {
    /* if poll(std::time::Duration::from_millis(10))? {
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
    } */
    // }

    Ok((input, output))
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
    let mut values = vec![0.0; chunks];

    //Get distance factor
    let pos = position.lock().expect("Could not retrieve position");

    let dist_factor = f32::max(0.0, 1.0 - (pos.distance() / pos.limit()));

    //Receive audio from microphone
    let mut last = 0.0;
    for i in (0..chunks) {
        let val = rx.recv().unwrap_or_else(|_| last);
        values[i] = val * dist_factor;
        last = val;
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

    if (output.chunks_mut(2).len() != chunks) {
        println!(
            "An oopsie occurred: out: {}, chunks: {}",
            output.chunks_mut(2).len(),
            chunks
        );
    }
    for (n, frame) in output.chunks_mut(2).enumerate() {
        for (ch, sample) in frame.iter_mut().enumerate() {
            let from = if ch == 1 { &use_l } else { &use_r };
            *sample = SampleType::from_sample(from[n].into());
        }
    }

    Ok(())
}
