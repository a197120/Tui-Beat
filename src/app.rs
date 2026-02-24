use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::synth::{Synth, note_name};

const FALLBACK_RELEASE_THRESHOLD: Duration = Duration::from_millis(600);

// ── Key → MIDI note mapping ───────────────────────────────────────────────────

pub fn key_to_note(key: char, base_octave: i32) -> Option<u8> {
    let (st, oct): (i32, i32) = match key {
        // Lower row – white keys
        'z' => (0,0), 'x' => (2,0), 'c' => (4,0), 'v' => (5,0),
        'b' => (7,0), 'n' => (9,0), 'm' => (11,0),
        ',' => (12,0), '.' => (14,0), '/' => (16,0),
        // Lower row – black keys
        's' => (1,0), 'd' => (3,0), 'g' => (6,0),
        'h' => (8,0), 'j' => (10,0), 'l' => (13,0), ';' => (15,0),
        // Upper row – white keys
        'q' => (0,1), 'w' => (2,1), 'e' => (4,1), 'r' => (5,1),
        't' => (7,1), 'y' => (9,1), 'u' => (11,1),
        'i' => (12,1), 'o' => (14,1), 'p' => (16,1),
        // Upper row – black keys
        '2' => (1,1), '3' => (3,1), '5' => (6,1),
        '6' => (8,1), '7' => (10,1), '9' => (13,1), '0' => (15,1),
        _ => return None,
    };
    let note = (base_octave + oct) * 12 + 12 + st;
    if (0..=127).contains(&note) { Some(note as u8) } else { None }
}

// ── App mode ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    /// Live keyboard play.
    Play,
    /// Edit the melodic step sequencer.
    SynthSeq,
    /// Edit the drum machine.
    Drums,
}

// ── App state ─────────────────────────────────────────────────────────────────

pub struct App {
    pub synth:        Arc<Mutex<Synth>>,
    pub base_octave:  i32,
    pub pressed_keys: HashSet<char>,
    key_last_seen:    HashMap<char, Instant>,
    pub active_notes: Vec<u8>,
    pub should_quit:  bool,
    pub status_msg:   String,

    pub mode: AppMode,

    // Melodic sequencer cursor
    pub seq_cursor: usize,

    // Drum machine cursors
    pub drum_track: usize,  // selected track (row)
    pub drum_step:  usize,  // selected step (column)
}

impl App {
    pub fn new(synth: Arc<Mutex<Synth>>) -> Self {
        Self {
            synth,
            base_octave:  4,
            pressed_keys: HashSet::new(),
            key_last_seen: HashMap::new(),
            active_notes: Vec::new(),
            should_quit:  false,
            status_msg:   String::new(),
            mode:         AppMode::Play,
            seq_cursor:   0,
            drum_track:   0,
            drum_step:    0,
        }
    }

    // ── Keyboard / note playback ──────────────────────────────────────────

    pub fn key_press(&mut self, key: char) {
        if self.pressed_keys.contains(&key) { return; }
        self.pressed_keys.insert(key);
        if let Some(note) = key_to_note(key, self.base_octave) {
            self.synth.lock().unwrap().note_on(note);
        }
    }

    pub fn key_release(&mut self, key: char) {
        if !self.pressed_keys.remove(&key) { return; }
        if let Some(note) = key_to_note(key, self.base_octave) {
            self.synth.lock().unwrap().note_off(note);
        }
    }

    pub fn key_press_fallback(&mut self, key: char) {
        self.key_last_seen.insert(key, Instant::now());
        if self.pressed_keys.contains(&key) { return; }
        self.pressed_keys.insert(key);
        if let Some(note) = key_to_note(key, self.base_octave) {
            self.synth.lock().unwrap().note_on(note);
        }
    }

    pub fn tick_fallback_release(&mut self) {
        let now = Instant::now();
        let stale: Vec<char> = self.pressed_keys.iter().copied()
            .filter(|k| {
                key_to_note(*k, self.base_octave).is_some()
                    && self.key_last_seen.get(k)
                        .map(|t| now.duration_since(*t) >= FALLBACK_RELEASE_THRESHOLD)
                        .unwrap_or(true)
            })
            .collect();
        for k in stale { self.key_last_seen.remove(&k); self.key_release(k); }
    }

    pub fn release_all(&mut self) {
        let keys: Vec<char> = self.pressed_keys.iter().copied().collect();
        for k in keys { self.key_release(k); }
        self.key_last_seen.clear();
    }

    // ── Global controls ───────────────────────────────────────────────────

