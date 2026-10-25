//! Keyframed part-transform tracks on the frame index timeline.
//!
//! A curve names one part and one channel. Keys sit on integer frame indices,
//! the same axis as animation tags and `setFrameDelay`. Sampling takes a frame
//! position that may fall between those indices. It does not read `delayMs`
//! and it does not write pixels. The number is the pose relative to the part's
//! rest origin. [`crate::Document::bake`] reads that sample and warps rest ink.
//! It does not call `rotatePart`. Applying the sample again with `rotatePart`
//! on a frame that was already transformed stacks.
//!
//! The ease on a key is how the value leaves that key toward the next one.
//! With `t` in `[0, 1]` along the frame-index span:
//!
//! - `linear`: `u = t`
//! - `easeInOut`: `u = t² (3 − 2t)` (Hermite smoothstep)
//! - `hold`: `u = 0` until the next key, which is returned exactly on its frame
//!
//! At `t = 1/2`, smoothstep is `1/2`, so an ease-in-out midpoint matches linear.
//! At `t = 1/4`, smoothstep is `5/32`. Rotation is interpolated in stored
//! centi-degrees, then divided by 100. The blend follows the stored numbers,
//! not the short arc across ±180°.

use crate::error::KernelError;
use crate::part::Part;
use crate::part_xform;

/// One parameter a curve can store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveChannel {
    /// Clockwise degrees from the rest pose. Stored as centi-degrees.
    Rotation,
    /// Whole cells added to the rest origin `x`.
    TranslateX,
    /// Whole cells added to the rest origin `y`.
    TranslateY,
    /// `0` rest, `1` mirrored left-right from rest. Hold only.
    FlipHorizontal,
    /// `0` rest, `1` mirrored top-bottom from rest. Hold only.
    FlipVertical,
}

impl CurveChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rotation => "rotation",
            Self::TranslateX => "translateX",
            Self::TranslateY => "translateY",
            Self::FlipHorizontal => "flipHorizontal",
            Self::FlipVertical => "flipVertical",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "rotation" => Some(Self::Rotation),
            "translateX" => Some(Self::TranslateX),
            "translateY" => Some(Self::TranslateY),
            "flipHorizontal" => Some(Self::FlipHorizontal),
            "flipVertical" => Some(Self::FlipVertical),
            _ => None,
        }
    }

    /// Ease used when a key omits one. Flip channels step. The others blend.
    pub fn default_ease(self) -> Ease {
        match self {
            Self::FlipHorizontal | Self::FlipVertical => Ease::Hold,
            _ => Ease::Linear,
        }
    }

    pub fn allows(self, ease: Ease) -> bool {
        match self {
            Self::FlipHorizontal | Self::FlipVertical => ease == Ease::Hold,
            _ => true,
        }
    }

    /// Canonical stored integer for a tool or file value.
    pub fn store(self, value: f64) -> Result<i32, KernelError> {
        match self {
            Self::Rotation => {
                part_xform::quantize_degrees(value).map_err(|_| KernelError::CurveValue)
            }
            Self::TranslateX | Self::TranslateY => store_translate(value),
            Self::FlipHorizontal | Self::FlipVertical => store_flip(value),
        }
    }

    /// Tool and file number for a stored integer.
    pub fn decode(self, stored: i32) -> f64 {
        match self {
            Self::Rotation => f64::from(stored) / 100.0,
            _ => f64::from(stored),
        }
    }
}

fn store_translate(value: f64) -> Result<i32, KernelError> {
    if !value.is_finite()
        || value.fract() != 0.0
        || value < f64::from(i32::MIN)
        || value > f64::from(i32::MAX)
    {
        return Err(KernelError::CurveValue);
    }
    Ok(value as i32)
}

fn store_flip(value: f64) -> Result<i32, KernelError> {
    if value == 0.0 {
        Ok(0)
    } else if value == 1.0 {
        Ok(1)
    } else {
        Err(KernelError::CurveValue)
    }
}

/// How a key blends toward the next key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ease {
    Linear,
    EaseInOut,
    Hold,
}

impl Ease {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::EaseInOut => "easeInOut",
            Self::Hold => "hold",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "linear" => Some(Self::Linear),
            "easeInOut" => Some(Self::EaseInOut),
            "hold" => Some(Self::Hold),
            _ => None,
        }
    }

    /// Maps a segment parameter `t` in `[0, 1]` onto the blend weight `u`.
    pub fn apply(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            // Hermite smoothstep: t² (3 − 2t). t = 1/2 → 1/2. t = 1/4 → 5/32.
            Self::EaseInOut => t * t * (3.0 - 2.0 * t),
            Self::Hold => 0.0,
        }
    }
}

