//! Finding-fingerprint deduplication truth-table tests.

use ae_sdd_contracts::review::{ReviewFinding, ReviewFindingSeverity};
use ae_sdd_contracts::{BoundedText, ReasonCode};
use ae_sdd_review::{ReviewSupervisorError, dedup_findings, finding_fingerprint};

fn finding(code: &str, severity: ReviewFindingSeverity, summary: &str) -> ReviewFinding {
    ReviewFinding::new(
        ReasonCode::new(code).unwrap(),
        severity,
        BoundedText::<1024>::new(summary).unwrap(),
    )
}

#[test]
fn identical_findings_collapse_to_one() {
    let a = finding("RC-1", ReviewFindingSeverity::Major, "summary");
    let deduped = dedup_findings(&[a.clone(), a.clone(), a.clone()]).unwrap();
    assert_eq!(deduped.len(), 1);
    assert_eq!(finding_fingerprint(&deduped[0]), finding_fingerprint(&a));
}

#[test]
fn severity_difference_produces_distinct_fingerprints() {
    let blocker = finding("RC-1", ReviewFindingSeverity::Blocker, "summary");
    let major = finding("RC-1", ReviewFindingSeverity::Major, "summary");
    let minor = finding("RC-1", ReviewFindingSeverity::Minor, "summary");
    let deduped = dedup_findings(&[blocker.clone(), major.clone(), minor.clone()]).unwrap();
    assert_eq!(deduped.len(), 3);
}

#[test]
fn summary_difference_produces_distinct_fingerprints() {
    let a = finding("RC-1", ReviewFindingSeverity::Major, "summary-a");
    let b = finding("RC-1", ReviewFindingSeverity::Major, "summary-b");
    let deduped = dedup_findings(&[a.clone(), b.clone()]).unwrap();
    assert_eq!(deduped.len(), 2);
}

#[test]
fn code_difference_produces_distinct_fingerprints() {
    let a = finding("RC-1", ReviewFindingSeverity::Major, "summary");
    let b = finding("RC-2", ReviewFindingSeverity::Major, "summary");
    let deduped = dedup_findings(&[a.clone(), b.clone()]).unwrap();
    assert_eq!(deduped.len(), 2);
}

#[test]
fn first_seen_order_is_preserved() {
    let a = finding("RC-A", ReviewFindingSeverity::Major, "first");
    let b = finding("RC-B", ReviewFindingSeverity::Major, "second");
    let c = finding("RC-C", ReviewFindingSeverity::Major, "third");
    let deduped = dedup_findings(&[a.clone(), b.clone(), c.clone(), a.clone(), b.clone()]).unwrap();
    assert_eq!(deduped.len(), 3);
    assert_eq!(finding_fingerprint(&deduped[0]), finding_fingerprint(&a));
    assert_eq!(finding_fingerprint(&deduped[1]), finding_fingerprint(&b));
    assert_eq!(finding_fingerprint(&deduped[2]), finding_fingerprint(&c));
}

#[test]
fn exceeding_max_findings_is_rejected() {
    let one = finding("RC-1", ReviewFindingSeverity::Major, "summary");
    let many: Vec<ReviewFinding> = (0..200)
        .map(|index| {
            ReviewFinding::new(
                ReasonCode::new(format!("RC-{index}")).unwrap(),
                ReviewFindingSeverity::Major,
                BoundedText::<1024>::new("summary").unwrap(),
            )
        })
        .collect();
    let _ = one;
    let err = dedup_findings(&many).unwrap_err();
    assert!(matches!(
        err,
        ReviewSupervisorError::InvalidCollectedInput(_)
    ));
}