    pub fn octave_up(&mut self) {
        if self.base_octave < 8 {
            self.release_all();
            self.base_octave += 1;
            self.status_msg = format!("Octave: {}", self.base_octave);
        }
    }

    pub fn octave_down(&mut self) {
        if self.base_octave > 0 {
            self.release_all();
            self.base_octave -= 1;
            self.status_msg = format!("Octave: {}", self.base_octave);
        }
    }

    pub fn cycle_wave(&mut self) {
        let mut s = self.synth.lock().unwrap();
        s.wave_type = s.wave_type.next();
        self.status_msg = format!("Wave: {}", s.wave_type.name());
    }

    pub fn volume_up(&mut self) {
        let mut s = self.synth.lock().unwrap();
        s.volume = (s.volume + 0.05).min(1.0);
        self.status_msg = format!("Volume: {:.0}%", s.volume * 100.0);
    }

    pub fn volume_down(&mut self) {
        let mut s = self.synth.lock().unwrap();
        s.volume = (s.volume - 0.05).max(0.0);
        self.status_msg = format!("Volume: {:.0}%", s.volume * 100.0);
    }

    /// Shared master BPM — affects both the melodic and drum sequencers.
    pub fn bpm_up(&mut self) {
        let mut s = self.synth.lock().unwrap();
        s.bpm = (s.bpm + 5.0).clamp(30.0, 300.0);
        self.status_msg = format!("BPM: {:.0}", s.bpm);
    }

    pub fn bpm_down(&mut self) {
        let mut s = self.synth.lock().unwrap();
        s.bpm = (s.bpm - 5.0).clamp(30.0, 300.0);
        self.status_msg = format!("BPM: {:.0}", s.bpm);
    }

    pub fn refresh_active_notes(&mut self) {
        self.active_notes = self.synth.lock().unwrap().active_notes();
    }

    // ── UI read helpers ───────────────────────────────────────────────────

    pub fn wave_name(&self) -> String {
        self.synth.lock().unwrap().wave_type.name().to_string()
    }

    pub fn volume(&self) -> f32 { self.synth.lock().unwrap().volume }

    pub fn active_note_names(&self) -> Vec<String> {
        let mut notes = self.active_notes.clone();
        notes.sort();
        notes.iter().map(|&n| note_name(n)).collect()
    }

    pub fn highlighted_notes(&self) -> HashSet<u8> {
        self.active_notes.iter().map(|n| n % 12).collect()
    }

    pub fn seq_playing(&self) -> bool {
        self.synth.lock().unwrap().sequencer.playing
    }

    pub fn drum_playing(&self) -> bool {
        self.synth.lock().unwrap().drum_machine.playing
    }

    // ── Mode cycling ──────────────────────────────────────────────────────

    /// Cycle focus: Keyboard → SynthSeq → Drums → Keyboard.
    pub fn toggle_mode(&mut self) {
        self.release_all();
        self.mode = match self.mode {
            AppMode::Play     => AppMode::SynthSeq,
            AppMode::SynthSeq => AppMode::Drums,
            AppMode::Drums    => AppMode::Play,
        };
        self.status_msg = match self.mode {
            AppMode::Play     => "Focus: Keyboard".to_string(),
            AppMode::SynthSeq => "Focus: Synth Seq".to_string(),
            AppMode::Drums    => "Focus: Drums".to_string(),
        };
    }

    // ── Melodic sequencer controls ────────────────────────────────────────

    pub fn seq_cursor_left(&mut self) {
        let n = self.synth.lock().unwrap().sequencer.num_steps;
        self.seq_cursor = if self.seq_cursor == 0 { n - 1 } else { self.seq_cursor - 1 };
    }

    pub fn seq_cursor_right(&mut self) {
        let n = self.synth.lock().unwrap().sequencer.num_steps;
        self.seq_cursor = (self.seq_cursor + 1) % n;
    }

    pub fn seq_set_note(&mut self, key: char) {
        let Some(note) = key_to_note(key, self.base_octave) else { return };
        let cursor = self.seq_cursor;
        let n = {
            let mut s = self.synth.lock().unwrap();
            s.sequencer.set_step(cursor, note);
            s.sequencer.num_steps
        };
        self.status_msg = format!("Step {}: {}", cursor + 1, note_name(note));
        self.seq_cursor = (cursor + 1) % n;
    }

    pub fn seq_clear_step(&mut self) {
        let cursor = self.seq_cursor;
        self.synth.lock().unwrap().sequencer.clear_step(cursor);
        self.status_msg = format!("Step {} cleared", cursor + 1);
    }

