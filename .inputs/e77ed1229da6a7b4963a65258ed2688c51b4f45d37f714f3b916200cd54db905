//! Open-Source-Watch engine: the intelligence cycle behind the IRONSIGHT front end.
//!
//!     collect → process → store → fuse → analyze → produce,   served by api
//!
//! The core (fuse, analyze, gnss, classifier, process::when, geo) computes on rows;
//! store, collect and api are the shell that reads and writes them.
#![allow(non_snake_case, clippy::type_complexity)]

pub mod analyze;
pub mod api;
pub mod classifier;
pub mod collect;
pub mod context;
pub mod cycle;
pub mod fmt;
pub mod fuse;
pub mod geo;
pub mod gnss;
pub mod process;
pub mod produce;
pub mod regions;
pub mod schema;
pub mod signals;
pub mod store;
pub mod validate;
