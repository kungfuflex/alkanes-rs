// Include the generated code from WIT codegen
#[allow(unused_imports, dead_code, clippy::all)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

use alkanes_runtime::runtime::AlkaneResponder;
use alkanes_runtime::auth::AuthenticatedResponder;
use alkanes_std_factory_support::MintableToken;
use alkanes_support::{parcel::AlkaneTransfer, response::CallResponse};
use anyhow::{anyhow, Result};

use generated::OwnedTokenInterface;

#[derive(Default)]
pub struct OwnedToken(());

impl MintableToken for OwnedToken {}
impl AuthenticatedResponder for OwnedToken {}
impl AlkaneResponder for OwnedToken {}

/// Implement the WIT-generated trait for the contract.
impl OwnedTokenInterface for OwnedToken {
    fn initialize(&self, auth_token_units: u128, token_units: u128) -> Result<CallResponse> {
        self.initialize_with_name_symbol(
            auth_token_units,
            token_units,
            String::from("OWNED"),
            String::from("OWNED"),
        )
    }

    fn initialize_with_name_symbol(
        &self,
        auth_token_units: u128,
        token_units: u128,
        name: String,
        symbol: String,
    ) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes.clone());

        <Self as MintableToken>::set_name_and_symbol_str(self, name, symbol);

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
        let mut response = CallResponse::forward(&context.incoming_alkanes.clone());

        self.only_owner()?;

        let transfer = <Self as MintableToken>::mint(self, &context, token_units)?;
        response.alkanes.0.push(transfer);

        Ok(response)
    }

    fn burn(&self) -> Result<CallResponse> {
        let context = self.context()?;
        if context.incoming_alkanes.0.len() != 1 {
            return Err(anyhow!("Input must be 1 alkane"));
        }
        if context.myself != context.incoming_alkanes.0[0].id {
            return Err(anyhow!("Input must be owned token"));
        }

        self.decrease_total_supply(context.incoming_alkanes.0[0].value)?;

        Ok(CallResponse::default())
    }

    fn get_name(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes.clone());
        response.data = self.name().into_bytes().to_vec();
        Ok(response)
    }

    fn get_symbol(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes.clone());
        response.data = self.symbol().into_bytes().to_vec();
        Ok(response)
    }

    fn get_total_supply(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes.clone());
        response.data = self.total_supply().to_le_bytes().to_vec();
        Ok(response)
    }

    fn get_data(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes.clone());
        response.data = self.data();
        Ok(response)
    }
}
