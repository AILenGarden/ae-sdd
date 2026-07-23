#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProcessPhase {
    Initialized,
    RouteSelected,
    RequirementAnalyzed,
    DrGenerated,
    StoryGenerated,
    TestcaseGenerated,
    CodingProcess,
    Coding,
    TestRunning,
    CodeReviewed,
    Completed,
    Paused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WorkScale {
    Large,
    Medium,
    Small,
    Micro,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DesignRoute {
    Dr,
    Story,
    CodingPlan,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_phase_is_vocabulary_not_a_transition_table() {
        let phases = [
            ProcessPhase::Initialized,
            ProcessPhase::RouteSelected,
            ProcessPhase::RequirementAnalyzed,
            ProcessPhase::DrGenerated,
            ProcessPhase::StoryGenerated,
            ProcessPhase::TestcaseGenerated,
            ProcessPhase::CodingProcess,
            ProcessPhase::Coding,
            ProcessPhase::TestRunning,
            ProcessPhase::CodeReviewed,
            ProcessPhase::Completed,
            ProcessPhase::Paused,
        ];

        assert_eq!(phases.len(), 12);
    }
}
