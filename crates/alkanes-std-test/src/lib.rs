use alkanes_runtime::{declare_alkane, message::MessageDispatch, runtime::AlkaneResponder};
use alkanes_support::{
    cellpack::Cellpack,
    parcel::{AlkaneTransfer, AlkaneTransferParcel},
    response::CallResponse,
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
pub struct LoggerAlkane(());

#[derive(MessageDispatch)]
enum LoggerAlkaneMessage {
    #[opcode(2)]
    SelfCall,

    #[opcode(3)]
    CheckIncoming,

    #[opcode(4)]
    MintTokens,

    #[opcode(5)]
    #[returns(Vec<u8>)]
    ReturnData1,

    #[opcode(50)]
    GetTransaction,

    #[opcode(78)]
    HashLoop,

    #[opcode(99)]
    #[returns(Vec<u8>)]
    ReturnDefaultData,
}

impl LoggerAlkane {
    fn self_call(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

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

        Ok(response)
    }

    fn check_incoming(&self) -> Result<CallResponse> {
        let context = self.context()?;

        if context.incoming_alkanes.0.len() != 1 {
            println!("{:#?}", context.incoming_alkanes.0);
            return Err(anyhow!("received either 0 or more than 1 alkane"));
        } else {
            return Ok(CallResponse::default());
        }
    }

    fn mint_tokens(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.alkanes.0.push(AlkaneTransfer {
            id: context.myself.clone(),
            value: 100u128,
        });

        Ok(response)
    }

    fn return_data_1(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.data = vec![0x05, 0x06, 0x07, 0x08];

        Ok(response)
    }

    fn get_transaction(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        self.transaction();

        Ok(response)
    }

    fn hash_loop(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

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

        Ok(response)
    }

    fn return_default_data(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.data = vec![0x01, 0x02, 0x03, 0x04];

        Ok(response)
    }
}

impl AlkaneResponder for LoggerAlkane {
    fn execute(&self) -> Result<CallResponse> {
        // The opcode extraction and dispatch logic is now handled by the declare_alkane macro
        // This method is still required by the AlkaneResponder trait, but we can just return an error
        // indicating that it should not be called directly
        Err(anyhow!(
            "This method should not be called directly. Use the declare_alkane macro instead."
        ))
    }
}

// Use the new macro format
declare_alkane! {
    impl AlkaneResponder for LoggerAlkane {
        type Message = LoggerAlkaneMessage;
    }
}
