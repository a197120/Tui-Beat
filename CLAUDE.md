# RustTuiSynth — Claude context

Terminal synthesizer and drum machine written in Rust.
No tests exist yet. Build with `cargo build`, run with `cargo run`.

## Dependencies
- `ratatui 0.29` — TUI rendering
- `crossterm 0.28` — terminal I/O, keyboard events
- `cpal 0.15` — cross-platform audio output
- `anyhow 1.0` — error handling

## Module map

| File | Purpose |
|------|---------|
| `main.rs` | Terminal setup, event loop, key routing |
| `app.rs` | All application state; keyboard→action methods |
| `audio.rs` | CPAL audio stream; calls `Synth::generate_sample()` per frame |
| `synth.rs` | Melodic polyphonic voices, ADSR, waveforms, master mix |
| `sequencer.rs` | Melodic step sequencer (sample-accurate) |
| `drums.rs` | 8-track drum machine with synthesized voices |
| `effects.rs` | `AudioEffect` trait + `EffectChain` (effects scaffold) |
| `ui.rs` | All Ratatui rendering; one function per panel |

## Architecture

### Audio thread
`AudioEngine` holds a CPAL stream. The callback locks `Arc<Mutex<Synth>>` and calls
`Synth::generate_sample()` once per sample. **Everything audio-generating lives inside
`Synth`** and runs in this thread.

```
CPAL callback
  └─ Synth::generate_sample()
       ├─ Sequencer::tick(bpm)          → note_on/note_off into voices
       ├─ melodic voice pool mix        → Synth::fx (EffectChain, empty)
       ├─ DrumMachine::generate_sample(bpm)
       │    ├─ fire_step() → DrumVoice pool (polyphonic)
       │    └─ DrumMachine::fx (EffectChain, empty)
       └─ (melodic + drums).tanh()      → master output
```

### UI / event thread
`main::run()` polls crossterm events at 16 ms. Key events call methods on `App`, which
locks the synth mutex only for the duration of each method call.

### Shared state
```
Arc<Mutex<Synth>>
  ├─ bpm: f32              ← single master clock for both sequencers
  ├─ volume: f32           ← master volume (applied to both buses)
  ├─ voices: HashMap<u8,Voice>
  ├─ sequencer: Sequencer
  ├─ drum_machine: DrumMachine
  └─ fx: EffectChain       ← melodic bus effects (empty)
```

### BPM
`Synth::bpm` is the **one** master tempo. Both `Sequencer::tick(bpm)` and
`DrumMachine::generate_sample(bpm)` receive it as a parameter so they are always
phase-locked. Changing BPM in any mode affects both sequencers immediately.

## Layout (all panels always visible)

```
Title bar (3 lines)   — focus indicator, seq/drum play status
Keyboard panel (12)   — piano + note highlights
Synth Seq panel (8)   — step grid (up to 32 steps)
Drum Machine (12)     — 8 track rows with volume
Status (4)            — wave, BPM, master vol, active notes
Help (remaining)      — context-sensitive key hints
```

Active focus is shown with a **cyan border** on the focused panel.
Inactive panels have a dim border but are always rendered.

## Focus (`AppMode` enum, cycle with Tab or F2)

| Focus | `↑/↓` | `←/→` | `Space` | piano keys |
|-------|--------|--------|---------|------------|
| `Play` (Keyboard) | volume | octave | — | play notes |
| `SynthSeq` | BPM | cursor | play/pause | set step note |
| `Drums` | select track | move step | toggle step | preview drums |

**Global keys** (any focus): Tab/F2 cycle focus, F1 waveform,
F3 drum play/stop, PageUp/PageDown BPM ±5, Esc quit.

In **Drums focus**, `-`/`=` adjust per-track volume (0–100 %).

## Per-track drum volume

Each `DrumTrack` has a `volume: f32` (default 0.85, range 0.0–1.0).
`DrumMachine::track_volume_up/down(track)` adjust it by ±0.05.
The volume is displayed in the drum grid as `VVV%` beside the mute indicator.
`App::drum_vol_up/down()` call through and update `status_msg`.

