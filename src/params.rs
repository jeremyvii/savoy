use vst::plugin::PluginParameters;
use vst::util::AtomicFloat;

pub struct Parameters {
    oscillator: AtomicFloat,
    attack: AtomicFloat,
    decay: AtomicFloat,
    sustain: AtomicFloat,
    release: AtomicFloat,
}

impl Default for Parameters {
    fn default() -> Self {
        Parameters {
            oscillator: AtomicFloat::new(0.0),
            attack: AtomicFloat::new(0.0),
            decay: AtomicFloat::new(1.0),
            sustain: AtomicFloat::new(1.0),
            release: AtomicFloat::new(0.0),
        }
    }
}

impl PluginParameters for Parameters {
    fn get_parameter(&self, index: i32) -> f32 {
        match index {
            0 => self.oscillator.get(),
            1 => self.attack.get(),
            2 => self.decay.get(),
            3 => self.sustain.get(),
            4 => self.release.get(),
            _ => 0.0,
        }
    }

    fn set_parameter(&self, index: i32, value: f32) {
        match index {
            0 => self.oscillator.set(value),
            1 => self.attack.set(value),
            2 => self.decay.set(value),
            3 => self.sustain.set(value),
            4 => self.release.set(value),
            _ => (),
        }
    }

    fn get_parameter_text(&self, index: i32) -> String {
        match index {
            0 => format!("{:.2}", (self.oscillator.get() - 0.5) * 2f32),
            1 => format!("{:.2}", (self.attack.get() - 0.5) * 2f32),
            2 => format!("{:.2}", (self.decay.get() - 0.5) * 2f32),
            3 => format!("{:.2}", (self.sustain.get() - 0.5) * 2f32),
            4 => format!("{:.2}", (self.release.get() - 0.5) * 2f32),
            _ => "".to_string(),
        }
    }

    fn get_parameter_name(&self, index: i32) -> String {
        match index {
            0 => "Oscillator",
            1 => "Attack",
            2 => "Decay",
            3 => "Sustain",
            4 => "Release",
            _ => "",
        }
        .to_string()
    }
}
