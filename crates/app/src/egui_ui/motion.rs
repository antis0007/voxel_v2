//! Shared, interruptible interaction feedback. Layout and glyphs never move.
#[derive(Clone, Copy, Default)]
pub(super) struct Feedback {
    pub hover: f32,
    pub press: f32,
    pub selection: f32,
}

pub(super) fn feedback(response: &egui::Response, selected: bool, enabled: bool) -> Feedback {
    let ctx = &response.ctx;
    let target = Feedback {
        hover: u8::from(enabled && (response.hovered() || response.has_focus())) as f32,
        press: u8::from(enabled && response.is_pointer_button_down_on()) as f32,
        selection: u8::from(selected) as f32,
    };
    let now = ctx.input(|i| i.time);
    let key = response.id.with("interaction-feedback");
    let reduced = super::magic_fx::reduced(ctx);
    let value = ctx.data_mut(|d| {
        let (mut value, previous) = d.get_temp::<(Feedback, f64)>(key)
            .unwrap_or((Feedback { selection: target.selection, ..Default::default() }, now));
        let dt = (now - previous).clamp(0.0, 0.20) as f32;
        for (value, target, rise, fall) in [
            (&mut value.hover, target.hover, 32.0, 22.0),
            (&mut value.press, target.press, 55.0, 28.0),
            (&mut value.selection, target.selection, 30.0, 24.0),
        ] {
            let rate = if target > *value { rise } else { fall };
            *value += (target - *value) * (1.0 - (-dt * rate).exp());
            if reduced || (*value - target).abs() < 0.001 { *value = target; }
        }
        d.insert_temp(key, (value, now));
        value
    });
    if (value.hover - target.hover).abs().max((value.press - target.press).abs())
        .max((value.selection - target.selection).abs()) > 0.001 {
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
    value
}
