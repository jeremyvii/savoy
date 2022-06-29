use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use std::fmt::Display;

use vst::plugin::PluginParameters;
use vst::util::AtomicFloat;

pub struct Parameters {
    pub oscillator: AtomicFloat,
    pub attack: AtomicFloat,
    pub decay: AtomicFloat,
    pub sustain: AtomicFloat,
    pub release: AtomicFloat,
}

impl Default for Parameters {
    fn default() -> Self {
        Parameters {
            oscillator: AtomicFloat::new(0.0),
            attack: AtomicFloat::new(0.0),
            decay: AtomicFloat::new(1.0),
            sustain: AtomicFloat::new(1.0),
            release: AtomicFloat::new(0.2),
        }
    }
}

impl PluginParameters for Parameters {
    fn get_parameter(&self, index: i32) -> f32 {
        match FromPrimitive::from_i32(index) {
            Some(Parameter::Oscillator) => self.oscillator.get(),
            Some(Parameter::Attack) => self.attack.get(),
            Some(Parameter::Decay) => self.decay.get(),
            Some(Parameter::Sustain) => self.sustain.get(),
            Some(Parameter::Release) => self.release.get(),
            _ => 0.0,
        }
    }

    fn set_parameter(&self, index: i32, value: f32) {
        match FromPrimitive::from_i32(index) {
            Some(Parameter::Oscillator) => self.oscillator.set(value),
            Some(Parameter::Attack) => self.attack.set(value),
            Some(Parameter::Decay) => self.decay.set(value),
            Some(Parameter::Sustain) => self.sustain.set(value),
            Some(Parameter::Release) => self.release.set(value),
            _ => (),
        }
    }

    fn get_parameter_name(&self, index: i32) -> String {
        let param: Option<Parameter> = FromPrimitive::from_i32(index);
        param
            .map(|f| f.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

#[derive(FromPrimitive, Clone, Copy)]
pub enum Parameter {
    Oscillator = 0,
    Attack = 1,
    Decay = 2,
    Sustain = 3,
    Release = 4,
}

impl Display for Parameter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Parameter::Oscillator => "Oscillator",
                Parameter::Attack => "Attack",
                Parameter::Decay => "Decay",
                Parameter::Sustain => "Sustain",
                Parameter::Release => "Release",
            }
        )
    }
}
