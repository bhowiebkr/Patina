use panel::{Dial, Knob, TEAL, inspector, panel};
use patina_sdk::Ui;

/// A reusable parameter knob. Wire its output to a scalar audio input. Set its range and label in the Inspector.
#[patina_sdk::node(domain = frame, free_layout, full_gui, color = "#175c59")]
pub struct Control {
    value: f32,
    low: f32,
    high: f32,
    logarithmic: bool,
    label: String,
    dial: Dial,
}

impl Control {
    pub fn init() -> Self {
        Self {
            value: 0.5,
            low: 0.0,
            high: 1.0,
            logarithmic: false,
            label: "VALUE".into(),
            dial: Dial::default(),
        }
    }
    pub fn process(
        &mut self,
        value: f32,
        low: f32,
        high: f32,
        logarithmic: bool,
        label: &str,
    ) -> f32 {
        self.low = synth_dsp::finite(low, 0.0, -100_000.0, 100_000.0);
        self.high = synth_dsp::finite(high, self.low + 1.0, self.low + 0.01, 100_001.0);
        self.logarithmic = logarithmic && self.low > 0.0;
        if self.label != label {
            self.label.clear();
            self.label.extend(label.chars().take(14));
        }
        self.value = synth_dsp::finite(value, self.low, self.low, self.high);
        self.value
    }
    pub fn ui(&mut self, ui: &mut Ui) {
        if inspector(
            ui,
            &[
                "Wire this output to a scalar parameter.",
                "Drag vertically, or use arrow keys.",
                "Logarithmic ranges need a positive minimum.",
            ],
        ) {
            return;
        }
        let mut c = ui.canvas("Parameter control", [156.0, 200.0]);
        panel(&mut c, [156.0, 200.0], "CONTROL", &self.label, TEAL);
        drop(c);
        self.dial.show(
            ui,
            &mut self.value,
            Knob {
                at: [34.0, 72.0],
                key: "value",
                label: "VALUE",
                range: [self.low, self.high],
                color: TEAL,
                logarithmic: self.logarithmic,
                unit: "",
            },
        );
        ui.pin("out", [156.0, 112.0]);
    }
}

#[cfg(test)]
mod tests {
    use super::Control;

    #[test]
    fn invalid_ranges_still_produce_a_finite_knob_value() {
        let mut control = Control::init();
        for (low, high) in [
            (f32::NAN, f32::INFINITY),
            (10.0, -10.0),
            (100_000.0, 100_000.0),
        ] {
            let value = control.process(f32::NAN, low, high, true, "A LONG PARAMETER NAME");
            assert!(value.is_finite() && control.high > control.low);
            assert!((control.low..=control.high).contains(&value));
            assert_eq!(control.label.chars().count(), 14);
        }
        control.process(0.5, -1.0, 1.0, true, "VALUE");
        assert!(!control.logarithmic);
    }
}
