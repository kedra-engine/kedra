//! Stable engine categories, independent of the trace backend's category representation.

/// Semantic scope category. Rendering means CPU-side rendering work, not GPU time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Category {
    /// Project/application-defined work.
    #[default]
    Application,
    /// Engine runtime work not covered by a more specific category.
    Runtime,
    /// Scheduler/executor internals; retained for detailed investigation.
    Scheduling,
    /// Retained user-interface work.
    Ui,
    /// CPU-side render preparation/submission.
    Rendering,
    /// Asset admission/loading/processing.
    Assets,
    /// Script execution and logical script operations.
    Scripting,
    /// Editor orchestration and authoring tools.
    Editor,
    /// Recording identity and explicit frame correlation.
    Metadata,
}

impl Category {
    /// Stable display/trace category name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Application => "Application",
            Self::Runtime => "Runtime",
            Self::Scheduling => "Scheduling",
            Self::Ui => "UI",
            Self::Rendering => "Rendering CPU",
            Self::Assets => "Assets",
            Self::Scripting => "Scripts",
            Self::Editor => "Editor",
            Self::Metadata => "Metadata",
        }
    }
}