/// One key. `value` is the channel's stored integer, not the tool-facing number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurveKey {
    pub frame: usize,
    pub value: i32,
    /// Blend from this key to the next. Unused while this key is the last one.
    pub ease: Ease,
}

/// One part channel, keys in increasing frame order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Curve {
    pub part: String,
    pub channel: CurveChannel,
    pub keys: Vec<CurveKey>,
}

/// Why a sample returned this number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleHit {
    /// The position landed on a key.
    Key,
    /// The position is outside the keyed span, so the end key is held.
    Clamp,
    Linear,
    EaseInOut,
    Hold,
}

impl SampleHit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Key => "key",
            Self::Clamp => "clamp",
            Self::Linear => "linear",
            Self::EaseInOut => "easeInOut",
            Self::Hold => "hold",
        }
    }

    fn from_ease(ease: Ease) -> Self {
        match ease {
            Ease::Linear => Self::Linear,
            Ease::EaseInOut => Self::EaseInOut,
            Ease::Hold => Self::Hold,
        }
    }
}

/// Interpolated channel value at one frame position.
#[derive(Debug, Clone, PartialEq)]
pub struct CurveSample {
    pub value: f64,
    pub hit: SampleHit,
    pub left_frame: usize,
    pub right_frame: Option<usize>,
}

/// What [`crate::Document::set_curve_key`] stored.
#[derive(Debug, Clone, PartialEq)]
pub struct CurveWrite {
    pub index: usize,
    pub key_index: usize,
    pub value: f64,
    pub ease: Ease,
}

/// What [`crate::Document::remove_curve_key`] removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurveRemoval {
    pub index: usize,
    pub part: String,
    pub channel: CurveChannel,
    pub frame: usize,
    /// True when that key was the curve's last one and the curve was dropped.
    pub removed_curve: bool,
}

/// Every keyed channel of one part at one frame position.
///
/// A missing channel was not keyed. It is not a zero pose.
#[derive(Debug, Clone, PartialEq)]
pub struct PartSample {
    pub part: String,
    pub frame: f64,
    pub rotation: Option<f64>,
    pub translate_x: Option<f64>,
    pub translate_y: Option<f64>,
    pub flip_horizontal: Option<f64>,
    pub flip_vertical: Option<f64>,
}

/// Sample `keys` at `frame`. `keys` is non-empty and sorted.
pub(crate) fn sample_keys(channel: CurveChannel, keys: &[CurveKey], frame: f64) -> CurveSample {
    debug_assert!(!keys.is_empty(), "a curve keeps at least one key");
    if let Some(key) = keys.iter().find(|key| on_key(frame, key.frame)) {
        return CurveSample {
            value: channel.decode(key.value),
            hit: SampleHit::Key,
            left_frame: key.frame,
            right_frame: None,
        };
    }
    if frame < keys[0].frame as f64 {
        return held(channel, &keys[0]);
    }
    let last = keys.last().expect("curve has a key");
    if frame > last.frame as f64 {
        return held(channel, last);
    }
    let right_index = keys
        .iter()
        .position(|key| key.frame as f64 > frame)
        .expect("frame sits under the last key");
    let left = &keys[right_index - 1];
    let right = &keys[right_index];
    let span = (right.frame - left.frame) as f64;
    let t = (frame - left.frame as f64) / span;
    let u = left.ease.apply(t);
    let stored = f64::from(left.value) + (f64::from(right.value) - f64::from(left.value)) * u;
    let value = match channel {
        CurveChannel::Rotation => stored / 100.0,
        _ => stored,
    };
    CurveSample {
        value,
        hit: SampleHit::from_ease(left.ease),
        left_frame: left.frame,
        right_frame: Some(right.frame),
    }
}

fn held(channel: CurveChannel, key: &CurveKey) -> CurveSample {
    CurveSample {
        value: channel.decode(key.value),
        hit: SampleHit::Clamp,
        left_frame: key.frame,
        right_frame: None,
    }
}

fn on_key(frame: f64, key_frame: usize) -> bool {
    (frame - key_frame as f64).abs() <= 1e-9
}

/// Keys at or after `index` move right by one. Order stays sorted.
pub(crate) fn curves_after_insert(curves: &[Curve], index: usize) -> Vec<Curve> {
    curves
        .iter()
        .map(|curve| {
            let mut next = curve.clone();
            for key in &mut next.keys {
                if key.frame >= index {
                    key.frame += 1;
                }
            }
            next
        })
        .collect()
}

