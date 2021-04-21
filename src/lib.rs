#[macro_use]
extern crate vst;

use std::f64::consts::PI;

use vst::api::{Events, Supported};
use vst::buffer::AudioBuffer;
use vst::event::Event;
use vst::plugin::{CanDo, Category, HostCallback, Info, Plugin};

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

#[derive(Default)]
struct Savoy {
    sample_rate: f64,
    time: f64,
    note_duration: f64,
    note: Option<u8>,
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
    /// * `data[1]`: Contains the supplemental data for the message - so, if this was a NoteOn then this would contain the note.
    /// * `data[2]`: Further supplemental data. Would be velocity in the case of a NoteOn message.
    ///
    /// [source]: http://www.midimountain.com/midi/midi_status.htm
    fn process_midi_event(&mut self, data: [u8; 3]) {
        match data[0] {
            128 => self.note_off(data[1]),
            144 => self.note_on(data[1]),
            _ => (),
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
            ..Info::default()
        }
    }

    fn new(_host: HostCallback) -> Self {
        Savoy {
            sample_rate: 44100.0,
            note_duration: 0.0,
            time: 0.0,
            note: None,
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

            if let Some(current_note) = self.note {
                let signal = (time * midi_pitch_to_freq(current_note) * TAU).sin();

                let attack = 0.15;
                let alpha = if note_duration < attack {
                    note_duration / attack
                } else {
                    1.0
                };

                output_sample = (signal * alpha) as f32;

                self.time += per_sample;
                self.note_duration += per_sample;
            } else {
                output_sample = 0.0;
            }

            for buf_idx in 0..output_count {
                let buffer = output_buffer.get_mut(buf_idx);
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
}

plugin_main!(Savoy);
