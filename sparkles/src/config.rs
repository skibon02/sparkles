use crate::FinalizeGuard;

#[derive(Default)]
pub struct SparklesConfigBuilder {
    // todo
}

impl SparklesConfigBuilder {
    /// Initialize Sparkles.
    ///
    /// Must be called from main thread!
    ///
    /// Reinit is not supported, configuration is applied on the first init call!
    pub fn default_init() -> FinalizeGuard {
        super::init(SparklesConfigBuilder::default());
        FinalizeGuard
    }
}