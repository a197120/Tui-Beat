/// Mono audio effect: one sample in, one sample out.
#[allow(dead_code)]
///
/// All implementations must be `Send` so they can live inside the audio thread
/// (behind `Arc<Mutex<Synth>>`).  Stereo can be modelled as two independent
/// mono effects or as a future specialisation — that's a later concern.
pub trait AudioEffect: Send {
    fn process(&mut self, sample: f32) -> f32;
    fn name(&self) -> &'static str;
    /// Reset all internal state (clear delay lines, reset envelopes, etc.).
    fn reset(&mut self);
}

/// A serial chain of effects applied to a mono signal.
///
/// When the chain is empty the audio passes through completely unchanged,
/// so there is zero CPU overhead until effects are actually inserted.
pub struct EffectChain {
    pub effects: Vec<Box<dyn AudioEffect>>,
}

#[allow(dead_code)]
impl EffectChain {
    pub fn new() -> Self {
        Self { effects: Vec::new() }
    }

    #[inline]
    pub fn process(&mut self, sample: f32) -> f32 {
        if self.effects.is_empty() {
            return sample;
        }
        self.effects.iter_mut().fold(sample, |s, fx| fx.process(s))
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    pub fn reset_all(&mut self) {
        for fx in &mut self.effects {
            fx.reset();
        }
    }
}

impl Default for EffectChain {
    fn default() -> Self {
        Self::new()
    }
}