/// The key on `index` is dropped. Later keys shift left. An empty curve is dropped.
pub(crate) fn curves_after_remove(curves: &[Curve], index: usize) -> Vec<Curve> {
    let mut out = Vec::with_capacity(curves.len());
    for curve in curves {
        let mut keys = Vec::with_capacity(curve.keys.len());
        for key in &curve.keys {
            if key.frame < index {
                keys.push(key.clone());
            } else if key.frame > index {
                let mut next = key.clone();
                next.frame -= 1;
                keys.push(next);
            }
        }
        if !keys.is_empty() {
            out.push(Curve {
                keys,
                ..curve.clone()
            });
        }
    }
    out
}

/// Curves that named `from` now name `to`.
pub(crate) fn curves_after_rename(curves: &[Curve], from: &str, to: &str) -> Vec<Curve> {
    curves
        .iter()
        .map(|curve| {
            let mut next = curve.clone();
            if next.part == from {
                next.part = to.to_string();
            }
            next
        })
        .collect()
}

/// Drops curves whose part is no longer in `parts`.
pub(crate) fn curves_for_parts(curves: &[Curve], parts: &[Part]) -> Vec<Curve> {
    curves
        .iter()
        .filter(|curve| parts.iter().any(|part| part.name == curve.part))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(frame: usize, value: i32, ease: Ease) -> CurveKey {
        CurveKey { frame, value, ease }
    }

    #[test]
    fn ease_in_out_midpoint_matches_linear_and_the_quarter_does_not() {
        let eased = vec![key(0, 0, Ease::EaseInOut), key(4, 9_000, Ease::Linear)];
        let mid = sample_keys(CurveChannel::Rotation, &eased, 2.0);
        assert_eq!(mid.value, 45.0);
        assert_eq!(mid.hit, SampleHit::EaseInOut);
        assert_eq!(Ease::EaseInOut.apply(0.5), 0.5);
        let quarter = sample_keys(CurveChannel::Rotation, &eased, 1.0);
        assert_eq!(quarter.value, 14.0625);
        assert_eq!(Ease::EaseInOut.apply(0.25), 0.15625);

        let linear = vec![key(0, 0, Ease::Linear), key(4, 9_000, Ease::Linear)];
        assert_eq!(
            sample_keys(CurveChannel::Rotation, &linear, 2.0).value,
            45.0
        );
        assert_eq!(
            sample_keys(CurveChannel::Rotation, &linear, 1.0).value,
            22.5
        );
        assert_eq!(
            sample_keys(CurveChannel::Rotation, &linear, 0.0).hit,
            SampleHit::Key
        );
        assert_eq!(
            sample_keys(CurveChannel::Rotation, &linear, -1.0).hit,
            SampleHit::Clamp
        );
    }

    #[test]
    fn hold_keeps_the_left_value_until_the_next_key() {
        let keys = vec![key(0, 0, Ease::Hold), key(4, 1, Ease::Hold)];
        let between = sample_keys(CurveChannel::FlipHorizontal, &keys, 2.0);
        assert_eq!(between.value, 0.0);
        assert_eq!(between.hit, SampleHit::Hold);
        let at_end = sample_keys(CurveChannel::FlipHorizontal, &keys, 4.0);
        assert_eq!(at_end.value, 1.0);
        assert_eq!(at_end.hit, SampleHit::Key);
    }

    #[test]
    fn insert_and_remove_shift_keys_like_frame_indices() {
        let curves = vec![Curve {
            part: "gun".to_string(),
            channel: CurveChannel::Rotation,
            keys: vec![key(0, 0, Ease::Linear), key(2, 9_000, Ease::Linear)],
        }];
        let inserted = curves_after_insert(&curves, 1);
        assert_eq!(inserted[0].keys[0].frame, 0);
        assert_eq!(inserted[0].keys[1].frame, 3);
        let removed = curves_after_remove(&curves, 2);
        assert_eq!(removed[0].keys.len(), 1);
        assert_eq!(removed[0].keys[0].frame, 0);
        assert!(curves_after_remove(&curves, 0)
            .iter()
            .all(|curve| { curve.keys.len() == 1 && curve.keys[0].frame == 1 }));
        let only = vec![Curve {
            part: "gun".to_string(),
            channel: CurveChannel::TranslateX,
            keys: vec![key(1, 2, Ease::Linear)],
        }];
        assert!(curves_after_remove(&only, 1).is_empty());
    }
}
