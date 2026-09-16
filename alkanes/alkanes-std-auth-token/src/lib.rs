// Include the generated code from WIT codegen
#[allow(unused_imports, dead_code, clippy::all)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

use alkanes_runtime::runtime::AlkaneResponder;
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_std_factory_support::MintableToken;
use alkanes_support::{parcel::AlkaneTransfer, response::CallResponse};
use anyhow::{anyhow, Result};

use generated::AuthTokenInterface;

#[derive(Default)]
pub struct AuthToken(());

impl MintableToken for AuthToken {}
impl AlkaneResponder for AuthToken {}

impl AuthTokenInterface for AuthToken {
    fn initialize(&self, name: String, symbol: String, amount: u128) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;
        self.set_name_and_symbol_str(name, symbol);
        let mut response: CallResponse = CallResponse::forward(&context.incoming_alkanes.clone());
        response.alkanes = context.incoming_alkanes.clone();
        response.alkanes.0.push(AlkaneTransfer {
            id: context.myself.clone(),
            value: amount,
        });
        Ok(response)
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
