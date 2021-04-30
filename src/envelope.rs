use std::sync::Arc;

use crate::SavoyParameters;

pub enum EnvelopeStage {
    Off,
    Attack,
    Decay,
    Sustain,
    Release,
}

pub struct Envelope {
    value: f64,
    stage: EnvelopeStage,
    params: Arc<SavoyParameters>,
}

impl Envelope {
    fn multiplier(start: f64, end: f64, length: f64) -> f64 {
        1.0 + ((end.ln() - start.ln()) / length)
    }

    fn process(&mut self, signal: f64, stage: EnvelopeStage) {
        match stage {
            EnvelopeStage::Off => {

            }
            EnvelopeStage::Attack => {}
            EnvelopeStage::Decay => {}
            EnvelopeStage::Sustain => {}
            EnvelopeStage::Release => {}
        }
    }
}
