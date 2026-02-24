use std::collections::HashMap;
use std::f32::consts::PI;

use crate::drums::DrumMachine;
use crate::effects::EffectChain;
use crate::sequencer::Sequencer;

// ── Waveform ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WaveType { Sine, Square, Sawtooth, Triangle }

impl WaveType {
    pub fn next(self) -> Self {
        match self {
            Self::Sine => Self::Square, Self::Square => Self::Sawtooth,
            Self::Sawtooth => Self::Triangle, Self::Triangle => Self::Sine,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Sine => "Sine", Self::Square => "Square",
            Self::Sawtooth => "Sawtooth", Self::Triangle => "Triangle",
        }
    }
}

// ── ADSR envelope ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EnvelopeStage { Attack, Decay, Sustain, Release, Off }

// ── Melodic voice ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Voice {
    pub frequency:     f32,
    pub phase:         f32,
    pub stage:         EnvelopeStage,
    pub level:         f32,
    pub release_level: f32,
}

impl Voice {
    pub fn new(note: u8) -> Self {
        Self { frequency: note_to_freq(note), phase: 0.0,
               stage: EnvelopeStage::Attack, level: 0.0, release_level: 0.0 }
    }

    pub fn release(&mut self) {
        if self.stage != EnvelopeStage::Off {
            self.release_level = self.level;
            self.stage = EnvelopeStage::Release;
        }
    }

    pub fn is_finished(&self) -> bool { self.stage == EnvelopeStage::Off }

    pub fn next_sample(&mut self, sr: f32, wave: WaveType,
                       attack: f32, decay: f32, sustain: f32, release: f32) -> f32 {
        let dt = 1.0 / sr;
        match self.stage {
            EnvelopeStage::Attack => {
                self.level += dt / attack;
                if self.level >= 1.0 { self.level = 1.0; self.stage = EnvelopeStage::Decay; }
            }
            EnvelopeStage::Decay => {
                self.level -= dt * (1.0 - sustain) / decay;
                if self.level <= sustain { self.level = sustain; self.stage = EnvelopeStage::Sustain; }
            }
            EnvelopeStage::Sustain => { self.level = sustain; }
            EnvelopeStage::Release => {
                self.level -= dt * self.release_level / release;
                if self.level <= 0.0 { self.level = 0.0; self.stage = EnvelopeStage::Off; }
            }
            EnvelopeStage::Off => return 0.0,
        }

        let sample = match wave {
            WaveType::Sine     => (self.phase * 2.0 * PI).sin(),
            WaveType::Square   => if (self.phase * 2.0 * PI).sin() >= 0.0 { 1.0 } else { -1.0 },
            WaveType::Sawtooth => 2.0 * self.phase - 1.0,
            WaveType::Triangle => {
                if self.phase < 0.5 { 4.0 * self.phase - 1.0 } else { 3.0 - 4.0 * self.phase }
            }
        };

        self.phase += self.frequency / sr;
        if self.phase >= 1.0 { self.phase -= 1.0; }
        sample * self.level
    }
}

// ── Synth ─────────────────────────────────────────────────────────────────────

pub struct Synth {
    pub sample_rate: f32,
    pub bpm:         f32,       // master clock shared by all sequencers
    pub wave_type:   WaveType,
    pub voices:      HashMap<u8, Voice>,
    pub attack:  f32,
    pub decay:   f32,
    pub sustain: f32,
    pub release: f32,
    pub volume:  f32,
    pub sequencer:    Sequencer,
    pub drum_machine: DrumMachine,
    /// Insert effects applied to the melodic synth bus (before mixing with drums).
    pub fx: EffectChain,
}

impl Synth {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            bpm:      120.0,
            wave_type: WaveType::Sine,
            voices:   HashMap::new(),
            attack:   0.01, decay: 0.1, sustain: 0.7, release: 0.3,
            volume:   0.5,
            sequencer:    Sequencer::new(sample_rate),
            drum_machine: DrumMachine::new(sample_rate),
            fx:       EffectChain::new(),
        }
    }

    pub fn note_on(&mut self, note: u8) {
        self.voices.insert(note, Voice::new(note));
    }

    pub fn note_off(&mut self, note: u8) {
        if let Some(v) = self.voices.get_mut(&note) { v.release(); }
    }

    pub fn generate_sample(&mut self) -> f32 {
        // ── Melodic sequencer ──────────────────────────────────────────────
        if let Some(ev) = self.sequencer.tick(self.bpm) {
            if let Some(n) = ev.note_off {
                if let Some(v) = self.voices.get_mut(&n) { v.release(); }
            }
            if let Some(n) = ev.note_on {
                self.voices.insert(n, Voice::new(n));
            }
        }

        // ── Melodic voices ─────────────────────────────────────────────────
        let wave = self.wave_type;
        let sr   = self.sample_rate;
        let (a, d, s, r) = (self.attack, self.decay, self.sustain, self.release);

        let mut mel_mix = 0.0f32;
        for voice in self.voices.values_mut() {
            mel_mix += voice.next_sample(sr, wave, a, d, s, r);
        }
        self.voices.retain(|_, v| !v.is_finished());

        // Scale + soft clip melodic bus, apply insert fx
        let mel_scaled  = mel_mix * self.volume / (self.voices.len().max(1) as f32).sqrt();
        let mel_out     = self.fx.process(mel_scaled);

        // ── Drum bus ───────────────────────────────────────────────────────
        let drum_out = self.drum_machine.generate_sample(self.bpm) * self.volume;

        // ── Master mix ────────────────────────────────────────────────────
        (mel_out + drum_out).tanh()
    }

    pub fn active_notes(&self) -> Vec<u8> {
        self.voices.keys().copied().collect()
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

pub fn note_to_freq(note: u8) -> f32 {
    440.0 * 2f32.powf((note as f32 - 69.0) / 12.0)
}

pub fn note_name(note: u8) -> String {
    let names = ["C","C#","D","D#","E","F","F#","G","G#","A","A#","B"];
    format!("{}{}", names[(note % 12) as usize], (note / 12) as i32 - 1)
}
