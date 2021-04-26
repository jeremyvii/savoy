#[macro_use]
extern crate vst;

use std::f64::consts::PI;
use std::sync::Arc;

use vst::api::{Events, Supported};
use vst::buffer::AudioBuffer;
use vst::event::Event;
use vst::plugin::{CanDo, Category, HostCallback, Info, Plugin, PluginParameters};
use vst::util::AtomicFloat;

/// Returns the floating-point remainder of the given numerator/denominator.
///
/// # Examples
///
/// ```
/// let result = fmod(5.3, 2.0);
///
/// assert_eq!(result, 1.3);
/// ```
fn fmod(numerator: f64, denominator: f64) -> f64 {
    let remainder = (numerator / denominator).floor();

    numerator - remainder * denominator
}

/// Convert the midi note's pitch into the equivalent frequency.
///
/// This function assumes A4 is 440hz.
fn midi_pitch_to_freq(pitch: u8) -> f64 {
    const A4_PITCH: i8 = 69;
    const A4_FREQ: f64 = 440.0;

    // Midi notes can be 0-127
    ((f64::from(pitch as i8 - A4_PITCH)) / 12.).exp2() * A4_FREQ
}

pub const TAU: f64 = PI * 2.0;

enum Oscillator {
    Saw,
    Sine,
    Square,
    Triangle,
}

#[derive(Default)]
struct Savoy {
    sample_rate: f64,
    time: f64,
    note_duration: f64,
    note: Option<u8>,
    params: Arc<OscillatorParameters>,
}

struct OscillatorParameters {
    oscillator: AtomicFloat,
}

impl Default for OscillatorParameters {
    fn default() -> Self {
        OscillatorParameters { oscillator: AtomicFloat::new(0.0) }
    }
}

impl Savoy {
    fn time_per_sample(&self) -> f64 {
        1.0 / self.sample_rate
    }

    /// Process an incoming midi event.
    ///
    /// The midi data is split up like so:
    ///
    /// * `data[0]`: Contains the status and the channel. Source: [source]
    /// * `data[1]`: Contains the supplemental data for the message - so, if this was a
    ///   NoteOn then this would contain the note.
    /// * `data[2]`: Further supplemental data. Would be velocity in the case of a NoteOn
    ///   message.
    ///
    /// [source]: http://www.midimountain.com/midi/midi_status.htm
    fn process_midi_event(&mut self, data: [u8; 3]) {
        match data[0] {
            128 => self.note_off(data[1]),
            144 => self.note_on(data[1]),
            _ => (),
        }
    }

    fn oscillator(osc_parameter: f32) -> Option<Oscillator> {
        if (0.0..0.25).contains(&osc_parameter) {
            Some(Oscillator::Saw)
        } else if (0.25..0.5).contains(&osc_parameter) {
            Some(Oscillator::Square)
        } else if (0.5..0.75).contains(&osc_parameter) {
            Some(Oscillator::Triangle)
        } else if (0.75..1.0).contains(&osc_parameter) {
            Some(Oscillator::Sine)
        } else {
            None
        }
    }

    /// Generates signal based on the given time, pitch, and oscillator type.
    fn signal(time: f64, pitch: u8, shape: Oscillator) -> f64 {
        match shape {
            Oscillator::Saw => Savoy::saw_signal(time, pitch),
            Oscillator::Sine => Savoy::sine_signal(time, pitch),
            Oscillator::Square => Savoy::square_signal(time, pitch),
            Oscillator::Triangle => Savoy::triangle_signal(time, pitch),
        }
    }

    /// Generates a sawtooth wave signal based on the given time and pitch.
    fn saw_signal(time: f64, pitch: u8) -> f64 {
        let full_period_time = 1.0 / midi_pitch_to_freq(pitch);
        let local_time = fmod(time, full_period_time);

        (local_time / full_period_time) * 2.0 - 1.0
    }

    /// Generates a sine wave signal based on the given time and pitch.
    fn sine_signal(time: f64, pitch: u8) -> f64 {
        (time * midi_pitch_to_freq(pitch) * TAU).sin()
    }

