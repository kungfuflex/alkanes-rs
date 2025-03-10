use alkanes_runtime::runtime::AlkaneResponder;
use alkanes_runtime::{auth::AuthenticatedResponder, declare_alkane, message::MessageDispatch};
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_support::{context::Context, parcel::AlkaneTransfer, response::CallResponse};
use anyhow::{anyhow, Result};
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};
pub mod factory;

use crate::factory::MintableToken;

#[derive(Default)]
pub struct OwnedToken(());

impl MintableToken for OwnedToken {}

impl AuthenticatedResponder for OwnedToken {}

#[derive(MessageDispatch)]
enum OwnedTokenMessage {
    #[opcode(0)]
    #[method("initialize")]
    Initialize(u128, u128),

    #[opcode(77)]
    #[method("mint")]
    Mint(u128),

    #[opcode(99)]
    #[method("get_name")]
    GetName,

    #[opcode(100)]
    #[method("get_symbol")]
    GetSymbol,

    #[opcode(101)]
    #[method("get_total_supply")]
    GetTotalSupply,

    #[opcode(1000)]
    #[method("get_data")]
    GetData,
}

impl OwnedToken {
    fn initialize(&self, auth_token_units: u128, token_units: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response: CallResponse = CallResponse::forward(&context.incoming_alkanes.clone());

        self.observe_initialization()?;

        response
            .alkanes
            .0
            .push(self.deploy_auth_token(auth_token_units)?);

        response.alkanes.0.push(AlkaneTransfer {
            id: context.myself.clone(),
            value: token_units,
        });

        Ok(response)
    }

    fn mint(&self, token_units: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response: CallResponse = CallResponse::forward(&context.incoming_alkanes.clone());

        self.only_owner()?;

        // Call the mint method from the MintableToken trait
        let transfer = <Self as MintableToken>::mint(self, &context, token_units)?;
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

    fn get_total_supply(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response: CallResponse = CallResponse::forward(&context.incoming_alkanes.clone());

        response.data = self.total_supply().to_le_bytes().to_vec();

        Ok(response)
    }

    fn get_data(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response: CallResponse = CallResponse::forward(&context.incoming_alkanes.clone());

        response.data = self.data();

        Ok(response)
    }
}

impl AlkaneResponder for OwnedToken {
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
    impl AlkaneResponder for OwnedToken {
        type Message = OwnedTokenMessage;
    }
}
