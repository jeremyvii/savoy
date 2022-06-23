#[macro_use]
extern crate vst;

mod params;

use fundsp::hacker::*;

use params::Parameters;

use std::f64::consts::PI;
use std::sync::Arc;
use std::time::Duration;

use vst::api::Supported;
use vst::buffer::AudioBuffer;
use vst::plugin::{CanDo, Category, HostCallback, Info, Plugin, PluginParameters};

use wmidi::{Note, Velocity};

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

fn midi_velocity_to_amplitude(velocity: u8) -> f64 {
    f64::from(velocity) / 127.0
}

pub const TAU: f64 = PI * 2.0;

enum Oscillator {
    Saw,
    Sine,
    Square,
    Triangle,
}

struct Savoy {
    sample_rate: f64,
    time: Duration,
    note: Option<(Note, Velocity)>,
    enabled: bool,
    params: Arc<Parameters>,
    audio: Box<dyn AudioUnit64 + Send>,
}

impl Savoy {
    fn time_per_sample(&self) -> f64 {
        1.0 / self.sample_rate
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

    #[inline(always)]
    fn set_tag(&mut self, tag: Tag, value: f64) {
        self.audio.set(tag as i64, value);
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
            parameters: 5,
            ..Info::default()
        }
    }

    fn new(_host: HostCallback) -> Self {
        let freq = || tag(Tag::Freq as i64, 440.);

        let offset = || tag(Tag::NoteOn as i64, 0.);
        let env = || offset() >> envelope2(|t, offset| downarc((t - offset) * 2.));

        let audio_graph = freq() >> sine() * freq() + freq() >> env() * sine() >> split::<U2>();

        Savoy {
            sample_rate: 44100.0,
            time: Duration::default(),
            note: None,
            params: Arc::new(Parameters::default()),
            audio: Box::new(audio_graph) as Box<dyn AudioUnit64 + Send>,
            enabled: false,
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
        // ------------------------------------------- //
        // 3. Using fundsp to process our audio buffer //
        // ------------------------------------------- //
        let (_, mut outputs) = buffer.split();
        if outputs.len() == 2 {
            let (left, right) = (outputs.get_mut(0), outputs.get_mut(1));

            for (left_chunk, right_chunk) in left
                .chunks_mut(MAX_BUFFER_SIZE)
                .zip(right.chunks_mut(MAX_BUFFER_SIZE))
            {
                let mut right_buffer = [0f64; MAX_BUFFER_SIZE];
                let mut left_buffer = [0f64; MAX_BUFFER_SIZE];

                if let Some((note, ..)) = self.note {
                    self.set_tag(Tag::Freq, note.to_freq_f64())
                }

                if self.enabled {
                    // -------------- //
                    // 5. Timekeeping //
                    // -------------- //
                    self.time += Duration::from_secs_f64(MAX_BUFFER_SIZE as f64 / self.sample_rate);
                    self.audio.process(
                        MAX_BUFFER_SIZE,
                        &[],
                        &mut [&mut left_buffer, &mut right_buffer],
                    );
                }

                for (chunk, output) in left_chunk.iter_mut().zip(left_buffer.iter()) {
                    *chunk = *output as f32;
                }

                for (chunk, output) in right_chunk.iter_mut().zip(right_buffer.iter()) {
                    *chunk = *output as f32;
                }
            }
        }
    }

    fn process_events(&mut self, events: &vst::api::Events) {
        for event in events.events() {
            if let vst::event::Event::Midi(midi) = event {
                if let Ok(midi) = wmidi::MidiMessage::try_from(midi.data.as_slice()) {
                    match midi {
                        wmidi::MidiMessage::NoteOn(_channel, note, velocity) => {
                            // ----------------------------------------- //
                            // 6. Set `NoteOn` time tag and enable synth //
                            // ----------------------------------------- //
                            self.set_tag(Tag::NoteOn, self.time.as_secs_f64());
                            self.note = Some((note, velocity));
                            self.enabled = true;
                        }
                        wmidi::MidiMessage::NoteOff(_channel, note, _velocity) => {
                            if let Some((current_note, ..)) = self.note {
                                if current_note == note {
                                    self.note = None;
                                }
                            }
                        }
                        _ => (),
                    }
                }
            }
        }
    }

    fn get_parameter_object(&mut self) -> Arc<dyn PluginParameters> {
        Arc::clone(&self.params) as Arc<dyn PluginParameters>
    }
}

#[derive(Clone, Copy)]
pub enum Tag {
    Freq = 0,
    Modulation = 1,
    NoteOn = 2,
}

plugin_main!(Savoy);
