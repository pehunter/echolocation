use sofar::reader::{Filter, OpenOptions, Sofar};

//Create SOFA configuration, returning the Sofar object and hrtf filter
pub fn setup_hrtf(sample_rate: f32) -> Result<(Sofar, Filter), anyhow::Error> {
    //Open file with sample rate
    let sofa = OpenOptions::new()
        .sample_rate(sample_rate as f32)
        .open("hrtf_b_nh677.sofa")?;

    let filter_length = sofa.filter_len();
    let mut filter = Filter::new(filter_length);

    sofa.filter(0.0, 0.0, 0.0, &mut filter);

    return Ok((sofa, filter));
}
