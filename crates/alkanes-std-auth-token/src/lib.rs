use alkanes_runtime::runtime::AlkaneResponder;
use alkanes_runtime::{
    declare_alkane, message::MessageDispatch, storage::StoragePointer, token::Token,
};
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_support::{context::Context, parcel::AlkaneTransfer, response::CallResponse};
use anyhow::{anyhow, Result};
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};
use metashrew_support::index_pointer::KeyValuePointer;
use std::sync::Arc;

#[derive(Default)]
pub struct AuthToken(());

impl Token for AuthToken {
    fn name(&self) -> String {
        String::from("AUTH")
    }
    fn symbol(&self) -> String {
        String::from("AUTH")
    }
}

#[derive(MessageDispatch)]
enum AuthTokenMessage {
    #[opcode(0)]
    Initialize { amount: u128 },

    #[opcode(1)]
    Authenticate,

    #[opcode(99)]
    #[returns(String)]
    GetName,

    #[opcode(100)]
    #[returns(String)]
    GetSymbol,
}

impl AuthToken {
    fn initialize(&self, amount: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response: CallResponse = CallResponse::forward(&context.incoming_alkanes.clone());

        let mut pointer = StoragePointer::from_keyword("/initialized");
        if pointer.get().len() == 0 {
            response.alkanes = context.incoming_alkanes.clone();
            response.alkanes.0.push(AlkaneTransfer {
                id: context.myself.clone(),
                value: amount,
            });
            pointer.set(Arc::new(vec![0x01]));
            Ok(response)
        } else {
            return Err(anyhow!("already initialized"));
        }
    }

    fn authenticate(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response: CallResponse = CallResponse::forward(&context.incoming_alkanes.clone());

        if context.incoming_alkanes.0.len() != 1 {
            return Err(anyhow!(
                "did not authenticate with only the authentication token"
            ));
        }
        let transfer = context.incoming_alkanes.0[0].clone();
        if transfer.id != context.myself.clone() {
            return Err(anyhow!("supplied alkane is not authentication token"));
        }
        if transfer.value < 1 {
            return Err(anyhow!(
                "less than 1 unit of authentication token supplied to authenticate"
            ));
        }
        response.data = vec![0x01];
        response.alkanes.0.push(transfer);
        Ok(response)
    }

    fn get_name(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response: CallResponse = CallResponse::forward(&context.incoming_alkanes.clone());

        response.data = self.name().into_bytes().to_vec();
        Ok(response)
    }

    fn get_symbol(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response: CallResponse = CallResponse::forward(&context.incoming_alkanes.clone());

        response.data = self.symbol().into_bytes().to_vec();
        Ok(response)
    }
}

impl AlkaneResponder for AuthToken {
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
    impl AlkaneResponder for AuthToken {
        type Message = AuthTokenMessage;
    }
}
