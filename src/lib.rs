#![crate_type = "dylib"]

#[macro_use]
extern crate serde_derive;

mod core;
pub(crate) use crate::core::FeatureId;
pub(crate) use crate::core::InstanceId;
pub(crate) use crate::core::Scored;

/// Contains code for feature-at-a-time non-differentiable optimization.
pub mod coordinate_ascent;
pub mod dataset;
pub mod dense_dataset;
pub mod evaluators;
pub mod heap;
pub mod instance;
/// Contains code for reading compressed files based on their extension.
pub mod io_helper;
/// Contains code for reading ranklib and libsvm input files.
pub mod libsvm;
pub mod model;
pub mod qrel;
pub mod randutil;
pub mod sampling;

pub mod json_api;

mod cart;
pub mod random_forest;
/// Streaming computation of statistics.
pub mod stats;

use dataset::DatasetRef;
use dense_dataset::{DenseDataset, TypedArrayRef};
use json_api::TrainRequest;
use model::ModelEnum;
use qrel::QuerySetJudgments;

use libc::{c_char, c_void};
use once_cell::sync::Lazy;
use std::slice;
use std::{collections::HashMap, error::Error};
use std::{
    ffi::CString,
    sync::{Arc, Mutex},
};
use std::{ptr, sync::atomic::AtomicIsize};

mod ffi;
use ffi::*;

static ERROR_ID: AtomicIsize = AtomicIsize::new(1);
static ERRORS: Lazy<Arc<Mutex<HashMap<isize, String>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::default())));

fn next_error_id() -> isize {
    ERROR_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
}

fn store_err<T, E>(r: Result<T, E>, error: *mut isize) -> Result<T, ()>
where
    E: std::fmt::Display + Sized,
{
    match r {
        Ok(x) => Ok(x),
        Err(e) => {
            let err = next_error_id();
            ERRORS.as_ref().lock().unwrap().insert(err, e.to_string());
            unsafe {
                *error = err;
            }
            Err(())
        }
    }
}

fn store_err_to_ptr<T, E>(r: Result<T, E>, error: *mut isize) -> *const T
where
    E: std::fmt::Display + Sized,
{
    store_err(r, error).map(box_to_ptr).unwrap_or(ptr::null())
}

#[no_mangle]
pub extern "C" fn fetch_err(error: isize) -> *const c_void {
    match ERRORS.as_ref().lock().unwrap().remove(&error) {
        Some(msg) => return_string(&msg),
        None => ptr::null(),
    }
}

pub struct CDataset {
    /// Reference to Rust-based Dataset.
    reference: DatasetRef,
}

pub struct CModel {
    actual: ModelEnum,
}

pub struct CQRel {
    actual: QuerySetJudgments,
}

#[no_mangle]
pub extern "C" fn free_str(originally_from_rust: *mut c_void) {
    let _will_drop: CString = unsafe { CString::from_raw(originally_from_rust as *mut c_char) };
}

#[no_mangle]
pub extern "C" fn free_f64(originally_from_rust: *mut c_void) {
    let _will_drop: Box<f64> = unsafe { Box::from_raw(originally_from_rust as *mut f64) };
}

#[no_mangle]
pub extern "C" fn free_dataset(originally_from_rust: *mut CDataset) {
    let _will_drop: Box<CDataset> = unsafe { Box::from_raw(originally_from_rust) };
}

#[no_mangle]
pub extern "C" fn free_model(originally_from_rust: *mut CModel) {
    let _will_drop: Box<CModel> = unsafe { Box::from_raw(originally_from_rust) };
}

#[no_mangle]
pub extern "C" fn free_cqrel(originally_from_rust: *mut CQRel) {
    let _will_drop: Box<CQRel> = unsafe { Box::from_raw(originally_from_rust) };
}

fn box_to_ptr<T>(item: T) -> *const T {
    Box::into_raw(Box::new(item)) as *const T
}

#[no_mangle]
pub extern "C" fn load_cqrel(data_path: *const c_void, error: *mut isize) -> *const CQRel {
    let data_path = accept_str("data_path", data_path);
    store_err_to_ptr(
        result_load_cqrel(data_path).map(|actual| CQRel { actual }),
        error,
    )
}

#[no_mangle]
pub extern "C" fn cqrel_from_json(json_str: *const c_void, error: *mut isize) -> *const CQRel {
    store_err_to_ptr(
        deserialize_from_cstr_json::<QuerySetJudgments>(accept_str("json_str", json_str))
            .map(|actual| CQRel { actual }),
        error,
    )
}

