use alkanes_runtime::declare_alkane;
use alkanes_runtime::message::MessageDispatch;
use alkanes_runtime::runtime::AlkaneResponder;
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_support::{
    cellpack::Cellpack, context::Context, parcel::AlkaneTransfer, response::CallResponse,
    witness::find_witness_payload,
};
use anyhow::{anyhow, Result};
use bitcoin::blockdata::transaction::Transaction;
use metashrew_support::compat::{to_arraybuffer_layout, to_passback_ptr};
use protorune_support::utils::consensus_decode;

#[derive(Default)]
pub struct Proxy(());

#[derive(MessageDispatch)]
enum ProxyMessage {
    #[opcode(0)]
    Initialize,

    #[opcode(1)]
    CallWitness { witness_index: u128 },

    #[opcode(2)]
    DelegatecallWitness { witness_index: u128 },

    #[opcode(3)]
    CallInputs,

    #[opcode(4)]
    DelegatecallInputs,
}

impl Proxy {
    pub fn pull_incoming(&self, context: &mut Context) -> Option<AlkaneTransfer> {
        let i = context
            .incoming_alkanes
            .0
            .iter()
            .position(|v| v.id == context.myself)?;
        Some(context.incoming_alkanes.0.remove(i))
    }

    pub fn only_owner(&self, v: Option<AlkaneTransfer>) -> Result<()> {
        if let Some(auth) = v {
            if auth.value < 1 {
                Err(anyhow!(
                    "must spend a balance of this alkane to the alkane to use as a proxy"
                ))
            } else {
                Ok(())
            }
        } else {
            Err(anyhow!(
                "must spend a balance of this alkane to the alkane to use as a proxy"
            ))
        }
    }

    fn initialize(&self) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;
        let mut response: CallResponse = CallResponse::default();
        response.alkanes = context.incoming_alkanes.clone();
        response.alkanes.0.push(AlkaneTransfer {
            id: context.myself.clone(),
            value: 1,
        });
        return Ok(response);
    }

    fn call_witness(&self, witness_index: u128) -> Result<CallResponse> {
        let mut context = self.context()?;
        let auth = self.pull_incoming(&mut context);
        self.only_owner(auth.clone())?;

        let tx = consensus_decode::<Transaction>(&mut std::io::Cursor::new(self.transaction()))?;
        let cellpack = Cellpack::parse(&mut std::io::Cursor::new(
            find_witness_payload(&tx, witness_index.try_into()?)
                .ok_or("")
                .map_err(|_| anyhow!("witness envelope not found"))?,
        ))?;

        let mut response: CallResponse =
            self.call(&cellpack, &context.incoming_alkanes, self.fuel())?;
        response.alkanes.0.push(unwrap_auth(auth)?);
        Ok(response)
    }

    fn delegatecall_witness(&self, witness_index: u128) -> Result<CallResponse> {
        let mut context = self.context()?;
        let auth = self.pull_incoming(&mut context);
        self.only_owner(auth.clone())?;

        let tx = consensus_decode::<Transaction>(&mut std::io::Cursor::new(self.transaction()))?;
        let cellpack = Cellpack::parse(&mut std::io::Cursor::new(
            find_witness_payload(&tx, witness_index.try_into()?)
                .ok_or("")
                .map_err(|_| anyhow!("witness envelope not found"))?,
        ))?;

        let mut response: CallResponse =
            self.delegatecall(&cellpack, &context.incoming_alkanes, self.fuel())?;
        response.alkanes.0.push(unwrap_auth(auth)?);
        Ok(response)
    }

    fn call_inputs(&self) -> Result<CallResponse> {
        let mut context = self.context()?;
        let auth = self.pull_incoming(&mut context);
        self.only_owner(auth.clone())?;

        let cellpack: Cellpack = context.inputs.clone().try_into()?;
        let mut response: CallResponse =
            self.call(&cellpack, &context.incoming_alkanes, self.fuel())?;
        response.alkanes.0.push(unwrap_auth(auth)?);
        Ok(response)
    }

    fn delegatecall_inputs(&self) -> Result<CallResponse> {
        let mut context = self.context()?;
        let auth = self.pull_incoming(&mut context);
        self.only_owner(auth.clone())?;

        let cellpack: Cellpack = context.inputs.clone().try_into()?;
        let mut response: CallResponse =
            self.delegatecall(&cellpack, &context.incoming_alkanes, self.fuel())?;
        response.alkanes.0.push(unwrap_auth(auth)?);
        Ok(response)
    }
}

fn unwrap_auth(v: Option<AlkaneTransfer>) -> Result<AlkaneTransfer> {
    v.ok_or("")
        .map_err(|_| anyhow!("authentication token not present"))
}

impl AlkaneResponder for Proxy {}

// Use the new macro format
declare_alkane! {
    impl AlkaneResponder for Proxy {
        type Message = ProxyMessage;
    }
}
