use alkanes_runtime::runtime::AlkaneResponder;
use alkanes_runtime::{auth::AuthenticatedResponder, declare_alkane};
use alkanes_std_owned_token::factory::MintableToken;
use alkanes_support::utils::{shift_or_err, shift_bytes32_or_err};
use alkanes_support::{parcel::AlkaneTransfer, response::CallResponse};
use alkanes_runtime::storage::StoragePointer;
use anyhow::{anyhow, Result};
use rs_merkle::{algorithms::Sha256, Hasher, MerkleProof};
use std::io::Cursor;
use metashrew_support::utils::{consume_exact, consume_sized_int};

#[derive(Default)]
pub struct VestingToken(());

impl MintableToken for VestingToken {}
impl AuthenticatedResponder for VestingToken {}

// Vesting schedule structure
#[derive(Debug)]
pub struct VestingSchedule {
    start_time: u64,
    cliff_duration: u64,
    total_duration: u64,
    total_amount: u128,
    released_amount: u128,
    allocated_tokens: u128,  // Tracks tokens allocated for this schedule
}

impl VestingToken {
    // Merkle tree storage
    fn merkle_root_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/merkle_root")
    }

    fn merkle_length_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/merkle_length")
    }

    fn get_merkle_root(&self) -> Result<[u8; 32]> {
        let root_vec = self.merkle_root_pointer().get().as_ref().clone();
        root_vec.try_into()
            .map_err(|_| anyhow!("Invalid merkle root length"))
    }

    fn set_merkle_root(&self, root: Vec<u8>) {
        self.merkle_root_pointer().set(std::sync::Arc::new(root));
    }

    fn get_merkle_length(&self) -> usize {
        self.merkle_length_pointer().get_value::<usize>()
    }

    fn set_merkle_length(&self, length: usize) {
        self.merkle_length_pointer().set_value(length);
    }

    // Existing vesting functionality
    fn vesting_schedule_pointer(&self, beneficiary: &[u8]) -> StoragePointer {
        let mut pointer = StoragePointer::from_keyword("/vesting");
        pointer.extend(beneficiary);
        pointer
    }

    fn allocated_tokens_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/allocated_tokens")
    }

    fn get_allocated_tokens(&self) -> u128 {
        self.allocated_tokens_pointer().get_value::<u128>()
    }

    fn increase_allocated_tokens(&self, amount: u128) -> Result<()> {
        let current = self.get_allocated_tokens();
        let new_amount = current.checked_add(amount)
            .ok_or_else(|| anyhow!("Allocation overflow"))?;
        self.allocated_tokens_pointer().set_value(new_amount);
        Ok(())
    }

    fn verify_merkle_proof(&self, leaf: &[u8], proof: &[u8], index: usize) -> Result<bool> {
        let leaf_hash = Sha256::hash(leaf);
        Ok(MerkleProof::<Sha256>::try_from(proof.to_vec())?
            .verify(self.get_merkle_root()?, &[index], &[leaf_hash], self.get_merkle_length()))
    }

    fn create_vesting_schedule_with_proof(
        &self,
        beneficiary: &[u8],
        start_time: u64,
        cliff_duration: u64,
        total_duration: u64,
        total_amount: u128,
        merkle_proof: &[u8],
        index: usize,
    ) -> Result<()> {
        // Create leaf data that matches the merkle tree structure
        let mut leaf_data = Vec::new();
        leaf_data.extend_from_slice(beneficiary);
        leaf_data.extend_from_slice(&start_time.to_le_bytes());
        leaf_data.extend_from_slice(&cliff_duration.to_le_bytes());
        leaf_data.extend_from_slice(&total_duration.to_le_bytes());
        leaf_data.extend_from_slice(&total_amount.to_le_bytes());

        // Verify merkle proof
        if !self.verify_merkle_proof(&leaf_data, merkle_proof, index)? {
            return Err(anyhow!("Invalid merkle proof"));
        }

        // Continue with regular vesting schedule creation
        if cliff_duration > total_duration {
            return Err(anyhow!("Cliff duration cannot be greater than total duration"));
        }

        let allocated = self.get_allocated_tokens();
        let total = self.total_supply();
        
        if allocated.checked_add(total_amount).ok_or_else(|| anyhow!("Overflow"))? > total {
            return Err(anyhow!("Not enough tokens available for allocation"));
        }

        let schedule = VestingSchedule {
            start_time,
            cliff_duration,
            total_duration,
            total_amount,
            released_amount: 0,
            allocated_tokens: total_amount,
        };

        self.increase_allocated_tokens(total_amount)?;
        let pointer = self.vesting_schedule_pointer(beneficiary);
        pointer.set_value(&schedule);
        Ok(())
    }

    fn get_vesting_schedule(&self, beneficiary: &[u8]) -> Option<VestingSchedule> {
        let pointer = self.vesting_schedule_pointer(beneficiary);
        pointer.get_value()
    }

    fn calculate_releasable_amount(&self, schedule: &VestingSchedule, current_time: u64) -> u128 {
        if current_time < schedule.start_time + schedule.cliff_duration {
            return 0;
        }

        if current_time >= schedule.start_time + schedule.total_duration {
            return schedule.total_amount.saturating_sub(schedule.released_amount);
        }

        let time_from_start = current_time.saturating_sub(schedule.start_time);
        let vested_amount = schedule.total_amount
            .saturating_mul(time_from_start as u128)
            .checked_div(schedule.total_duration as u128)
            .unwrap_or(0);

        vested_amount.saturating_sub(schedule.released_amount)
    }

    fn release(&self, beneficiary: &[u8], current_time: u64) -> Result<u128> {
        let mut schedule = self.get_vesting_schedule(beneficiary)
            .ok_or_else(|| anyhow!("No vesting schedule found for beneficiary"))?;

        let releasable = self.calculate_releasable_amount(&schedule, current_time);
        if releasable == 0 {
            return Err(anyhow!("No tokens are due for release"));
        }

        schedule.released_amount = schedule.released_amount.saturating_add(releasable);
        let pointer = self.vesting_schedule_pointer(beneficiary);
        pointer.set_value(&schedule);

        Ok(releasable)
    }
}