#[no_mangle]
pub extern "C" fn cqrel_query_json(
    cqrel: *const CQRel,
    query_str: *const c_void,
    error: *mut isize,
) -> *const c_void {
    let cqrel: Option<&CQRel> = unsafe { (cqrel as *mut CQRel).as_ref() };
    store_err(
        result_cqrel_query_json(cqrel, accept_str("query_str", query_str)),
        error,
    )
    .map(|str| return_string(&str))
    .unwrap_or(ptr::null())
}

#[no_mangle]
pub extern "C" fn load_ranksvm_format(
    data_path: *mut c_void,
    feature_names_path: *mut c_void,
    error: *mut isize,
) -> *const CDataset {
    let data_path = accept_str("data_path", data_path);
    let feature_names_path: Option<Result<&str, Box<dyn Error>>> = if feature_names_path.is_null() {
        None
    } else {
        Some(accept_str("feature_names_path", feature_names_path))
    };
    store_err_to_ptr(
        result_load_ranksvm_format(data_path, feature_names_path).map(|response| CDataset {
            reference: response,
        }),
        error,
    )
}

#[no_mangle]
pub extern "C" fn dataset_query_sampling(
    dataset: *mut CDataset,
    queries_json_list: *const c_void,
    error: *mut isize,
) -> *const CDataset {
    let dataset: Option<&CDataset> = unsafe { (dataset as *mut CDataset).as_ref() };
    store_err_to_ptr(
        result_dataset_query_sampling(dataset, accept_str("queries_json_list", queries_json_list))
            .map(|response| CDataset {
                reference: response,
            }),
        error,
    )
}

#[no_mangle]
pub extern "C" fn dataset_feature_sampling(
    dataset: *mut CDataset,
    feature_json_list: *const c_void,
    error: *mut isize,
) -> *const CDataset {
    let dataset: Option<&CDataset> = unsafe { (dataset as *mut CDataset).as_ref() };
    store_err_to_ptr(
        result_dataset_feature_sampling(
            dataset,
            accept_str("feature_json_list", feature_json_list),
        )
        .map(|response| CDataset {
            reference: response,
        }),
        error,
    )
}

#[no_mangle]
pub extern "C" fn dataset_query_json(
    dataset: *mut c_void,
    json_cmd_str: *mut c_void,
    error: *mut isize,
) -> *const c_void {
    let dataset: Option<&CDataset> = unsafe { (dataset as *mut CDataset).as_ref() };
    store_err(
        result_dataset_query_json(dataset, accept_str("dataset_query_json", json_cmd_str)),
        error,
    )
    .map(|str| return_string(&str))
    .unwrap_or(ptr::null())
}

#[no_mangle]
pub extern "C" fn query_json(json_cmd_str: *const c_void, error: *mut isize) -> *const c_void {
    match store_err(
        result_exec_json(accept_str("query_json_str", json_cmd_str)),
        error,
    ) {
        Ok(item) => return_string(&item),
        Err(_) => ptr::null(),
    }
}

fn typed_array(
    name: &str,
    xs: *const c_void,
    n: usize,
    dtype: *const c_void,
) -> Result<TypedArrayRef, Box<dyn Error>> {
    if xs.is_null() {
        Err(format!("NULL array pointer for {}!", name))?;
    }
    let dtype = accept_str("dtype", dtype)?;

    let complete = match dtype {
        "float32" => TypedArrayRef::DenseF32(unsafe { slice::from_raw_parts(xs as *const f32, n) }),
        "float64" => TypedArrayRef::DenseF64(unsafe { slice::from_raw_parts(xs as *const f64, n) }),
        "int32" => TypedArrayRef::DenseI32(unsafe { slice::from_raw_parts(xs as *const i32, n) }),
        "int64" => TypedArrayRef::DenseI64(unsafe { slice::from_raw_parts(xs as *const i64, n) }),
        other => Err(format!("Unexpected dtype={} for {}", other, name))?,
    };

    Ok(complete)
}

#[no_mangle]
pub extern "C" fn make_dense_dataset_v2(
    n: usize,
    d: usize,
    x: *const c_void,
    x_type: *const c_void,
    y: *const c_void,
    y_type: *const c_void,
    qids: *const c_void,
    qids_type: *const c_void,
    qid_strs: *const c_void,
    error: *mut isize,
) -> *const CDataset {
    let x_len = n * d;
    store_err_to_ptr::<_, Box<dyn Error>>(
        (|| {
            let xs = typed_array("xs", x, x_len, x_type)?;
            let y = typed_array("y", y, n, y_type)?;
            let qids = typed_array("qids", qids, n, qids_type)?;
            let qid_strs: Option<HashMap<i64, String>> = if qid_strs.is_null() {
                None
            } else {
                Some(serde_json::from_str(accept_str("qid_strs", qid_strs)?)?)
            };
            let dataset = DenseDataset::try_new(n, d, xs, y, qids, qid_strs)?;
            Ok(CDataset {
                reference: dataset.into_ref(),
            })
        })(),
        error,
    )
}

