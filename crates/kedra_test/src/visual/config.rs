//! Shared strict comparison policy. Rectangles are logical xywh, not endpoints.
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf};

const MILLION: u64 = 1_000_000;
pub(super) const MAX_PIXELS: u64 = 8_388_608;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Profile {
    pub id: String,
    pub platform: String,
    pub renderer: String,
    pub environment: String,
    pub logical_size: [u32; 2],
    pub dpr: f64,
    /// Untagged SDR bytes are explicitly assumed sRGB; this is not color certification.
    pub color_space: ColorSpace,
    pub motion: Motion,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(super) enum ColorSpace {
    SrgbAssumed,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Motion {
    Normal,
    Reduced,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Config {
    pub schema: u32,
    pub scenario: String,
    pub fixture: String,
    pub profile: Profile,
    pub reference: Reference,
    pub animation: Animation,
    pub comparison: Comparison,
    pub exclusions: Vec<Exclusion>,
    pub critical_regions: Vec<Region>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reference {
    pub image: PathBuf,
    pub sha256: String,
    pub source: String,
    pub kind: ReferenceKind,
    pub approval: Approval,
    pub export_size: [u32; 2],
    /// Physical xywh; only the reference can be cropped, with an explicit reason.
    pub crop: [u32; 4],
    pub crop_reason: String,
    /// Optional fixed fixture canvas for an isolated component export. The actual
    /// frame is never cropped/moved; origin is declared independently of it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas: Option<ReferenceCanvas>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReferenceCanvas {
    /// Physical top-left placement of the reference crop, without scaling.
    pub origin: [u32; 2],
    /// Explicit opaque fixture background, RGBA8 in the declared SDR space.
    pub background: [u8; 4],
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ReferenceKind {
    DesignReference,
    RegressionGolden,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Approval {
    Pending,
    HumanReviewed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Animation {
    /// One frame; no claim about animation correctness. Volatility must be explicit.
    Static,
    /// Only declared animation interiors are excluded; not a motion test.
    ExcludedInteriors,
    /// Parsed to return an explicit unsupported result, never downgraded to static.
    Keyframes,
    Live,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Comparison {
    pub normalized_yiq_tolerance: f32,
    pub budget: Budget,
    pub max_excluded_ppm: u32,
    pub allow_uniform_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Budget {
    pub max_changed_pixels: u64,
    pub max_changed_ppm: u32,
}

impl Budget {
    pub fn passes(&self, changed: u64, eligible: u64) -> bool {
        eligible > 0
            && changed <= self.max_changed_pixels
            && changed * MILLION <= eligible * u64::from(self.max_changed_ppm)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Exclusion {
    pub name: String,
    pub reason: String,
    pub kind: ExclusionKind,
    pub rect: [f64; 4],
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ExclusionKind {
    VolatileData,
    AnimationInterior,
    /// Explicit owner-reviewed variance; unlike volatility, may touch a critical
    /// region while its remaining pixels stay under the unchanged region budget.
    ApprovedVisualVariance,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Region {
    pub name: String,
    pub rect: [f64; 4],
    pub budget: Budget,
}

/// Half-open physical bounds. Same immutable resolution applies to both images.
#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct Bounds(pub u32, pub u32, pub u32, pub u32);

impl Bounds {
    pub fn overlaps(self, other: Self) -> bool {
        self.0 < other.2 && other.0 < self.2 && self.1 < other.3 && other.1 < self.3
    }
}

impl Profile {
    pub fn physical_size(&self) -> crate::Result<[u32; 2]> {
        for value in [&self.id, &self.platform, &self.renderer, &self.environment] {
            nonempty(value)?;
        }
        if !self.dpr.is_finite() || !(0.5..=4.).contains(&self.dpr) {
            return Err("DPR must be finite and in 0.5..=4".into());
        }
        let size = self.logical_size.map(|n| f64::from(n) * self.dpr);
        if size
            .iter()
            .any(|n| *n < 1. || *n > 8192. || n.fract() != 0.)
        {
            return Err("viewport must resolve to integral physical dimensions in 1..=8192".into());
        }
        let size = size.map(|n| n as u32);
        if u64::from(size[0]) * u64::from(size[1]) > MAX_PIXELS {
            return Err("viewport exceeds visual-test memory limit".into());
        }
        Ok(size)
    }

    pub fn resolve(&self, rect: [f64; 4]) -> crate::Result<Bounds> {
        let [x, y, width, height] = rect;
        if rect.iter().any(|n| !n.is_finite())
            || x < 0.
            || y < 0.
            || width <= 0.
            || height <= 0.
            || x + width > f64::from(self.logical_size[0])
            || y + height > f64::from(self.logical_size[1])
        {
            return Err("logical rectangle must be nonempty and inside viewport".into());
        }
        Ok(Bounds(
            (x * self.dpr).floor() as u32,
            (y * self.dpr).floor() as u32,
            ((x + width) * self.dpr).ceil() as u32,
            ((y + height) * self.dpr).ceil() as u32,
        ))
    }
}

pub(super) fn nonempty(value: &str) -> crate::Result<()> {
    if value.trim().is_empty() || value.len() > 4096 {
        return Err("text must be nonempty and at most 4096 bytes".into());
    }
    Ok(())
}

impl Config {
    pub fn validate(&self) -> crate::Result<()> {
        if self.schema != 1 {
            return Err("unsupported visual configuration schema".into());
        }
        nonempty(&self.scenario)?;
        nonempty(&self.fixture)?;
        let size = self.profile.physical_size()?;
        let reference = &self.reference;
        nonempty(&reference.source)?;
        nonempty(&reference.crop_reason)?;
        if reference.sha256.len() != 64 || !reference.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("reference must pin a SHA-256 digest".into());
        }
        let [x, y, w, h] = reference.crop;
        if w == 0
            || h == 0
            || u64::from(x) + u64::from(w) > u64::from(reference.export_size[0])
            || u64::from(y) + u64::from(h) > u64::from(reference.export_size[1])
            || reference.export_size.contains(&0)
            || u64::from(reference.export_size[0]) * u64::from(reference.export_size[1])
                > MAX_PIXELS
        {
            return Err("reference crop/export dimensions do not match viewport".into());
        }
        if let Some(canvas) = &reference.canvas {
            if canvas.background[3] != 255
                || u64::from(canvas.origin[0]) + u64::from(w) > u64::from(size[0])
                || u64::from(canvas.origin[1]) + u64::from(h) > u64::from(size[1])
            {
                return Err(
                    "reference canvas requires opaque background and unclipped fixed placement"
                        .into(),
                );
            }
        } else if [w, h] != size {
            return Err("reference crop/export dimensions do not match viewport".into());
        }
        let comparison = &self.comparison;
        if !comparison.normalized_yiq_tolerance.is_finite()
            || !(0. ..=1.).contains(&comparison.normalized_yiq_tolerance)
            || comparison.max_excluded_ppm >= 1_000_000
            || comparison.budget.max_changed_ppm > 1_000_000
        {
            return Err("invalid color tolerance, pixel budget or exclusion limit".into());
        }
        if let Some(reason) = &comparison.allow_uniform_reason {
            nonempty(reason)?;
        }
        if self.exclusions.len() > 128 || self.critical_regions.len() > 128 {
            return Err("at most 128 exclusions and critical regions are supported".into());
        }
        let mut names = HashSet::new();
        let mut masks = Vec::new();
        let mut animated = false;
        for exclusion in &self.exclusions {
            nonempty(&exclusion.name)?;
            nonempty(&exclusion.reason)?;
            if !names.insert(&exclusion.name) {
                return Err("duplicate region name".into());
            }
            let approved = matches!(exclusion.kind, ExclusionKind::ApprovedVisualVariance);
            if approved && !matches!(reference.approval, Approval::HumanReviewed) {
                return Err("approved visual variance requires human-reviewed acceptance".into());
            }
            masks.push((self.profile.resolve(exclusion.rect)?, approved));
            animated |= matches!(exclusion.kind, ExclusionKind::AnimationInterior);
        }
        if animated != matches!(self.animation, Animation::ExcludedInteriors) {
            return Err(
                "animation interiors require the explicit excluded_interiors mode and vice versa"
                    .into(),
            );
        }
        for region in &self.critical_regions {
            nonempty(&region.name)?;
            if !names.insert(&region.name) {
                return Err("duplicate region name".into());
            }
            let bounds = self.profile.resolve(region.rect)?;
            if region.budget.max_changed_ppm > 1_000_000
                || masks
                    .iter()
                    .any(|(b, approved)| !approved && bounds.overlaps(*b))
            {
                return Err("invalid critical budget or exclusion overlaps critical region".into());
            }
        }
        Ok(())
    }
}
