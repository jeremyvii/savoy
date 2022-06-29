#[macro_use]
extern crate vst;

mod params;

use fundsp::hacker::*;

use num_derive::FromPrimitive;

use params::{Parameter, Parameters};

use std::sync::Arc;
use std::time::Duration;

use vst::api::Supported;
use vst::buffer::AudioBuffer;
use vst::plugin::{CanDo, Category, HostCallback, Info, Plugin, PluginParameters};

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
    #[inline(always)]
    fn set_tag(&mut self, tag: Tag, value: f64) {
        self.audio.set(tag as i64, value);
    }

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

    fn new(_host: HostCallback) -> Self {
        let Parameters { oscillator: _, attack, decay, sustain, release } = Parameters::default();

        let offset_on = || tag(Tag::NoteOn as i64, 0.0);
        let env_on = |attack: f64, decay: f64, sustain: f64| offset_on() >> envelope2(move |t, offset| {
            let position = t - offset;

            if position < attack {
                position / attack
            } else if position < decay + attack{
                let decay_position = (position - attack) / decay;

                (1.0 - decay_position) * (1.0 - sustain) + sustain
            } else {
                sustain
            }
        });

        let offset_off = || tag(Tag::NoteOff as i64, 0.0);
        let env_off = |release: f64| offset_off() >> envelope2(move |t, offset| {
            // Somewhat hacky: using 0.0 as a sentinel value indicating that the 'off'
            // envelope should be disabled when a note is playing.
            if offset <= 0.0 {
                return 1.0;
            }

            let position = t - offset;
            if position < release {
                1.0 - position / release
            } else {
                0.0
            }
        });


        let attack = || tag(Tag::Attack as i64, attack.get() as f64);
        let decay = || tag(Tag::Decay as i64, decay.get() as f64);
        let sustain = || tag(Tag::Sustain as i64, sustain.get() as f64);
        let release = || tag(Tag::Release as i64, release.get() as f64);

        //let env = env_on(attack(), decay(), sustain()) * env_off(release());
        //let offset = || tag(Tag::NoteOn as i64, 0.);
        //let env = || offset() >> envelope2(|t, offset| downarc((t - offset) * 2.));

        let freq = || tag(Tag::Freq as i64, 440.);

        let audio_graph = freq() >> (sine() * freq()) >> (env_on(attack().value(), decay().value(), sustain().value()) * env_off(release().value()) * sine())
            >> declick()
            >> split::<U2>();

        Self {
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

                if let Some((note, ..)) = self.note {
                    self.set_tag(Tag::Freq, note.to_freq_f64())
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

    fn process_events(&mut self, events: &vst::api::Events) {
        for event in events.events() {
            if let vst::event::Event::Midi(midi) = event {
                if let Ok(midi) = wmidi::MidiMessage::try_from(midi.data.as_slice()) {
                    match midi {
                        wmidi::MidiMessage::NoteOn(_channel, note, velocity) => {
                            self.set_tag(Tag::NoteOn, self.time.as_secs_f64());
                            self.note = Some((note, velocity));
                            self.enabled = true;
                        }
                        wmidi::MidiMessage::NoteOff(_channel, note, _velocity) => {
                            if let Some((current_note, ..)) = self.note {
                                if current_note == note {
                                    self.note = None;
                                    self.set_tag(Tag::NoteOff, self.time.as_secs_f64());
                                }
                            }
                        }
                        _ => (),
                    }
                }
            }
        }
    }

    fn set_sample_rate(&mut self, rate: f32) {
        self.sample_rate = rate;
        self.time = Duration::default();
        self.audio.reset(Some(rate as f64));
    }
}

#[derive(FromPrimitive, Clone, Copy)]
pub enum Tag {
    Oscillator = 0,
    Attack = 1,
    Decay = 2,
    Sustain = 3,
    Release = 4,
    Freq = 5,
    NoteOn = 6,
    NoteOff = 7,
}

plugin_main!(Savoy);
