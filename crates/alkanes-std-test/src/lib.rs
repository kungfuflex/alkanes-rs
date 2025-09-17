use std::collections::BTreeSet;

use alkanes_runtime::{
    declare_alkane, message::MessageDispatch, runtime::AlkaneResponder, storage::StoragePointer,
};
use alkanes_support::{
    cellpack::Cellpack,
    id::AlkaneId,
    parcel::{AlkaneTransfer, AlkaneTransferParcel},
    response::CallResponse,
};
use anyhow::{anyhow, Result};
use metashrew_support::{
    compat::to_arraybuffer_layout,
    index_pointer::KeyValuePointer,
    utils::consensus_encode,
};
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
    #[opcode(0)]
    Initialize,

    #[opcode(2)]
    SelfCall,

    #[opcode(3)]
    CheckIncoming,

    #[opcode(4)]
    MintTokens,

    #[opcode(5)]
    #[returns(Vec<u8>)]
    ReturnData1,

    #[opcode(6)]
    #[returns(Vec<u8>)]
    TestOrderedIncoming,

    #[opcode(7)]
    Donate,

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

    #[opcode(22)]
    TestSelfMint { amount: u128 },

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

    #[opcode(100)]
    Revert,

    #[opcode(101)]
    MyGetBlockHeader,

    #[opcode(102)]
    MyGetCoinbaseTx,

    #[opcode(103)]
    ClaimableFees,

    #[opcode(104)]
    SetClaimableFees { v: u128 },

    #[opcode(105)]
    IncClaimableFees,

    #[opcode(106)]
    MyGetNumberDieselMints,

    #[opcode(107)]
    MyGetTotalMinerFee,

    #[opcode(110)]
    TestExtCallReturnLeftovers { target: AlkaneId, inputs: Vec<u128> },
}

impl LoggerAlkane {
    fn initialize(&self) -> Result<CallResponse> {
        self.observe_initialization()?;
        let context = self.context()?;
        let response: CallResponse = CallResponse::forward(&context.incoming_alkanes);
        Ok(response)
    }
    fn claimable_fees_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/claimablefees")
    }
    fn claimable_fees(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        let fees = self.claimable_fees_pointer().get_value::<u128>();
        response.data = fees.to_le_bytes().to_vec();
        Ok(response)
    }
    fn set_claimable_fees(&self, v: u128) -> Result<CallResponse> {
        let context = self.context()?;
        self.claimable_fees_pointer().set_value::<u128>(v);
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }
    fn inc_claimable_fees(&self) -> Result<CallResponse> {
        let context = self.context()?;
        self.claimable_fees_pointer()
            .set_value::<u128>(self.claimable_fees_pointer().get_value::<u128>() + 1);
        Ok(CallResponse::forward(&context.incoming_alkanes))
    }
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

    fn donate(&self) -> Result<CallResponse> {
        Ok(CallResponse::default())
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

    fn test_ordered_incoming(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let response = CallResponse::forward(&context.incoming_alkanes);
        let transfers = context.incoming_alkanes.0;
        println!("{:?}", transfers);
        for i in 1..transfers.len() {
            if transfers[i] < transfers[i - 1] {
                return Err(anyhow!("Not sorted in ascending order"));
            }
        }

        Ok(response)
    }

    fn get_transaction(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let response = CallResponse::forward(&context.incoming_alkanes);

        self.transaction();

        Ok(response)
    }

    fn hash_loop(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let response = CallResponse::forward(&context.incoming_alkanes);

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
        let response = CallResponse::forward(&context.incoming_alkanes);

        loop {}

        Ok(response)
    }

    fn test_infinite_extcall(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let cellpack = Cellpack {
            target: context.myself,
            inputs: vec![21],
        };
        let response = self.call(&cellpack, &context.incoming_alkanes, u64::MAX)?;
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

    fn test_self_mint(&self, amount: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);

        response.alkanes.pay(AlkaneTransfer {
            id: context.myself,
            value: amount,
        });

        Ok(response)
    }

    fn _return_leftovers(
        &self,
        myself: AlkaneId,
        result: CallResponse,
        input_alkanes: AlkaneTransferParcel,
    ) -> Result<CallResponse> {
        let mut response = CallResponse::default();
        let mut unique_ids: BTreeSet<AlkaneId> = BTreeSet::new();
        for transfer in input_alkanes.0 {
            unique_ids.insert(transfer.id);
        }
        for transfer in result.alkanes.0 {
            unique_ids.insert(transfer.id);
        }
        for id in unique_ids {
            response.alkanes.pay(AlkaneTransfer {
                id: id,
                value: self.balance(&myself, &id),
            });
        }
        Ok(response)
    }

    fn test_ext_call(&self, target: AlkaneId, inputs: Vec<u128>) -> Result<CallResponse> {
        let context = self.context()?;
        let cellpack = Cellpack {
            target: target,
            inputs: inputs,
        };
        let response = self.call(&cellpack, &context.incoming_alkanes, self.fuel())?;
        Ok(response)
    }

    fn test_ext_call_return_leftovers(
        &self,
        target: AlkaneId,
        inputs: Vec<u128>,
    ) -> Result<CallResponse> {
        let context = self.context()?;
        let cellpack = Cellpack {
            target: target,
            inputs: inputs,
        };
        let response = self.call(&cellpack, &context.incoming_alkanes, self.fuel())?;
        self._return_leftovers(context.myself, response, context.incoming_alkanes)
    }

    fn test_ext_delegate_call(&self, target: AlkaneId, inputs: Vec<u128>) -> Result<CallResponse> {
        let context = self.context()?;
        let cellpack = Cellpack {
            target: target,
            inputs: inputs,
        };
        let response = self.delegatecall(&cellpack, &context.incoming_alkanes, self.fuel())?;
        Ok(response)
    }

    fn test_static_call(&self, target: AlkaneId, inputs: Vec<u128>) -> Result<CallResponse> {
        let context = self.context()?;
        let cellpack = Cellpack {
            target: target,
            inputs: inputs,
        };
        let response = self.staticcall(&cellpack, &context.incoming_alkanes, self.fuel())?;
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
        let _ = self.call(&cellpack, &context.incoming_alkanes, self.fuel()); // allow to fail

        let cellpack2 = Cellpack {
            target: target2,
            inputs: inputs2,
        };
        let _ = self.call(&cellpack2, &context.incoming_alkanes, self.fuel()); // allow to fail
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
            self.fuel(),
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

    fn revert(&self) -> Result<CallResponse> {
        Err(anyhow!("Revert"))
    }

    fn my_get_block_header(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        let header = self.block_header()?;
        response.data = consensus_encode(&header)?;

        Ok(response)
    }

    fn my_get_coinbase_tx(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        let tx = self.coinbase_tx()?;
        response.data = consensus_encode(&tx)?;

        Ok(response)
    }

    fn my_get_number_diesel_mints(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        let v = self.number_diesel_mints()?;
        response.data = v.to_le_bytes().to_vec();

        Ok(response)
    }

    fn my_get_total_miner_fee(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        let v = self.total_miner_fee()?;
        response.data = v.to_le_bytes().to_vec();

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
