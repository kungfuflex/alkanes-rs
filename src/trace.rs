// Copyright 2024-present, Fractal Industries, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Trace Module
//!
//! This module is responsible for saving execution traces of Alkane transactions.
//! Traces provide detailed information about the execution flow, including function
//! calls, inputs, and outputs, which is invaluable for debugging and analysis.

use crate::into_proto::IntoProto;
use crate::tables::{TRACES, TRACES_BY_HEIGHT};
use alkanes_support::trace::Trace;
use anyhow::Result;
use bitcoin::OutPoint;
use metashrew_support::index_pointer::KeyValuePointer;
use metashrew_support::utils::consensus_encode;
use protobuf::Message;
use std::sync::Arc;
#[allow(unused_imports)]
use {
    metashrew_core::{println, stdio::stdout},
    std::fmt::Write,
};

pub fn save_trace(outpoint: &OutPoint, height: u64, trace: Trace) -> Result<()> {
    let buffer: Vec<u8> = consensus_encode::<OutPoint>(outpoint)?;
    TRACES.select(&buffer).set(Arc::<Vec<u8>>::new(
        trace.into_proto()
            .write_to_bytes()?,
    ));
    TRACES_BY_HEIGHT
        .select_value(height)
        .append(Arc::new(buffer));
    Ok(())
}
