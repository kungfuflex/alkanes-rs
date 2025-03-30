use crate::AlkanesIndexer;
use metashrew_core::host;
use metashrew_core::view;
use std::any::TypeId;

// Define the WASM exports directly
static mut INDEXER_INSTANCE: Option<metashrew_core::indexer::MetashrewIndexer<AlkanesIndexer>> = None;

#[no_mangle]
pub extern "C" fn _start() {
    unsafe {
        if INDEXER_INSTANCE.is_none() {
            let indexer = AlkanesIndexer::default();
            INDEXER_INSTANCE = Some(metashrew_core::indexer::MetashrewIndexer::new(indexer));
        }

        if let Some(indexer) = &mut INDEXER_INSTANCE {
            if let Err(e) = indexer.process_block() {
                host::log(&format!("Error processing block: {}", e));
            }
        }
    }
}

// MultiSimulateRequest view function
#[no_mangle]
pub extern "C" fn multisimluate() -> i32 {
    unsafe {
        if INDEXER_INSTANCE.is_none() {
            let indexer = AlkanesIndexer::default();
            INDEXER_INSTANCE = Some(metashrew_core::indexer::MetashrewIndexer::new(indexer));
        }

        if let Some(indexer) = &INDEXER_INSTANCE {
            // Load the input data
            let (_height, input_bytes) = match host::load_input() {
                Ok(input) => input,
                Err(e) => {
                    host::log(&format!("Error loading input: {}", e));
                    return view::return_view_result(&[]);
                }
            };

            // Parse the request
            let request = match crate::proto::alkanes::MultiSimulateRequest::parse_from_bytes(&input_bytes) {
                Ok(req) => req,
                Err(e) => {
                    host::log(&format!("Error parsing request: {}", e));
                    return view::return_view_result(&[]);
                }
            };

            // Call the view function
            match indexer.get_indexer().multisimluate(request) {
                Ok(response) => {
                    // Serialize the response
                    match response.write_to_bytes() {
                        Ok(bytes) => view::return_view_result(&bytes),
                        Err(e) => {
                            host::log(&format!("Error serializing response: {}", e));
                            view::return_view_result(&[])
                        }
                    }
                },
                Err(e) => {
                    host::log(&format!("Error executing view function: {}", e));
                    view::return_view_result(&[])
                }
            }
        } else {
            view::return_view_result(&[])
        }
    }
}

// Simulate view function
#[no_mangle]
pub extern "C" fn simulate() -> i32 {
    unsafe {
        if INDEXER_INSTANCE.is_none() {
            let indexer = AlkanesIndexer::default();
            INDEXER_INSTANCE = Some(metashrew_core::indexer::MetashrewIndexer::new(indexer));
        }

        if let Some(indexer) = &INDEXER_INSTANCE {
            // Load the input data
            let (_height, input_bytes) = match host::load_input() {
                Ok(input) => input,
                Err(e) => {
                    host::log(&format!("Error loading input: {}", e));
                    return view::return_view_result(&[]);
                }
            };

            // Parse the request
            let request = match crate::proto::alkanes::MessageContextParcel::parse_from_bytes(&input_bytes) {
                Ok(req) => req,
                Err(e) => {
                    host::log(&format!("Error parsing request: {}", e));
                    return view::return_view_result(&[]);
                }
            };

            // Call the view function
            match indexer.get_indexer().simulate(request) {
                Ok(response) => {
                    // Serialize the response
                    match response.write_to_bytes() {
                        Ok(bytes) => view::return_view_result(&bytes),
                        Err(e) => {
                            host::log(&format!("Error serializing response: {}", e));
                            view::return_view_result(&[])
                        }
                    }
                },
                Err(e) => {
                    host::log(&format!("Error executing view function: {}", e));
                    view::return_view_result(&[])
                }
            }
        } else {
            view::return_view_result(&[])
        }
    }
}

