use std::sync::{Arc, Mutex};

use sofar::{
    reader::{Filter, OpenOptions, Sofar},
    render::Renderer,
};

//Create SOFA configuration, returning the Sofar object and hrtf filter
pub fn setup_hrtf(
    sample_rate: f32,
) -> Result<(Sofar, Filter, Arc<Mutex<Renderer>>), anyhow::Error> {
    //Open file with sample rate
    let sofa = OpenOptions::new()
        .sample_rate(sample_rate as f32)
        .open("hrtf_b_nh677.sofa")?;

    let filter_length = sofa.filter_len();
    let mut filter = Filter::new(filter_length);

    sofa.filter(0.0, 0.0, 0.0, &mut filter);

    let cur_render = Arc::new(Mutex::new(
        Renderer::builder(sofa.filter_len())
            .with_sample_rate(44100.0)
            .with_partition_len(64)
            .build()
            .unwrap(),
    ));

    return Ok((sofa, filter, cur_render));
}