impl AlkaneResponder for VestingToken {
    fn execute(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut inputs = context.inputs.clone();
        let mut response: CallResponse = CallResponse::forward(&context.incoming_alkanes.clone());

        match shift_or_err(&mut inputs)? {
            // Initialize token with total supply and merkle root
            0 => {
                self.observe_initialization()?;
                let auth_token_units = shift_or_err(&mut inputs)?;
                let token_units = shift_or_err(&mut inputs)?;
                let merkle_length = shift_or_err::<usize>(&mut inputs)?;
                let merkle_root = shift_bytes32_or_err(&mut inputs)?;
                
                // Initialize storage
                self.allocated_tokens_pointer().set_value::<u128>(0);
                self.set_merkle_length(merkle_length);
                self.set_merkle_root(merkle_root);
                
                response.alkanes.0.push(self.deploy_auth_token(auth_token_units)?);
                response.alkanes.0.push(AlkaneTransfer {
                    id: context.myself.clone(),
                    value: token_units,
                });
                Ok(response)
            }
            // Create vesting schedule with merkle proof
            78 => {
                let beneficiary = shift_or_err::<Vec<u8>>(&mut inputs)?;
                let start_time = shift_or_err(&mut inputs)?;
                let cliff_duration = shift_or_err(&mut inputs)?;
                let total_duration = shift_or_err(&mut inputs)?;
                let total_amount = shift_or_err(&mut inputs)?;
                let merkle_proof = shift_or_err::<Vec<u8>>(&mut inputs)?;
                let index = shift_or_err(&mut inputs)?;
                
                self.create_vesting_schedule_with_proof(
                    &beneficiary,
                    start_time,
                    cliff_duration,
                    total_duration,
                    total_amount,
                    &merkle_proof,
                    index,
                )?;
                Ok(response)
            }
            // Release vested tokens
            79 => {
                let beneficiary = shift_or_err::<Vec<u8>>(&mut inputs)?;
                let current_time = shift_or_err(&mut inputs)?;
                
                let amount = self.release(&beneficiary, current_time)?;
                response.alkanes.0.push(AlkaneTransfer {
                    id: context.myself.clone(),
                    value: amount,
                });
                Ok(response)
            }
            // Get allocated tokens
            80 => {
                response.data = self.get_allocated_tokens().to_le_bytes().to_vec();
                Ok(response)
            }
            // Standard ERC20 operations
            99 => {
                response.data = self.name().into_bytes().to_vec();
                Ok(response)
            }
            100 => {
                response.data = self.symbol().into_bytes().to_vec();
                Ok(response)
            }
            101 => {
                response.data = self.total_supply().to_le_bytes().to_vec();
                Ok(response)
            }
            1000 => {
                response.data = self.data();
                Ok(response)
            }
            _ => Err(anyhow!("unrecognized opcode")),
        }
    }
}

declare_alkane! {VestingToken} 