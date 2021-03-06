#[macro_use]
extern crate vst;

mod params;

use fundsp::hacker::*;

use num_derive::FromPrimitive;

use params::{Parameter, Parameters};

use std::sync::Arc;
use std::time::Duration;

use vst::prelude::*;

use wmidi::{Note, Velocity};

struct Savoy {
    sample_rate: f32,
    time: Duration,
    note: Option<(Note, Velocity)>,
    enabled: bool,
    params: Arc<Parameters>,
    audio: Box<dyn AudioUnit64 + Send>,
}

impl Savoy {
    /// Processes midi information when a note is pressed.
    fn note_on(&mut self, note: Note, velocity: Velocity) {
        self.set_tag(Tag::NoteOn, self.time.as_secs_f64());
        self.note = Some((note, velocity));
        self.enabled = true;
    }

    /// Processes midi information when a note is released.
    fn note_off(&mut self, note: Note) {
        if let Some((current_note, ..)) = self.note {
            if current_note == note {
                self.note = None;
                self.set_tag(Tag::NoteOff, self.time.as_secs_f64());
            }
        }
    }

    /// Sets a tag value to the audio graph.
    #[inline(always)]
    fn set_tag(&mut self, tag: Tag, value: f64) {
        self.audio.set(tag as i64, value);
    }

    /// Sets a tag with a parameter value to the audio graph.
    #[inline(always)]
    fn set_tag_with_param(&mut self, tag: Tag, param: Parameter) {
        self.set_tag(tag, self.params.get_parameter(param as i32) as f64);
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

    fn get_parameter_object(&mut self) -> Arc<dyn PluginParameters> {
        Arc::clone(&self.params) as Arc<dyn PluginParameters>
    }

    fn new(_host: HostCallback) -> Self {
        let Parameters { oscillator: _, attack, decay, sustain, release } = Parameters::default();

        let time = Duration::default();

        let offset_on = || tag(Tag::NoteOn as i64, time.as_secs_f64());
        let env_on = |attack: f64, decay: f64, sustain: f64| offset_on() >> envelope2(move |seconds, offset| {
            let position = seconds - offset;
            println!("env_on position value: {}", position);

            let result = if position < attack {
                // Attack stage.
                let attack_stage_val = position / attack;

                println!("Attack stage value: {}", attack_stage_val);

                attack_stage_val
            } else if position < decay + attack{
                // Decay stage.
                let decay_position = (position - attack) / decay;

                let decay_stage_val = (1.0 - decay_position) * (1.0 - sustain) + sustain;

                println!("Decay stage value: {}", decay_stage_val);

                decay_stage_val
            } else {
                let sustain_stage_val = sustain;

                println!("Sustain stage value: {}", sustain_stage_val);

                // Sustain stage.
                sustain_stage_val
            };

            result
        });

        let offset_off = || tag(Tag::NoteOff as i64, time.as_secs_f64());
        let env_off = |release: f64| offset_off() >> envelope2(move |seconds, offset| {
            // Use 0.0 as a sentinel value indicating that the 'off' envelope
            // should be disabled when a note is playing.
            if offset <= 0.0 {
                return 1.0;
            }

            let position = seconds - offset;
            println!("env_off position value: {}", position);

            let result = if position < release {
                1.0 - position / release
            } else {
                0.0
            };

            println!("Release stage value: {}", result);

            result
        });

        let attack = || tag(Tag::Attack as i64, attack.get() as f64);
        let decay = || tag(Tag::Decay as i64, decay.get() as f64);
        let sustain = || tag(Tag::Sustain as i64, sustain.get() as f64);
        let release = || tag(Tag::Release as i64, release.get() as f64);

        let env = env_on(attack().value(), decay().value(), sustain().value()) * env_off(release().value());

        let pitch = || tag(Tag::Pitch as i64, 440.);
        let velocity = || tag(Tag::Velocity as i64, 1.);

        let audio_graph =
            pitch()
            >> (sine() * pitch())
            >> ((env * sine()) * velocity())
            >> declick()
            >> split::<U2>();

        Self {
            sample_rate: 44100.0,
            time,
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
        let (_, mut outputs) = buffer.split();

        if outputs.len() == 2 {
            let (left, right) = (outputs.get_mut(0), outputs.get_mut(1));

            for (left_chunk, right_chunk) in left
                .chunks_mut(MAX_BUFFER_SIZE)
                .zip(right.chunks_mut(MAX_BUFFER_SIZE))
            {
                let mut right_buffer = [0f64; MAX_BUFFER_SIZE];
                let mut left_buffer = [0f64; MAX_BUFFER_SIZE];

                self.set_tag_with_param(Tag::Attack, Parameter::Attack);
                self.set_tag_with_param(Tag::Decay, Parameter::Decay);
                self.set_tag_with_param(Tag::Sustain, Parameter::Sustain);
                self.set_tag_with_param(Tag::Release, Parameter::Release);

                if let Some((note, velocity)) = self.note {
                    self.set_tag(Tag::Pitch, note.to_freq_f64());
                    self.set_tag(Tag::Velocity, u8::from(velocity) as f64 / 127.);
                }

                if self.enabled {
                    self.time += Duration::from_secs_f32(MAX_BUFFER_SIZE as f32 / self.sample_rate);

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

    /// Process incoming midi events.
    ///
    /// This plugin process midi notes and velocity. Any other events are
    /// ignored.
    fn process_events(&mut self, events: &vst::api::Events) {
        for event in events.events() {
            if let vst::event::Event::Midi(midi) = event {
                if let Ok(midi) = wmidi::MidiMessage::try_from(midi.data.as_slice()) {
                    match midi {
                        wmidi::MidiMessage::NoteOn(_channel, note, velocity) => self.note_on(note, velocity),
                        wmidi::MidiMessage::NoteOff(_channel, note, _velocity) => self.note_off(note),
                        _ => (),
                    }
                }
            }
        }
    }

    /// Handle a sample rate change by the host.
    fn set_sample_rate(&mut self, rate: f32) {
        self.sample_rate = rate;
        self.time = Duration::default();
        self.audio.reset(Some(rate as f64));
    }
}

#[derive(FromPrimitive, Clone, Copy)]
pub enum Tag {
    Oscillator,
    Attack,
    Decay,
    Sustain,
    Release,
    Pitch,
    NoteOn,
    NoteOff,
    Velocity,
}

plugin_main!(Savoy);