    pub fn seq_toggle_play(&mut self) {
        let mut s = self.synth.lock().unwrap();
        if let Some(note) = s.sequencer.toggle_play() { s.note_off(note); }
        self.status_msg = if s.sequencer.playing { "Seq: Playing".to_string() }
                          else                   { "Seq: Paused".to_string() };
    }

    pub fn seq_cycle_steps(&mut self) {
        let mut s = self.synth.lock().unwrap();
        s.sequencer.cycle_num_steps();
        let n = s.sequencer.num_steps;
        drop(s);
        if self.seq_cursor >= n { self.seq_cursor = 0; }
        self.status_msg = format!("Seq steps: {}", n);
    }

    // ── Drum machine controls ─────────────────────────────────────────────

    pub fn drum_track_up(&mut self) {
        let n = self.synth.lock().unwrap().drum_machine.tracks.len();
        self.drum_track = if self.drum_track == 0 { n - 1 } else { self.drum_track - 1 };
    }

    pub fn drum_track_down(&mut self) {
        let n = self.synth.lock().unwrap().drum_machine.tracks.len();
        self.drum_track = (self.drum_track + 1) % n;
    }

    pub fn drum_step_left(&mut self) {
        let n = self.synth.lock().unwrap().drum_machine.num_steps;
        self.drum_step = if self.drum_step == 0 { n - 1 } else { self.drum_step - 1 };
    }

    pub fn drum_step_right(&mut self) {
        let n = self.synth.lock().unwrap().drum_machine.num_steps;
        self.drum_step = (self.drum_step + 1) % n;
    }

    pub fn drum_toggle_step(&mut self) {
        let (track, step) = (self.drum_track, self.drum_step);
        self.synth.lock().unwrap().drum_machine.toggle_step(track, step);
    }

    pub fn drum_clear_step(&mut self) {
        let (track, step) = (self.drum_track, self.drum_step);
        self.synth.lock().unwrap().drum_machine.clear_step(track, step);
    }

    pub fn drum_toggle_mute(&mut self) {
        let track = self.drum_track;
        self.synth.lock().unwrap().drum_machine.toggle_mute(track);
        let muted = self.synth.lock().unwrap().drum_machine.tracks[track].muted;
        let kind  = self.synth.lock().unwrap().drum_machine.tracks[track].kind;
        self.status_msg = if muted {
            format!("{} muted", kind.name())
        } else {
            format!("{} unmuted", kind.name())
        };
    }

    pub fn drum_toggle_play(&mut self) {
        self.synth.lock().unwrap().drum_machine.toggle_play();
        let playing = self.synth.lock().unwrap().drum_machine.playing;
        self.status_msg = if playing { "Drums: Playing".to_string() }
                          else       { "Drums: Stopped".to_string() };
    }

    pub fn drum_cycle_steps(&mut self) {
        let mut s = self.synth.lock().unwrap();
        s.drum_machine.cycle_num_steps();
        let n = s.drum_machine.num_steps;
        drop(s);
        if self.drum_step >= n { self.drum_step = 0; }
        self.status_msg = format!("Drum steps: {}", n);
    }

    pub fn drum_vol_up(&mut self) {
        let track = self.drum_track;
        let mut s = self.synth.lock().unwrap();
        s.drum_machine.track_volume_up(track);
        let vol  = s.drum_machine.tracks[track].volume;
        let kind = s.drum_machine.tracks[track].kind;
        self.status_msg = format!("{} vol: {}%", kind.name(), (vol * 100.0).round() as u32);
    }

    pub fn drum_vol_down(&mut self) {
        let track = self.drum_track;
        let mut s = self.synth.lock().unwrap();
        s.drum_machine.track_volume_down(track);
        let vol  = s.drum_machine.tracks[track].volume;
        let kind = s.drum_machine.tracks[track].kind;
        self.status_msg = format!("{} vol: {}%", kind.name(), (vol * 100.0).round() as u32);
    }

    /// Preview a drum track by key: z=Kick x=Snare c=C-Hat v=O-Hat b=Clap
    /// n=L.Tom m=M.Tom ,=H.Tom  — all fully polyphonic.
    pub fn drum_preview(&mut self, key: char) {
        let idx: usize = match key {
            'z' => 0, 'x' => 1, 'c' => 2, 'v' => 3,
            'b' => 4, 'n' => 5, 'm' => 6, ',' => 7,
            _ => return,
        };
        self.synth.lock().unwrap().drum_machine.trigger_now(idx);
    }
}
