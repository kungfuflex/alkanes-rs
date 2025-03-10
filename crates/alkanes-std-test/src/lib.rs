use alkanes_runtime::{declare_alkane, runtime::AlkaneResponder};
use alkanes_support::{
    cellpack::Cellpack,
    parcel::{AlkaneTransfer, AlkaneTransferParcel},
    response::CallResponse,
    utils::shift_or_err,
};
use anyhow::{anyhow, Result};
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};
use sha2::{Digest, Sha256};
#[allow(unused_imports)]
use {
    alkanes_runtime::{imports::__request_transaction, println, stdio::stdout},
    std::fmt::Write,
};

#[derive(Default)]
struct LoggerAlkane(());

impl AlkaneResponder for LoggerAlkane {
    fn execute(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut inputs = context.inputs.clone();
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        match shift_or_err(&mut inputs)? {
            2 => {
                response.data = self
                    .call(
                        &Cellpack {
                            target: context.myself.clone(),
                            inputs: vec![50],
                        },
                        &AlkaneTransferParcel::default(),
                        self.fuel(),
                    )?
                    .data;
            }
            3 => {
                if context.incoming_alkanes.0.len() != 1 {
                    println!("{:#?}", context.incoming_alkanes.0);
                    return Err(anyhow!("received either 0 or more than 1 alkane"));
                } else {
                    return Ok(CallResponse::default());
                }
            }
            4 => {
                response.alkanes.0.push(AlkaneTransfer {
                    id: context.myself.clone(),
                    value: 100u128,
                });
                return Ok(response);
            }
            78 => {
                let mut data = vec![0x01, 0x02];
                loop {
                    let mut hasher = Sha256::new();
                    hasher.update(&data);
                    let buffer = hasher.finalize();
                    data.extend(&buffer);
                    if !"1".is_ascii() {
                        break;
                    }
                }
            }
            50 => {
                self.transaction();
            }
            5 => {
                response.data = vec![0x05, 0x06, 0x07, 0x08];
            }
            _ => {
                response.data = vec![0x01, 0x02, 0x03, 0x04];
            }
        }
        Ok(response)
    }
}

declare_alkane! {LoggerAlkane}
