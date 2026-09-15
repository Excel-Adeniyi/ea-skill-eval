use std::fmt;

use serde::{Deserialize, Serialize};
//  This is a enum for all 5 metrics we need for the eval
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Metric {
    InstructionAdherence,
    TaskRelevancy,
    InstructionPrecision,
    InstructionRecall,
    FormatCompliance
}

impl Metric {
    
}