//! Lossless bridge from a validated finite metric artifact to its induced
//! finite topology.  The bridge never treats an arbitrary distance value or
//! vector as a topology; it requires the metric pack's validated artifact.

use crate::source_metric_pack::{MetricArtifact, MetricResult, MetricStatus};
use crate::source_topology_pack::{TopologyOperation, TopologyRequest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BridgeStatus {
    Complete,
    Ambiguous,
    Unsupported,
    Inconsistent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricTopologyBridgeResult {
    pub status: BridgeStatus,
    pub request: Option<TopologyRequest>,
    pub reasons: Vec<String>,
    pub provenance: Vec<String>,
    pub replay_hash: String,
}

fn digest<T: Serialize>(value: &T) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}

fn payload(result: &MetricTopologyBridgeResult) -> impl Serialize + '_ {
    (
        result.status,
        &result.request,
        &result.reasons,
        &result.provenance,
    )
}

fn output(
    status: BridgeStatus,
    request: Option<TopologyRequest>,
    reasons: Vec<String>,
    provenance: Vec<String>,
) -> MetricTopologyBridgeResult {
    let mut result = MetricTopologyBridgeResult {
        status,
        request,
        reasons,
        provenance,
        replay_hash: String::new(),
    };
    let replay_hash = digest(&payload(&result));
    result.replay_hash = replay_hash;
    result
}

fn induced_open_sets(
    points: &[String],
    distances: &[(String, String, i64)],
) -> Option<Vec<Vec<String>>> {
    let mut index = BTreeMap::new();
    for (position, point) in points.iter().enumerate() {
        index.insert(point.clone(), position);
    }
    let mut map = BTreeMap::new();
    let mut maximum = 0_i64;
    for (left, right, distance) in distances {
        let left_index = *index.get(left)?;
        let right_index = *index.get(right)?;
        map.insert(
            (left_index.min(right_index), left_index.max(right_index)),
            *distance,
        );
        maximum = maximum.max(*distance);
    }
    let pair = |left: usize, right: usize| {
        if left <= right {
            map.get(&(left, right)).copied()
        } else {
            map.get(&(right, left)).copied()
        }
    };
    let mut balls = Vec::new();
    for center in 0..points.len() {
        for radius in 1..=maximum.saturating_add(1) {
            let mut ball = 0_u16;
            for target in 0..points.len() {
                if pair(center, target)? < radius {
                    ball |= 1 << target;
                }
            }
            balls.push(ball);
        }
    }
    balls.sort_unstable();
    balls.dedup();
    let universe = 1_u16 << points.len();
    let mut open_sets = BTreeSet::new();
    for target in 0..universe {
        let mut open = true;
        for point in 0..points.len() {
            let point_bit = 1_u16 << point;
            if target & point_bit == 0 {
                continue;
            }
            if !balls
                .iter()
                .any(|ball| ball & point_bit != 0 && ball & !target == 0)
            {
                open = false;
                break;
            }
        }
        if open {
            open_sets.insert(
                (0..points.len())
                    .filter(|point| target & (1_u16 << point) != 0)
                    .map(|point| points[point].clone())
                    .collect::<Vec<_>>(),
            );
        }
    }
    Some(open_sets.into_iter().collect())
}

/// Derive the finite topology induced by a complete metric result.
pub fn metric_result_to_topology(result: &MetricResult) -> MetricTopologyBridgeResult {
    let provenance = result.provenance.clone();
    if !result.replay_verified() {
        return output(
            BridgeStatus::Inconsistent,
            None,
            vec!["metric result replay verification is required before composition".into()],
            provenance,
        );
    }
    match result.status {
        MetricStatus::Ambiguous => {
            return output(
                BridgeStatus::Ambiguous,
                None,
                vec!["metric semantics remain ambiguous".into()],
                provenance,
            )
        }
        MetricStatus::Complete => {}
        _ => {
            return output(
                BridgeStatus::Unsupported,
                None,
                vec!["only a complete validated metric can induce a topology".into()],
                provenance,
            )
        }
    }
    let Some(MetricArtifact::ValidatedMetric { points, distances }) = result.artifact.as_ref()
    else {
        return output(
            BridgeStatus::Unsupported,
            None,
            vec!["distance, scalar, and set artifacts do not carry a full metric carrier".into()],
            provenance,
        );
    };
    let distance_rows = distances
        .iter()
        .map(|entry| (entry.left.clone(), entry.right.clone(), entry.distance))
        .collect::<Vec<_>>();
    let Some(open_sets) = induced_open_sets(points, &distance_rows) else {
        return output(
            BridgeStatus::Inconsistent,
            None,
            vec!["metric distance table does not cover its declared carrier".into()],
            provenance,
        );
    };
    let request = TopologyRequest {
        operation: TopologyOperation::ValidateTopology,
        topology: "finite_topology_axioms".into(),
        points: points.clone(),
        open_sets,
        target_set: None,
        domain: "source_derived_finite_topology".into(),
        ambiguity: None,
        provenance: {
            let mut spans = provenance;
            spans.push("metric-to-induced-topology:open-balls-and-unions".into());
            spans
        },
    };
    output(
        BridgeStatus::Complete,
        Some(request),
        Vec::new(),
        result.provenance.clone(),
    )
}

impl MetricTopologyBridgeResult {
    pub fn replay_verified(&self) -> bool {
        self.replay_hash == digest(&payload(self)) && !self.provenance.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_metric_pack::{
        evaluate_metric, DistanceEntry, MetricDefinitionRecord, MetricOperation, MetricRequest,
        DOMAIN,
    };

    #[test]
    fn validated_metric_has_a_replayable_induced_topology_request() {
        let record = MetricDefinitionRecord {
            metric_id: "finite_metric_axioms".into(),
            aliases: vec!["finite metric".into()],
            domain: DOMAIN.into(),
            max_points: 8,
            axioms: vec![
                "nonnegative".into(),
                "identity".into(),
                "symmetry".into(),
                "triangle".into(),
            ],
            source: crate::source_formula_pack::SourceCitation {
                source_id: "test-source".into(),
                title: "test".into(),
                section: "metric".into(),
                url: "https://example.test".into(),
                license: "test".into(),
                retrieved_utc: "2026-08-16".into(),
                evidence_span: "finite metric axioms".into(),
            },
        };
        let result = evaluate_metric(
            &MetricRequest {
                operation: MetricOperation::ValidateMetric,
                metric: "finite_metric_axioms".into(),
                points: vec!["p0".into(), "p1".into(), "p2".into()],
                distances: vec![
                    DistanceEntry {
                        left: "p0".into(),
                        right: "p0".into(),
                        distance: 0,
                    },
                    DistanceEntry {
                        left: "p0".into(),
                        right: "p1".into(),
                        distance: 1,
                    },
                    DistanceEntry {
                        left: "p0".into(),
                        right: "p2".into(),
                        distance: 2,
                    },
                    DistanceEntry {
                        left: "p1".into(),
                        right: "p1".into(),
                        distance: 0,
                    },
                    DistanceEntry {
                        left: "p1".into(),
                        right: "p2".into(),
                        distance: 1,
                    },
                    DistanceEntry {
                        left: "p2".into(),
                        right: "p2".into(),
                        distance: 0,
                    },
                ],
                center: None,
                target: None,
                radius: None,
                domain: DOMAIN.into(),
                ambiguity: None,
                provenance: vec!["test-metric".into()],
            },
            &[record],
        );
        let bridge = metric_result_to_topology(&result);
        assert_eq!(bridge.status, BridgeStatus::Complete);
        assert!(bridge.request.is_some());
        assert!(bridge.replay_verified());
    }
}
