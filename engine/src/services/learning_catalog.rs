//! Compatibility facade for existing teaching callers. Domain rules belong to the teaching pack;
//! the pack resolves and assesses definitions through the shared task catalogue.
pub use crate::domain_packs::ebpf_teaching::legacy::{
    assess_lab_run, find_lab, lab_definitions, LabAssessment,
};