// Meta view function
#[no_mangle]
pub extern "C" fn meta() -> i32 {
    unsafe {
        if INDEXER_INSTANCE.is_none() {
            let indexer = AlkanesIndexer::default();
            INDEXER_INSTANCE = Some(metashrew_core::indexer::MetashrewIndexer::new(indexer));
        }

        if let Some(indexer) = &INDEXER_INSTANCE {
            // Load the input data
            let (_height, input_bytes) = match host::load_input() {
                Ok(input) => input,
                Err(e) => {
                    host::log(&format!("Error loading input: {}", e));
                    return view::return_view_result(&[]);
                }
            };

            // Parse the request
            let request = match crate::proto::alkanes::MessageContextParcel::parse_from_bytes(&input_bytes) {
                Ok(req) => req,
                Err(e) => {
                    host::log(&format!("Error parsing request: {}", e));
                    return view::return_view_result(&[]);
                }
            };

            // Call the view function
            match indexer.get_indexer().meta(request) {
                Ok(response) => {
                    // For Vec<u8>, just return the bytes directly
                    view::return_view_result(&response)
                },
                Err(e) => {
                    host::log(&format!("Error executing view function: {}", e));
                    view::return_view_result(&[])
                }
            }
        } else {
            view::return_view_result(&[])
        }
    }
}

// Runesbyaddress view function
#[no_mangle]
pub extern "C" fn runesbyaddress() -> i32 {
    unsafe {
        if INDEXER_INSTANCE.is_none() {
            let indexer = AlkanesIndexer::default();
            INDEXER_INSTANCE = Some(metashrew_core::indexer::MetashrewIndexer::new(indexer));
        }

        if let Some(indexer) = &INDEXER_INSTANCE {
            // Load the input data
            let (_height, input_bytes) = match host::load_input() {
                Ok(input) => input,
                Err(e) => {
                    host::log(&format!("Error loading input: {}", e));
                    return view::return_view_result(&[]);
                }
            };

            // For Vec<u8>, just use the input bytes directly
            let request = input_bytes;

            // Call the view function
            match indexer.get_indexer().runesbyaddress(request) {
                Ok(response) => {
                    // Serialize the response
                    match response.write_to_bytes() {
                        Ok(bytes) => view::return_view_result(&bytes),
                        Err(e) => {
                            host::log(&format!("Error serializing response: {}", e));
                            view::return_view_result(&[])
                        }
                    }
                },
                Err(e) => {
                    host::log(&format!("Error executing view function: {}", e));
                    view::return_view_result(&[])
                }
            }
        } else {
            view::return_view_result(&[])
        }
    }
}

// Runesbyoutpoint view function
#[no_mangle]
pub extern "C" fn runesbyoutpoint() -> i32 {
    unsafe {
        if INDEXER_INSTANCE.is_none() {
            let indexer = AlkanesIndexer::default();
            INDEXER_INSTANCE = Some(metashrew_core::indexer::MetashrewIndexer::new(indexer));
        }

        if let Some(indexer) = &INDEXER_INSTANCE {
            // Load the input data
            let (_height, input_bytes) = match host::load_input() {
                Ok(input) => input,
                Err(e) => {
                    host::log(&format!("Error loading input: {}", e));
                    return view::return_view_result(&[]);
                }
            };

            // For Vec<u8>, just use the input bytes directly
            let request = input_bytes;

            // Call the view function
            match indexer.get_indexer().runesbyoutpoint(request) {
                Ok(response) => {
                    // Serialize the response
                    match response.write_to_bytes() {
                        Ok(bytes) => view::return_view_result(&bytes),
                        Err(e) => {
                            host::log(&format!("Error serializing response: {}", e));
                            view::return_view_result(&[])
                        }
                    }
                },
                Err(e) => {
                    host::log(&format!("Error executing view function: {}", e));
                    view::return_view_result(&[])
                }
            }
        } else {
            view::return_view_result(&[])
        }
    }
}