#[no_mangle]
pub extern "C" fn evaluate_query(
    measure: *const c_void,
    n: usize,
    gains: *const f32,
    scores: *const f64,
    depth: i64,
    opts: *const c_void,
    error: *mut isize,
) -> *const f64 {
    store_err_to_ptr(
        (|| {
            let measure = accept_str("measure", measure)?;
            let gains = unsafe { slice::from_raw_parts(gains, n) };
            let scores = unsafe { slice::from_raw_parts(scores, n) };
            let depth = if depth <= 0 {
                None
            } else {
                Some(depth as usize)
            };
            let opts = serde_json::from_str(accept_str("options_json", opts)?)?;
            json_api::evaluate_query(measure, gains, scores, depth, &opts)
        })(),
        error,
    )
}

#[no_mangle]
pub extern "C" fn train_model(
    train_request_json: *mut c_void,
    dataset: *mut c_void,
    error: *mut isize,
) -> *const CModel {
    let dataset: Option<&CDataset> = unsafe { (dataset as *mut CDataset).as_ref() };
    let request: Result<TrainRequest, _> =
        deserialize_from_cstr_json(accept_str("train_request_json", train_request_json));
    store_err(result_train_model(request, dataset), error)
        .map(|actual| CModel { actual })
        .map(box_to_ptr)
        .unwrap_or(ptr::null())
}

#[no_mangle]
pub extern "C" fn model_from_json(json_str: *const c_void, error: *mut isize) -> *const CModel {
    store_err(
        deserialize_from_cstr_json::<ModelEnum>(accept_str("json_str", json_str)),
        error,
    )
    .map(|actual| CModel { actual })
    .map(box_to_ptr)
    .unwrap_or(ptr::null())
}

#[no_mangle]
pub extern "C" fn model_query_json(
    model: *const c_void,
    json_cmd_str: *const c_void,
    error: *mut isize,
) -> *const c_void {
    let model: Option<&CModel> = unsafe { (model as *const CModel).as_ref() };
    store_err(
        result_model_query_json(model, accept_str("query_json", json_cmd_str)),
        error,
    )
    .map(|s| return_string(&s))
    .unwrap_or(ptr::null())
}

/// returns json of qid->score for evaluator; or error-json.
#[no_mangle]
pub extern "C" fn evaluate_by_query(
    model: *const CModel,
    dataset: *const CDataset,
    qrel: *const CQRel,
    evaluator: *const c_void,
) -> *const c_void {
    let model: Option<&CModel> = unsafe { (model as *const CModel).as_ref() };
    let dataset: Option<&CDataset> = unsafe { (dataset as *const CDataset).as_ref() };
    let qrel: Option<&CQRel> = unsafe { (qrel as *const CQRel).as_ref() };
    let evaluator: Result<&str, Box<dyn Error>> = accept_str("evaluator_name", evaluator);
    result_to_json(result_evaluate_by_query(model, dataset, qrel, evaluator))
}

#[no_mangle]
pub extern "C" fn predict_scores(model: *const CModel, dataset: *const CDataset) -> *const c_void {
    let model: Option<&CModel> = unsafe { (model as *const CModel).as_ref() };
    let dataset: Option<&CDataset> = unsafe { (dataset as *const CDataset).as_ref() };
    result_to_json(result_predict_scores(model, dataset))
}

#[no_mangle]
pub extern "C" fn predict_to_trecrun(
    model: *const CModel,
    dataset: *const CDataset,
    output_path: *const c_void,
    system_name: *const c_void,
    depth: usize,
) -> *const c_void {
    let model: Option<&CModel> = unsafe { (model as *const CModel).as_ref() };
    let dataset: Option<&CDataset> = unsafe { (dataset as *const CDataset).as_ref() };
    let output_path: Result<&str, Box<dyn Error>> = accept_str("output_path", output_path);
    let system_name: Result<&str, Box<dyn Error>> = accept_str("system_name", system_name);
    result_to_json(result_predict_to_trecrun(
        model,
        dataset,
        output_path,
        system_name,
        depth,
    ))
}
