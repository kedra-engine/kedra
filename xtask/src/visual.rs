//! Repository CLI presentation only; shared comparison lives in kedra_test.
use std::path::PathBuf;

#[derive(clap::Args, Default)]
#[group(id = "visual_options")]
pub(crate) struct Options {
    /// Inspect original Figma PNGs without claiming native acceptance.
    #[arg(long, conflicts_with_all = ["config", "actual"])]
    inspect_reference: Option<PathBuf>,
    /// Versioned visual comparison JSON.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Actual capture evidence JSON.
    #[arg(long)]
    actual: Option<PathBuf>,
    /// New artifact directory; existing evidence is never overwritten.
    #[arg(long)]
    output: Option<PathBuf>,
}

impl Options {
    pub fn is_present(&self) -> bool {
        self.inspect_reference.is_some()
            || self.config.is_some()
            || self.actual.is_some()
            || self.output.is_some()
    }
}

pub(crate) fn run(options: &Options, interactive: bool) -> crate::Result<()> {
    let output = options
        .output
        .as_deref()
        .ok_or("visual requires --output NEW_DIRECTORY")?;
    let mut session = crate::output::Session::new(interactive, "visual")?;
    let result = session.stage(
        1,
        1,
        "Visual integrity/comparison (not design acceptance)",
        |session| {
            session.message("Validating policy, evidence and independent reference…")?;
            let result = if let Some(manifest) = &options.inspect_reference {
                kedra_test::visual::inspect_reference(manifest, output)
            } else {
                kedra_test::visual::verify(
                    options
                        .config
                        .as_deref()
                        .ok_or("visual requires --config")?,
                    options
                        .actual
                        .as_deref()
                        .ok_or("visual requires --actual evidence.json")?,
                    output,
                )
            };
            session.message(&format!("Artifacts: {}", output.display()))?;
            result
        },
    );
    session.finish(1)?;
    result
}