    /// Generates a square wave signal based on the given time and pitch.
    fn square_signal(time: f64, pitch: u8) -> f64 {
        (2.0 * PI * midi_pitch_to_freq(pitch) * time).sin().signum()
    }

    /// Generates a triangle wave signal based on the given time and pitch.
    fn triangle_signal(time: f64, pitch: u8) -> f64 {
        let full_period_time = 1.0 / midi_pitch_to_freq(pitch);
        let local_time = fmod(time, full_period_time);

        let value = local_time / full_period_time;

        if value < 0.25 {
            value * 4.0
        } else if value < 0.75 {
            2.0 - (value * 4.0)
        } else {
            value * 4.0 - 4.0
        }
    }

    fn note_on(&mut self, note: u8) {
        self.note_duration = 0.0;
        self.note = Some(note)
    }

    fn note_off(&mut self, note: u8) {
        if self.note == Some(note) {
            self.note = None
        }
    }
}

impl Plugin for Savoy {
    fn get_info(&self) -> Info {
        Info {
            name: "Savoy".to_string(),
            unique_id: 19952505,
            inputs: 2,
            outputs: 2,
            category: Category::Synth,
            parameters: 1,
            ..Info::default()
        }
    }

    fn new(_host: HostCallback) -> Self {
        Savoy {
            sample_rate: 44100.0,
            note_duration: 0.0,
            time: 0.0,
            note: None,
            params: Arc::new(OscillatorParameters::default()),
        }
    }

    /// Inform the host that this plugin accepts midi input.
    fn can_do(&self, can_do: CanDo) -> Supported {
        match can_do {
            CanDo::ReceiveMidiEvent => Supported::Yes,
            _ => Supported::No,
        }
    }

    fn process(&mut self, buffer: &mut AudioBuffer<f32>) {
        let samples = buffer.samples();

        let (_, mut output_buffer) = buffer.split();

        let output_count = output_buffer.len();

        let per_sample = self.time_per_sample();

        let mut output_sample;

        for sample_index in 0..samples {
            let time = self.time;
            let note_duration = self.note_duration;

            let osc = Savoy::oscillator(self.params.oscillator.get());

            let osc = match osc {
                Some(osc) => osc,
                None => Oscillator::Sine,
            };

            if let Some(current_note) = self.note {
                let signal = Savoy::signal(time, current_note, osc);

                let attack = 0.15;
                let alpha = if note_duration < attack { note_duration / attack } else { 1.0 };

                output_sample = (signal * alpha) as f32;

                self.time += per_sample;
                self.note_duration += per_sample;
            } else {
                output_sample = 0.0;
            }

            for buffer_index in 0..output_count {
                let buffer = output_buffer.get_mut(buffer_index);
                buffer[sample_index] = output_sample;
            }
        }
    }

    /// Process any incoming midi events.
    fn process_events(&mut self, events: &Events) {
        for event in events.events() {
            match event {
                Event::Midi(ev) => self.process_midi_event(ev.data),
                // More events can be handled here.
                _ => (),
            }
        }
    }

    // Return the parameter object. This method can be omitted if the
    // plugin has no parameters.
    fn get_parameter_object(&mut self) -> Arc<dyn PluginParameters> {
        Arc::clone(&self.params) as Arc<dyn PluginParameters>
    }
}

impl PluginParameters for OscillatorParameters {
    fn get_parameter(&self, index: i32) -> f32 {
        match index {
            0 => self.oscillator.get(),
            _ => 0.0,
        }
    }

    fn set_parameter(&self, index: i32, value: f32) {
        match index {
            0 => self.oscillator.set(value),
            _ => (),
        }
    }

    fn get_parameter_text(&self, index: i32) -> String {
        match index {
            0 => format!("{:.2}", (self.oscillator.get() - 0.5) * 2f32),
            _ => "".to_string(),
        }
    }

    fn get_parameter_name(&self, index: i32) -> String {
        match index {
            0 => "Osc",
            _ => "",
        }
        .to_string()
    }
}

plugin_main!(Savoy);