// Protorunesbyheight view function
#[no_mangle]
pub extern "C" fn protorunesbyheight() -> i32 {
    unsafe {
        if INDEXER_INSTANCE.is_none() {
            let indexer = AlkanesIndexer::default();
            INDEXER_INSTANCE = Some(metashrew_core::indexer::MetashrewIndexer::new(indexer));
        }

        if let Some(indexer) = &INDEXER_INSTANCE {
            // Load the input data
            let (_height, input_bytes) = match host::load_input() {
                Ok(input) => input,
                Err(e) => {
                    host::log(&format!("Error loading input: {}", e));
                    return view::return_view_result(&[]);
                }
            };

            // For Vec<u8>, just use the input bytes directly
            let request = input_bytes;

            // Call the view function
            match indexer.get_indexer().protorunesbyheight(request) {
                Ok(response) => {
                    // Serialize the response
                    match response.write_to_bytes() {
                        Ok(bytes) => view::return_view_result(&bytes),
                        Err(e) => {
                            host::log(&format!("Error serializing response: {}", e));
                            view::return_view_result(&[])
                        }
                    }
                },
                Err(e) => {
                    host::log(&format!("Error executing view function: {}", e));
                    view::return_view_result(&[])
                }
            }
        } else {
            view::return_view_result(&[])
        }
    }
}

// Traceblock view function
#[no_mangle]
pub extern "C" fn traceblock() -> i32 {
    unsafe {
        if INDEXER_INSTANCE.is_none() {
            let indexer = AlkanesIndexer::default();
            INDEXER_INSTANCE = Some(metashrew_core::indexer::MetashrewIndexer::new(indexer));
        }

        if let Some(indexer) = &INDEXER_INSTANCE {
            // Load the input data
            let (_height, input_bytes) = match host::load_input() {
                Ok(input) => input,
                Err(e) => {
                    host::log(&format!("Error loading input: {}", e));
                    return view::return_view_result(&[]);
                }
            };

            // For u32, convert from bytes
            let request = if input_bytes.len() >= 4 {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(&input_bytes[0..4]);
                u32::from_le_bytes(bytes)
            } else {
                host::log("Error: input bytes too short for u32");
                return view::return_view_result(&[]);
            };

            // Call the view function
            match indexer.get_indexer().traceblock(request) {
                Ok(response) => {
                    // For Vec<u8>, just return the bytes directly
                    view::return_view_result(&response)
                },
                Err(e) => {
                    host::log(&format!("Error executing view function: {}", e));
                    view::return_view_result(&[])
                }
            }
        } else {
            view::return_view_result(&[])
        }
    }
}

// Trace view function
#[no_mangle]
pub extern "C" fn trace() -> i32 {
    unsafe {
        if INDEXER_INSTANCE.is_none() {
            let indexer = AlkanesIndexer::default();
            INDEXER_INSTANCE = Some(metashrew_core::indexer::MetashrewIndexer::new(indexer));
        }

        if let Some(indexer) = &INDEXER_INSTANCE {
            // Load the input data
            let (_height, input_bytes) = match host::load_input() {
                Ok(input) => input,
                Err(e) => {
                    host::log(&format!("Error loading input: {}", e));
                    return view::return_view_result(&[]);
                }
            };

            // Parse the request
            let request = match protorune_support::proto::protorune::Outpoint::parse_from_bytes(&input_bytes) {
                Ok(req) => req,
                Err(e) => {
                    host::log(&format!("Error parsing request: {}", e));
                    return view::return_view_result(&[]);
                }
            };

            // Call the view function
            match indexer.get_indexer().trace(request) {
                Ok(response) => {
                    // For Vec<u8>, just return the bytes directly
                    view::return_view_result(&response)
                },
                Err(e) => {
                    host::log(&format!("Error executing view function: {}", e));
                    view::return_view_result(&[])
                }
            }
        } else {
            view::return_view_result(&[])
        }
    }
}

