use super::*;

/// No Deserialize. Adapters must still authenticate and verify source/policy/reviewer;
/// private fields are not an authorization boundary.
pub struct ReviewDraft {
    pub(super) targets: Vec<ArtifactRef>,
    pub(super) evidence: Vec<ArtifactRef>,
    pub(super) judgment: ReviewJudgment,
}
pub struct HumanReviewEdit {
    pub(super) verdict: HumanReviewVerdict,
    pub(super) comment: String,
}
impl HumanReviewEdit {
    pub fn new(verdict: HumanReviewVerdict, comment: impl Into<String>) -> Result<Self> {
        let comment = comment.into();
        validate_comment(verdict, &comment)?;
        Ok(Self { verdict, comment })
    }
}
impl ReviewDraft {
    pub fn human(
        targets: Vec<ArtifactRef>,
        evidence: Vec<ArtifactRef>,
        policy: VersionedName,
        edit: HumanReviewEdit,
    ) -> Result<Self> {
        let judgment = ReviewJudgment::Human {
            policy,
            verdict: edit.verdict,
            comment: edit.comment,
        };
        validate(&targets, &evidence, &judgment)?;
        Ok(Self {
            targets,
            evidence,
            judgment,
        })
    }
    /// Trusted rule adapter only: records pinned attribution, not proof the rule ran on these bytes.
    /// A rerun is a new Review; an automatic result cannot be amended into a human approval.
    pub fn rule(
        targets: Vec<ArtifactRef>,
        evidence: Vec<ArtifactRef>,
        assessment: TaskAssessment,
    ) -> Result<Self> {
        let judgment = ReviewJudgment::Rule { assessment };
        validate(&targets, &evidence, &judgment)?;
        Ok(Self {
            targets,
            evidence,
            judgment,
        })
    }
}
fn validate_comment(verdict: HumanReviewVerdict, comment: &str) -> Result<()> {
    if comment.len() > 8192
        || comment
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
        || (verdict != HumanReviewVerdict::Approved && comment.trim().is_empty())
    {
        return Err(ReviewStoreError::InvalidInput);
    }
    Ok(())
}
pub(super) fn validate(
    targets: &[ArtifactRef],
    evidence: &[ArtifactRef],
    judgment: &ReviewJudgment,
) -> Result<()> {
    if targets.is_empty() || targets.len() > 32 || evidence.len() > 32 {
        return Err(ReviewStoreError::InvalidInput);
    }
    // Repeating a coordinate with a changed digest is not a different version.
    for refs in [targets, evidence] {
        for (index, reference) in refs.iter().enumerate() {
            if refs[..index]
                .iter()
                .any(|other| coordinates_match(reference, other))
            {
                return Err(ReviewStoreError::InvalidInput);
            }
        }
    }
    if targets.iter().any(|target| {
        evidence
            .iter()
            .any(|item| coordinates_match(target, item) && target != item)
    }) {
        return Err(ReviewStoreError::InvalidInput);
    }
    match judgment {
        ReviewJudgment::Human {
            verdict, comment, ..
        } => validate_comment(*verdict, comment),
        ReviewJudgment::Rule { assessment } => {
            let namespace = format!("{}.", assessment.definition.package.name);
            if !assessment.definition.name.as_str().starts_with(&namespace)
                || !assessment.policy.name.as_str().starts_with(&namespace)
                || !assessment
                    .evidence_schema
                    .name
                    .as_str()
                    .starts_with(&namespace)
                || assessment.outcome.feedback.len() > 64
                || assessment
                    .outcome
                    .feedback
                    .iter()
                    .any(|item| item.len() > 2048)
                || assessment
                    .outcome
                    .feedback
                    .iter()
                    .map(String::len)
                    .sum::<usize>()
                    > 16384
            {
                return Err(ReviewStoreError::InvalidInput);
            }
            Ok(())
        }
    }
}
fn coordinates_match(a: &ArtifactRef, b: &ArtifactRef) -> bool {
    a.workspace == b.workspace && a.artifact_id == b.artifact_id && a.revision_id == b.revision_id
}