## Drum machine (`drums.rs`)

8 tracks, each a `DrumTrack`:
- `kind: DrumKind` — Kick / Snare / ClosedHat / OpenHat / Clap / LowTom / MidTom / HighTom
- `steps: Vec<bool>` — 8/16/24/32 steps (16th notes)
- `muted: bool`, `volume: f32`
- `fx: EffectChain` — per-track insert effects (currently empty)

`DrumMachine` maintains:
- A polyphonic `Vec<DrumVoice>` pool — all currently sounding hits
- A master `fx: EffectChain` for the summed drum bus
- Hi-hat choke: triggering ClosedHat kills all ringing OpenHat voices

All drum sounds are synthesized with XOR-shift noise and phase-accumulated oscillators
(no samples). Key parameters per sound:

| Sound | Technique |
|-------|-----------|
| Kick | Sine pitch sweep 150→50 Hz + transient click |
| Snare | Noise + 195 Hz body tone |
| C-Hat | Very short noise burst (~60 ms) |
| O-Hat | Longer noise decay (~380 ms), choked by C-Hat |
| Clap | 3 staggered noise bursts (0/9/17 ms) + decaying body |
| Toms | Sine pitch sweep + noise; different freq/decay per tom |

## Effects scaffold (`effects.rs`)

```rust
pub trait AudioEffect: Send {
    fn process(&mut self, sample: f32) -> f32;
    fn name(&self) -> &'static str;
    fn reset(&mut self);
}

pub struct EffectChain { pub effects: Vec<Box<dyn AudioEffect>> }
```

`EffectChain::process()` short-circuits to a direct return when empty (zero overhead).
Every instrument bus (`Synth::fx`, `DrumMachine::fx`) and every track (`DrumTrack::fx`)
already owns an `EffectChain`. To add an effect, implement the trait and push an instance.

## Melodic sequencer (`sequencer.rs`)

- `steps: Vec<Option<u8>>` — MIDI note per step (`None` = rest)
- 16th-note steps; step count cycles 8→16→24→32→8
- `tick(bpm)` called once per audio sample; returns `StepEvent{note_on, note_off}` at
  step boundaries
- Removing `bpm` from `Sequencer` and passing it at call-site was deliberate so BPM is
  controlled from one place (`Synth::bpm`)

## UI (`ui.rs`)

Three layouts, all with the same outer structure:
```
Title (3 lines)
Main panel — Piano | SynthSeq grid | Drum grid
Status (5 lines)   — wave, BPM, volume, playing notes
Help (remaining)   — mode-specific key hints
```

`draw_drums()` renders an 11-line block: 1 header + 1 step-number row + 8 track rows.
Each step cell is a colored `█`/`·` + space (2 chars). Beat groups of 4 are separated
by `┆`. Playhead = green bg, cursor = yellow bg, playhead+cursor = cyan bg.

## Key things to know for future work

- **Adding a new effect**: implement `AudioEffect`, push onto the relevant `EffectChain`.
  No other changes needed — the chain is already wired into every bus/track.
- **Adding a new drum sound**: add variant to `DrumKind::ALL`, implement a synthesis
  function in `DrumVoice`, add a `DrumTrack` in `DrumMachine::new()`.
- **Adding a new waveform**: extend `WaveType` enum in `synth.rs`.
- **MIDI/OSC input**: would hook into `app.rs` methods (`key_press`, `seq_set_note`,
  `drum_toggle_step`, etc.) — all side-effects go through `Arc<Mutex<Synth>>`.
- **Stereo**: `AudioEngine` already writes the same mono sample to all channels. A stereo
  `EffectChain` would need a new trait or a paired mono-chain approach.
- **The audio callback acquires the mutex on every frame.** If the UI thread holds the
  lock for too long, you will get audio dropouts. Keep lock durations short.
