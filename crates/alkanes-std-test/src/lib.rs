use alkanes_runtime::{declare_alkane, message::MessageDispatch, runtime::AlkaneResponder};
use alkanes_support::{
    cellpack::Cellpack,
    id::AlkaneId,
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

    #[opcode(11)]
    ProcessNumbers { numbers: Vec<u128> },

    #[opcode(12)]
    ProcessStrings { strings: Vec<String> },

    #[opcode(13)]
    ProcessNestedVec { nested: Vec<Vec<u128>> },

    #[opcode(20)]
    TestInfiniteLoop,

    #[opcode(21)]
    TestInfiniteExtcall,

    #[opcode(30)]
    TestArbitraryMint { alkane: AlkaneId, amount: u128 },

    #[opcode(31)]
    TestExtCall { target: AlkaneId, inputs: Vec<u128> },

    #[opcode(32)]
    TestExtDelegateCall { target: AlkaneId, inputs: Vec<u128> },

    #[opcode(33)]
    TestStaticCall { target: AlkaneId, inputs: Vec<u128> },

    #[opcode(34)]
    TestMultipleExtCall {
        target: AlkaneId,
        inputs: Vec<u128>,
        target2: AlkaneId,
        inputs2: Vec<u128>,
    },

    #[opcode(40)]
    TestLargeTransferParcel,

    #[opcode(41)]
    TestLargeTransferParcelExtcall,

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

    fn test_infinite_loop(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        loop {}

        Ok(response)
    }

    fn test_infinite_extcall(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let cellpack = Cellpack {
            target: context.myself,
            inputs: vec![21],
        };
        let response = self.call(&cellpack, &context.incoming_alkanes)?;
        Ok(response)
    }

    fn test_arbitrary_mint(&self, alkane: AlkaneId, amount: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.alkanes.pay(AlkaneTransfer {
            id: alkane,
            value: amount,
        });

        Ok(response)
    }

    fn test_ext_call(&self, target: AlkaneId, inputs: Vec<u128>) -> Result<CallResponse> {
        let context = self.context()?;
        let cellpack = Cellpack {
            target: target,
            inputs: inputs,
        };
        let response = self.call(&cellpack, &context.incoming_alkanes)?;
        Ok(response)
    }

    fn test_ext_delegate_call(&self, target: AlkaneId, inputs: Vec<u128>) -> Result<CallResponse> {
        let context = self.context()?;
        let cellpack = Cellpack {
            target: target,
            inputs: inputs,
        };
        let response = self.delegatecall(&cellpack, &context.incoming_alkanes)?;
        Ok(response)
    }

    fn test_static_call(&self, target: AlkaneId, inputs: Vec<u128>) -> Result<CallResponse> {
        let context = self.context()?;
        let cellpack = Cellpack {
            target: target,
            inputs: inputs,
        };
        let response = self.staticcall(&cellpack, &context.incoming_alkanes)?;
        Ok(response)
    }

    fn test_multiple_ext_call(
        &self,
        target: AlkaneId,
        inputs: Vec<u128>,
        target2: AlkaneId,
        inputs2: Vec<u128>,
    ) -> Result<CallResponse> {
        let context = self.context()?;
        let cellpack = Cellpack {
            target: target,
            inputs: inputs,
        };
        let _ = self.call(&cellpack, &context.incoming_alkanes); // allow to fail

        let cellpack2 = Cellpack {
            target: target2,
            inputs: inputs2,
        };
        let _ = self.call(&cellpack2, &context.incoming_alkanes); // allow to fail
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }

    fn test_large_transfer_parcel(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.alkanes = AlkaneTransferParcel(vec![AlkaneTransfer {
            id: context.myself,
            value: u128::MAX, // Extremely large value
        }]);

        Ok(response)
    }

    fn test_large_transfer_parcel_extcall(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let cellpack = Cellpack {
            target: context.myself,
            inputs: vec![5],
        };
        let response = self.call(
            &cellpack,
            &AlkaneTransferParcel(vec![AlkaneTransfer {
                id: context.myself,
                value: u128::MAX, // Extremely large value
            }]),
        )?;
        Ok(response)
    }

    fn return_default_data(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.data = vec![0x01, 0x02, 0x03, 0x04];

        Ok(response)
    }

    fn process_numbers(&self, numbers: Vec<u128>) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        // Sum the numbers and store in response data
        let sum: u128 = numbers.iter().sum();
        response.data = sum.to_le_bytes().to_vec();

        Ok(response)
    }

    fn process_strings(&self, strings: Vec<String>) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        // Concatenate the strings and store in response data
        let concat = strings.join(",");
        response.data = concat.into_bytes();

        Ok(response)
    }

    fn process_nested_vec(&self, nested: Vec<Vec<u128>>) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        // Count total elements in the nested vector
        let total_elements: usize = nested.iter().map(|v| v.len()).sum();
        response.data = (total_elements as u128).to_le_bytes().to_vec();

        Ok(response)
    }
}

impl AlkaneResponder for LoggerAlkane {}

// Use the new macro format
declare_alkane! {
    impl AlkaneResponder for LoggerAlkane {
        type Message = LoggerAlkaneMessage;
    }
}
