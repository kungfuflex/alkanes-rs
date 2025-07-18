use crate::trace::types::TraceEvent;
use bitcoin::OutPoint;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BlockTraceItem {
    pub outpoint: OutPoint,
    pub trace: Vec<TraceEvent>,
}
