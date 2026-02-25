// ── Scale definitions ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scale {
    Off,
    Major,
    Minor,
    PentaMajor,
    PentaMinor,
    Blues,
    Dorian,
    Mixolydian,
}

impl Scale {
    pub const ALL: [Scale; 8] = [
        Self::Off,
        Self::Major,
        Self::Minor,
        Self::PentaMajor,
        Self::PentaMinor,
        Self::Blues,
        Self::Dorian,
        Self::Mixolydian,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Off        => "Off",
            Self::Major      => "Major",
            Self::Minor      => "Minor",
            Self::PentaMajor => "Penta Maj",
            Self::PentaMinor => "Penta Min",
            Self::Blues      => "Blues",
            Self::Dorian     => "Dorian",
            Self::Mixolydian => "Mixolydian",
        }
    }

    /// Abbreviated name for the status bar.
    pub fn short_name(self) -> &'static str {
        match self {
            Self::Off        => "Off",
            Self::Major      => "Maj",
            Self::Minor      => "Min",
            Self::PentaMajor => "PMaj",
            Self::PentaMinor => "PMin",
            Self::Blues      => "Blues",
            Self::Dorian     => "Dor",
            Self::Mixolydian => "Mix",
        }
    }

    /// Semitone intervals from the root note (root = 0).
    pub fn intervals(self) -> &'static [u8] {
        match self {
            Self::Off        => &[0,1,2,3,4,5,6,7,8,9,10,11],
            Self::Major      => &[0,2,4,5,7,9,11],
            Self::Minor      => &[0,2,3,5,7,8,10],
            Self::PentaMajor => &[0,2,4,7,9],
            Self::PentaMinor => &[0,3,5,7,10],
            Self::Blues      => &[0,3,5,6,7,10],
            Self::Dorian     => &[0,2,3,5,7,9,10],
            Self::Mixolydian => &[0,2,4,5,7,9,10],
        }
    }

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|&s| s == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

// ── Quantizer ─────────────────────────────────────────────────────────────────

pub struct ScaleQuantizer {
    pub scale: Scale,
    pub root:  u8,   // 0 = C, 1 = C#, … 11 = B
}

impl ScaleQuantizer {
    pub fn new() -> Self {
        Self { scale: Scale::Off, root: 0 }
    }

    pub fn active(&self) -> bool {
        self.scale != Scale::Off
    }

    /// Snap `note` to the nearest MIDI note in the selected scale.
    /// When scale is Off, returns `note` unchanged.
    pub fn quantize(&self, note: u8) -> u8 {
        if self.scale == Scale::Off { return note; }
        let intervals = self.scale.intervals();
        let root  = self.root as i32;
        let note  = note as i32;

        // Semitone distance from root within one octave
        let rel = (note - root).rem_euclid(12);

        // For each scale interval, check whether snapping to it (in the current
        // octave, one below, or one above) is closer than the current best.
        let mut best_offset = 0i32;
        let mut best_dist   = i32::MAX;
        for &iv in intervals {
            let iv = iv as i32;
            for &candidate in &[iv - rel, iv - 12 - rel, iv + 12 - rel] {
                if candidate.abs() < best_dist {
                    best_dist   = candidate.abs();
                    best_offset = candidate;
                }
            }
        }

        (note + best_offset).clamp(0, 127) as u8
    }

    pub fn root_name(&self) -> &'static str {
        ["C","C#","D","D#","E","F","F#","G","G#","A","A#","B"][self.root as usize]
    }

    pub fn cycle_root(&mut self) {
        self.root = (self.root + 1) % 12;
    }
}
