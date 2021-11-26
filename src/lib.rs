#![crate_type = "lib"]

#[macro_use]
extern crate serde_derive;

pub mod core;

/// Contains code for feature-at-a-time non-differentiable optimization.
pub mod coordinate_ascent;
pub mod dataset;
pub mod dense_dataset;
pub mod evaluators;
pub mod instance;
/// Contains code for reading compressed files based on their extension.
pub mod io_helper;
/// Contains code for reading ranklib and libsvm input files.
pub mod libsvm;
pub mod model;
pub mod normalizers;
pub mod qrel;
pub mod randutil;
pub mod sampling;

pub mod json_api;

pub mod random_forest;
/// Streaming computation of statistics.
pub mod stats;
