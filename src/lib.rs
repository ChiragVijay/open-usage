pub mod analytics;
pub mod config;
mod input;
#[cfg(any(windows, test))]
pub mod install;
pub mod model;
pub mod pricing;
pub mod providers;
mod storage;
pub mod store;
pub mod update;
