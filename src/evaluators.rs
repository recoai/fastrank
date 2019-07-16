use crate::dataset::*;
use crate::qrel::QuerySetJudgments;
use ordered_float::NotNan;
use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(PartialEq, PartialOrd, Eq)]
pub struct RankedInstance {
    pub score: NotNan<f64>,
    pub gain: NotNan<f32>,
    pub identifier: u32,
}

/// Natural sort: first by socre, descending, then by gain ascending (yielding pessimistic scores on ties), finally by identifier.
impl Ord for RankedInstance {
    fn cmp(&self, other: &RankedInstance) -> Ordering {
        let cmp = other.score.cmp(&self.score);
        if cmp != Ordering::Equal {
            return cmp;
        }
        let cmp = self.gain.cmp(&other.gain);
        if cmp != Ordering::Equal {
            return cmp;
        }
        self.identifier.cmp(&other.identifier)
    }
}

impl RankedInstance {
    pub fn new(score: NotNan<f64>, gain: NotNan<f32>, identifier: u32) -> Self {
        Self {
            score,
            gain,
            identifier,
        }
    }
    pub fn is_relevant(&self) -> bool {
        self.gain.into_inner() > 0.0
    }
}

pub trait Evaluator {
    fn name(&self) -> String;
    fn score(&self, qid: &str, ranked_list: &[RankedInstance]) -> f64;
}

pub struct ReciprocalRank;

impl Evaluator for ReciprocalRank {
    fn name(&self) -> String {
        String::from("RR")
    }
    fn score(&self, _qid: &str, ranked_list: &[RankedInstance]) -> f64 {
        // Compute RR:
        let mut recip_rank = 0.0;
        if let Some(rel_rank) = ranked_list
            .iter()
            .map(|ri| ri.is_relevant())
            .enumerate()
            .filter(|(_, rel)| *rel)
            .nth(0)
            .map(|(i, _)| i + 1)
        {
            recip_rank = 1.0 / (rel_rank as f64)
        }
        return recip_rank;
    }
}

fn compute_dcg(gains: &[NotNan<f32>], depth: usize) -> f64 {
    let mut dcg = 0.0;
    for (i, gain) in gains.iter().enumerate().take(depth) {
        let gain = gain.into_inner() as f64;
        let i = i as f64;
        dcg += ((2.0 as f64).powf(gain) - 1.0) / (i + 2.0).log2();
    }
    dcg
}

pub struct NDCG {
    depth: usize,
    ideal_gains: HashMap<String, Option<f64>>,
}
impl NDCG {
    pub fn new(
        depth: usize,
        dataset: &RankingDataset,
        judgments: Option<QuerySetJudgments>,
    ) -> Self {
        let mut query_norms = HashMap::new();

        for (qid, instance_ids) in dataset.data_by_query.iter() {
            // Determine the total number of relevant documents:
            let all_gains: Option<Vec<NotNan<f32>>> = judgments
                .as_ref()
                .and_then(|j| j.get(qid))
                .map(|data| data.gain_vector());
            // Calculate if unavailable in config:
            let mut ideal_gains: Vec<NotNan<f32>> = all_gains.unwrap_or_else(|| {
                instance_ids
                    .iter()
                    .map(|index| dataset.instances[*index].gain)
                    .filter(|g| g.into_inner() > 0.0)
                    .collect()
            });

            // Sort descending:
            ideal_gains.sort_unstable_by(|lhs, rhs| rhs.cmp(lhs));
            query_norms.insert(
                qid.clone(),
                if ideal_gains.is_empty() {
                    None
                } else {
                    Some(compute_dcg(&ideal_gains, depth))
                },
            );
        }

        Self {
            depth,
            ideal_gains: query_norms,
        }
    }
}

impl Evaluator for NDCG {
    fn name(&self) -> String {
        String::from("NDCG")
    }
    fn score(&self, qid: &str, ranked_list: &[RankedInstance]) -> f64 {
        let actual_gain_vector: Vec<_> = ranked_list
            .iter()
            .map(|ri| ri.gain)
            .filter(|g| g.into_inner() > 0.0)
            .collect();

        let normalizer = self.ideal_gains.get(qid).cloned().unwrap_or_else(|| {
            let mut gain_vector: Vec<_> = actual_gain_vector.clone();
            gain_vector.sort_unstable_by(|lhs, rhs| rhs.cmp(lhs));
            if gain_vector.is_empty() {
                None
            } else {
                Some(compute_dcg(&gain_vector, self.depth))
            }
        });

        if let Some(ideal_ndcg) = normalizer {
            // Compute NDCG:
            let actual_dcg = compute_dcg(&actual_gain_vector, self.depth);
            actual_dcg / ideal_ndcg
        } else {
            // If not gains, there's nothing to calculate.
            0.0
        }
    }
}

pub struct AveragePrecision {
    /// Norms are the number of relevant by query for mAP.
    query_norms: HashMap<String, u32>,
}

impl AveragePrecision {
    pub fn new(dataset: &RankingDataset, judgments: Option<QuerySetJudgments>) -> Self {
        let mut query_norms = HashMap::new();

        for (qid, instance_ids) in dataset.data_by_query.iter() {
            // Determine the total number of relevant documents:
            let param_num_relevant: Option<u32> = judgments
                .as_ref()
                .and_then(|j| j.get(qid))
                .map(|data| data.num_relevant());
            // Calculate if unavailable in config:
            let num_relevant: u32 = param_num_relevant.unwrap_or_else(|| {
                instance_ids
                    .iter()
                    .filter(|index| dataset.instances[**index].is_relevant())
                    .count() as u32
            });

            if num_relevant > 0 {
                query_norms.insert(qid.clone(), num_relevant);
            }
        }

        Self { query_norms }
    }
}

impl Evaluator for AveragePrecision {
    fn name(&self) -> String {
        String::from("AP")
    }
    fn score(&self, qid: &str, ranked_list: &[RankedInstance]) -> f64 {
        let num_relevant = self
            .query_norms
            .get(qid)
            .cloned()
            .unwrap_or_else(|| ranked_list.iter().filter(|ri| ri.is_relevant()).count() as u32);

        if num_relevant == 0 {
            return 0.0;
        }

        // Compute AP:
        let mut recall_points = 0;
        let mut sum_precision = 0.0;
        for rank in ranked_list
            .iter()
            .map(|ri| ri.is_relevant())
            .enumerate()
            .filter(|(_, rel)| *rel)
            .map(|(i, _)| i + 1)
        {
            recall_points += 1;
            sum_precision += f64::from(recall_points) / (rank as f64);
        }
        sum_precision / (num_relevant as f64)
    }
}