use crate::dataset::{DatasetRef, RankingDataset};
use crate::instance::FeatureRead;
use crate::model::Model;
use crate::{FeatureId, InstanceId};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum TypedArrayRef {
    DenseI32(&'static [i32]),
    DenseI64(&'static [i64]),
    DenseF32(&'static [f32]),
    DenseF64(&'static [f64]),
}

impl TypedArrayRef {
    pub fn len(&self) -> usize {
        match self {
            TypedArrayRef::DenseI32(arr) => arr.len(),
            TypedArrayRef::DenseI64(arr) => arr.len(),
            TypedArrayRef::DenseF32(arr) => arr.len(),
            TypedArrayRef::DenseF64(arr) => arr.len(),
        }
    }
    pub fn get_i32(&self, index: usize) -> Option<i32> {
        match self {
            TypedArrayRef::DenseI32(arr) => arr.get(index).cloned(),
            TypedArrayRef::DenseI64(_) => None,
            TypedArrayRef::DenseF32(_) => None,
            TypedArrayRef::DenseF64(_) => None,
        }
    }
    pub fn get_i64(&self, index: usize) -> Option<i64> {
        match self {
            TypedArrayRef::DenseI32(arr) => arr.get(index).cloned().map(|x| x as i64),
            TypedArrayRef::DenseI64(arr) => arr.get(index).cloned(),
            TypedArrayRef::DenseF32(_) => None,
            TypedArrayRef::DenseF64(_) => None,
        }
    }
    pub fn get_f32(&self, index: usize) -> Option<f32> {
        match self {
            TypedArrayRef::DenseI32(arr) => arr.get(index).cloned().map(|x| x as f32),
            TypedArrayRef::DenseI64(arr) => arr.get(index).cloned().map(|x| x as f32),
            TypedArrayRef::DenseF32(arr) => arr.get(index).cloned(),
            TypedArrayRef::DenseF64(arr) => arr.get(index).cloned().map(|x| x as f32),
        }
    }
    pub fn get_f64(&self, index: usize) -> Option<f64> {
        match self {
            TypedArrayRef::DenseI32(arr) => arr.get(index).cloned().map(|x| x as f64),
            TypedArrayRef::DenseI64(arr) => arr.get(index).cloned().map(|x| x as f64),
            TypedArrayRef::DenseF32(arr) => arr.get(index).cloned().map(|x| x as f64),
            TypedArrayRef::DenseF64(arr) => arr.get(index).cloned(),
        }
    }
    pub fn dot(&self, weights: &[f64], start: usize) -> f64 {
        let mut sum = 0.0;
        match self {
            TypedArrayRef::DenseI32(_) => todo! {},
            TypedArrayRef::DenseI64(_) => todo! {},
            TypedArrayRef::DenseF32(arr) => {
                for (w, x) in arr[start..].iter().cloned().zip(weights.iter().cloned()) {
                    sum += (w as f64) * x;
                }
            }
            TypedArrayRef::DenseF64(arr) => {
                for (w, x) in arr[start..].iter().zip(weights) {
                    sum += w * x;
                }
            }
        }
        sum
    }
}

#[derive(Debug, Clone)]
pub struct DenseDataset {
    num_features: usize,
    num_instances: usize,
    xs: TypedArrayRef,
    ys: TypedArrayRef,
    qid_strings: HashMap<i64, String>,
    qids: TypedArrayRef,
    feature_names: HashMap<FeatureId, String>,
}

impl DenseDataset {
    pub fn into_ref(self) -> DatasetRef {
        DatasetRef {
            data: Arc::new(self),
        }
    }
    pub fn try_new(
        num_instances: usize,
        num_features: usize,
        xs: TypedArrayRef,
        ys: TypedArrayRef,
        qids: TypedArrayRef,
        qid_strs: Option<HashMap<i64, String>>,
    ) -> Result<DenseDataset, Box<dyn Error>> {
        if ys.len() != num_instances {
            Err("Bad y-length")?;
        }
        if qids.len() != num_instances {
            Err("Bad qids-length")?;
        }
        if xs.len() != (num_instances * num_features) {
            Err("Bad xs-length")?;
        }

        let qid_strings = if let Some(from_py) = qid_strs {
            from_py
        } else {
            let mut computed = HashMap::new();
            for id in 0..num_instances {
                let qid = qids.get_i64(id).unwrap();
                computed.entry(qid).or_insert_with(|| format!("{}", qid));
            }
            computed
        };

        Ok(DenseDataset {
            num_instances,
            num_features,
            xs,
            ys,
            qids,
            qid_strings,
            feature_names: HashMap::new(),
        })
    }
}

struct DenseDatasetInstance<'dataset> {
    dataset: &'dataset DenseDataset,
    id: InstanceId,
}

impl FeatureRead for DenseDatasetInstance<'_> {
    fn get(&self, idx: FeatureId) -> Option<f64> {
        self.dataset.get_feature_value(self.id, idx)
    }
    fn dotp(&self, weights: &[f64]) -> f64 {
        let start = self.id.to_index() * self.dataset.num_features;
        self.dataset.xs.dot(weights, start)
    }
}

impl RankingDataset for DenseDataset {
    fn get_ref(&self) -> Option<DatasetRef> {
        None
        //panic!("Use into_ref() instead!")
    }
    fn is_sampled(&self) -> bool {
        false
    }
    fn features(&self) -> Vec<FeatureId> {
        (0..self.num_features)
            .map(|i| FeatureId::from_index(i))
            .collect()
    }
    fn n_dim(&self) -> u32 {
        self.num_features as u32
    }
    fn n_instances(&self) -> u32 {
        return self.num_instances as u32;
    }
    fn instances(&self) -> Vec<InstanceId> {
        (0..self.num_instances)
            .map(|i| InstanceId::from_index(i))
            .collect()
    }
    fn instances_by_query(&self) -> HashMap<String, Vec<InstanceId>> {
        let mut ref_map = HashMap::<&str, Vec<InstanceId>>::new();
        for i in 0..self.qids.len() {
            let qid_no = self.qids.get_i64(i).unwrap();
            let qid_str = &self.qid_strings[&qid_no];
            ref_map
                .entry(qid_str.as_str())
                .or_default()
                .push(InstanceId::from_index(i));
        }
        ref_map
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect()
    }
    fn score(&self, id: InstanceId, model: &dyn Model) -> f64 {
        let instance = DenseDatasetInstance { id, dataset: self };
        model.score(&instance)
    }
    fn gain(&self, id: InstanceId) -> f32 {
        let index = id.to_index();
        self.ys
            .get_f32(index)
            .expect("only valid TrainingInstances should exist")
    }
    fn query_id(&self, id: InstanceId) -> &str {
        let qid_no = self.qids.get_i64(id.to_index()).unwrap();
        self.qid_strings[&qid_no].as_str()
    }
    fn document_name(&self, _id: InstanceId) -> Option<&str> {
        // TODO: someday support names array!
        None
    }
    fn queries(&self) -> Vec<String> {
        self.qid_strings.values().cloned().collect()
    }
    /// For printing, the name if available or the number.
    fn feature_name(&self, fid: FeatureId) -> String {
        self.feature_names
            .get(&fid)
            .cloned()
            .unwrap_or_else(|| format!("{}", fid.to_index()))
    }
    /// Lookup a feature value for a particular instance.
    fn get_feature_value(&self, instance: InstanceId, fid: FeatureId) -> Option<f64> {
        let index = self.num_features * instance.to_index() + fid.to_index();
        self.xs.get_f64(index)
    }
    // Given a name or number as a string, lookup the feature id:
    fn try_lookup_feature(&self, name_or_num: &str) -> Result<FeatureId, Box<dyn Error>> {
        crate::dataset::try_lookup_feature(self, &self.feature_names, name_or_num)
    }

    fn score_all(&self, model: &dyn Model) -> Vec<f64> {
        let mut output = Vec::with_capacity(self.num_instances);
        for id in 0..self.num_instances {
            let id = InstanceId::from_index(id);
            let instance = DenseDatasetInstance { id, dataset: self };
            output.push(model.score(&instance))
        }
        output
    }

    fn gains(&self) -> Vec<f32> {
        let mut output = Vec::with_capacity(self.num_instances);
        assert_eq!(self.num_instances, self.ys.len());
        for id in 0..self.num_instances {
            output.push(self.ys.get_f32(id).expect("present"));
        }
        output
    }

    fn query_ids(&self) -> Vec<&str> {
        (0..self.num_instances)
            .map(|index| self.qids.get_i64(index).expect("present"))
            .map(|qid_id| self.qid_strings[&qid_id].as_str())
            .collect()
    }

    fn copy_features_f32(&self, destination: &mut [f32]) -> Result<usize, Box<dyn Error>> {
        let n = self.n_instances() as usize;
        let d = self.n_dim() as usize;
        assert_eq!(destination.len(), (n * d));

        for (index, dest) in destination.iter_mut().enumerate() {
            *dest = self.xs.get_f32(index).unwrap_or_default();
        }
        Ok(destination.len())
    }
}