// Getbytecode view function
#[no_mangle]
pub extern "C" fn getbytecode() -> i32 {
    unsafe {
        if INDEXER_INSTANCE.is_none() {
            let indexer = AlkanesIndexer::default();
            INDEXER_INSTANCE = Some(metashrew_core::indexer::MetashrewIndexer::new(indexer));
        }

        if let Some(indexer) = &INDEXER_INSTANCE {
            // Load the input data
            let (_height, input_bytes) = match host::load_input() {
                Ok(input) => input,
                Err(e) => {
                    host::log(&format!("Error loading input: {}", e));
                    return view::return_view_result(&[]);
                }
            };

            // For Vec<u8>, just use the input bytes directly
            let request = input_bytes;

            // Call the view function
            match indexer.get_indexer().getbytecode(request) {
                Ok(response) => {
                    // For Vec<u8>, just return the bytes directly
                    view::return_view_result(&response)
                },
                Err(e) => {
                    host::log(&format!("Error executing view function: {}", e));
                    view::return_view_result(&[])
                }
            }
        } else {
            view::return_view_result(&[])
        }
    }
}

// Protorunesbyoutpoint view function
#[no_mangle]
pub extern "C" fn protorunesbyoutpoint() -> i32 {
    unsafe {
        if INDEXER_INSTANCE.is_none() {
            let indexer = AlkanesIndexer::default();
            INDEXER_INSTANCE = Some(metashrew_core::indexer::MetashrewIndexer::new(indexer));
        }

        if let Some(indexer) = &INDEXER_INSTANCE {
            // Load the input data
            let (_height, input_bytes) = match host::load_input() {
                Ok(input) => input,
                Err(e) => {
                    host::log(&format!("Error loading input: {}", e));
                    return view::return_view_result(&[]);
                }
            };

            // For Vec<u8>, just use the input bytes directly
            let request = input_bytes;

            // Call the view function
            match indexer.get_indexer().protorunesbyoutpoint(request) {
                Ok(response) => {
                    // Serialize the response
                    match response.write_to_bytes() {
                        Ok(bytes) => view::return_view_result(&bytes),
                        Err(e) => {
                            host::log(&format!("Error serializing response: {}", e));
                            view::return_view_result(&[])
                        }
                    }
                },
                Err(e) => {
                    host::log(&format!("Error executing view function: {}", e));
                    view::return_view_result(&[])
                }
            }
        } else {
            view::return_view_result(&[])
        }
    }
}

// Runesbyheight view function
#[no_mangle]
pub extern "C" fn runesbyheight() -> i32 {
    unsafe {
        if INDEXER_INSTANCE.is_none() {
            let indexer = AlkanesIndexer::default();
            INDEXER_INSTANCE = Some(metashrew_core::indexer::MetashrewIndexer::new(indexer));
        }

        if let Some(indexer) = &INDEXER_INSTANCE {
            // Load the input data
            let (_height, input_bytes) = match host::load_input() {
                Ok(input) => input,
                Err(e) => {
                    host::log(&format!("Error loading input: {}", e));
                    return view::return_view_result(&[]);
                }
            };

            // For Vec<u8>, just use the input bytes directly
            let request = input_bytes;

            // Call the view function
            match indexer.get_indexer().runesbyheight(request) {
                Ok(response) => {
                    // Serialize the response
                    match response.write_to_bytes() {
                        Ok(bytes) => view::return_view_result(&bytes),
                        Err(e) => {
                            host::log(&format!("Error serializing response: {}", e));
                            view::return_view_result(&[])
                        }
                    }
                },
                Err(e) => {
                    host::log(&format!("Error executing view function: {}", e));
                    view::return_view_result(&[])
                }
            }
        } else {
            view::return_view_result(&[])
        }
    }
}