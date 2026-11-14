use glam::IVec3;

#[derive(Default)]
pub(super) struct RegionOptions {
    pub origin: Option<IVec3>,
    pub checkpoint_in: Option<String>,
    pub checkpoint_out: Option<String>,
    pub session_in: Option<String>,
    pub session_out: Option<String>,
    pub gpu_backend: bool,
}

pub(super) fn parse() -> Result<RegionOptions, String> {
    let args: Vec<_> = std::env::args().collect();
    let value = |flag: &str| -> Result<Option<String>, String> {
        let mut found = None;
        for arg in args.iter().filter(|arg| arg.starts_with(flag)) {
            let text = arg.strip_prefix(&format!("{flag}=")).filter(|s| !s.is_empty())
                .ok_or_else(|| format!("expected {flag}=<value>"))?;
            if found.replace(text.to_owned()).is_some() { return Err(format!("duplicate {flag}")); }
        }
        Ok(found)
    };
    let origin = value("--ca-cpu-region")?.map(|text| -> Result<IVec3, String> {
        let axes: Vec<_> = text.split(',').map(str::parse::<i32>).collect::<Result<_, _>>()
            .map_err(|_| "invalid --ca-cpu-region=x,y,z")?;
        if axes.len() != 3 { return Err("expected --ca-cpu-region=x,y,z".into()); }
        Ok(IVec3::new(axes[0], axes[1], axes[2]))
    }).transpose()?;
    let backend = value("--ca-backend")?;
    let options = RegionOptions { origin,
        checkpoint_in: value("--ca-checkpoint-in")?, checkpoint_out: value("--ca-checkpoint-out")?,
        session_in: value("--ca-session-in")?, session_out: value("--ca-session-out")?,
        gpu_backend: match backend.as_deref() {
            None | Some("cpu") => false, Some("gpu") => true,
            _ => return Err("expected --ca-backend=cpu|gpu".into()),
        },
    };
    if options.checkpoint_in.is_some() && options.session_in.is_some() {
        return Err("choose a region checkpoint or an exact session input".into());
    }
    if options.checkpoint_out.is_some() && options.session_out.is_some() {
        return Err("choose a region checkpoint or an exact session output".into());
    }
    if options.checkpoint_in.is_some() && options.origin.is_none() {
        return Err("region checkpoint input requires --ca-cpu-region=x,y,z".into());
    }
    if options.origin.is_none() && options.session_in.is_none()
        && (backend.is_some() || options.checkpoint_out.is_some() || options.session_out.is_some()) {
        return Err("cellular options require a region origin or exact session input".into());
    }
    Ok(options)
}
