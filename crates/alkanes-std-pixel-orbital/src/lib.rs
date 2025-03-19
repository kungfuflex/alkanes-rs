use alkanes_runtime::declare_alkane;
use alkanes_runtime::message::MessageDispatch;
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_runtime::{runtime::AlkaneResponder, storage::StoragePointer, token::Token};
use alkanes_support::{id::AlkaneId, parcel::AlkaneTransfer, response::CallResponse};
use anyhow::{anyhow, Result};
use hex_lit::hex;
use metashrew_support::compat::to_arraybuffer_layout;
use metashrew_support::index_pointer::KeyValuePointer;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Default)]
pub struct PixelOrbital(());

#[derive(MessageDispatch)]
enum PixelOrbitalMessage {
    #[opcode(0)]
    Initialize {
        pixel_id: u128,
        color_r: u128,
        color_g: u128,
        color_b: u128,
        pattern: u128,
        rarity: u128,
    },

    #[opcode(10)]
    Transfer {
        recipient: u128,
    },

    #[opcode(99)]
    #[returns(String)]
    GetName,

    #[opcode(100)]
    #[returns(String)]
    GetSymbol,

    #[opcode(101)]
    #[returns(u128)]
    GetTotalSupply,

    #[opcode(200)]
    #[returns(Vec<u8>)]
    GetPixelMetadata,

    #[opcode(201)]
    #[returns(Vec<u8>)]
    GetImage,

    #[opcode(202)]
    #[returns(Vec<u8>)]
    GetPixelOwner,

    #[opcode(203)]
    #[returns(Vec<u8>)]
    GetPixelCollection,

    #[opcode(1000)]
    #[returns(Vec<u8>)]
    GetData,
}

// Implement pixel metadata structure
#[derive(Serialize, Deserialize, Clone)]
pub struct PixelMetadata {
    id: u64,
    color: [u8; 3],  // RGB color
    pattern: u8,     // Pattern type (0-255)
    rarity: u8,      // Rarity score (0-100)
}

impl Token for PixelOrbital {
    fn name(&self) -> String {
        format!("Pixel #{}", self.get_pixel_id())
    }
    fn symbol(&self) -> String {
        String::from("PIXEL")
    }
}

impl PixelOrbital {
    pub fn total_supply_pointer(&self) -> StoragePointer {
        StoragePointer::from_keyword("/totalsupply")
    }
    
    pub fn set_total_supply(&self, v: u128) {
        self.total_supply_pointer().set_value::<u128>(v);
    }
    
    pub fn observe_initialization(&self) -> Result<()> {
        let mut initialized_pointer = StoragePointer::from_keyword("/initialized");
        if initialized_pointer.get().len() == 0 {
            initialized_pointer.set_value::<u32>(1);
            Ok(())
        } else {
            Err(anyhow!("already initialized"))
        }
    }
    
    // Get pixel ID
    pub fn get_pixel_id(&self) -> u64 {
        let pointer = StoragePointer::from_keyword("/pixel_id");
        if pointer.get().len() == 0 {
            0
        } else {
            pointer.get_value::<u64>()
        }
    }
    
    // Set pixel ID
    pub fn set_pixel_id(&self, id: u64) {
        let mut pointer = StoragePointer::from_keyword("/pixel_id");
        pointer.set_value::<u64>(id);
    }
    
    // Get pixel metadata
    pub fn get_metadata(&self) -> Option<PixelMetadata> {
        let pointer = StoragePointer::from_keyword("/metadata");
        if pointer.get().len() == 0 {
            None
        } else {
            serde_json::from_slice(&pointer.get()).ok()
        }
    }
    
    // Store pixel metadata
    pub fn store_metadata(&self, metadata: &PixelMetadata) {
        let mut pointer = StoragePointer::from_keyword("/metadata");
        pointer.set(Arc::new(serde_json::to_vec(metadata).unwrap()));
    }
    
    // Get pixel owner
    pub fn get_owner(&self) -> Vec<u8> {
        let pointer = StoragePointer::from_keyword("/owner");
        pointer.get().as_ref().clone()
    }
    
    // Set pixel owner
    pub fn set_owner(&self, owner: &[u8]) {
        let mut pointer = StoragePointer::from_keyword("/owner");
        pointer.set(Arc::new(owner.to_vec()));
    }
    
    // Get collection ID
    pub fn get_collection(&self) -> AlkaneId {
        let pointer = StoragePointer::from_keyword("/collection");
        if pointer.get().len() == 0 {
            AlkaneId { block: 0, tx: 0 }
        } else {
            pointer.get().as_ref().clone().try_into().unwrap_or_else(|_| {
                AlkaneId { block: 0, tx: 0 }
            })
        }
    }
    
    // Set collection ID
    pub fn set_collection(&self, collection_id: &AlkaneId) {
        let mut pointer = StoragePointer::from_keyword("/collection");
        pointer.set(Arc::new(<AlkaneId as Into<Vec<u8>>>::into(collection_id.clone())));
    }
    
    // Generate a PNG image for a pixel
    pub fn generate_pixel_image(&self, metadata: &PixelMetadata) -> Vec<u8> {
        // Simple 16x16 PNG with the pixel's color
        // In a real implementation, this would generate a more complex image
        // based on the pattern and other attributes
        let [_r, _g, _b] = metadata.color;
        
        // Return a simple colored PNG
        // This is a placeholder - in a real implementation, we would use the image crate
        // to generate a proper PNG
        (&hex!("89504e470d0a1a0a0000000d494844520000001000000010010300000025db56ca00000003504c5445000000a77a3dda0000000174524e530040e6d8660000000a4944415408d76360000000020001e221bc330000000049454e44ae426082")).to_vec()
    }
    
    // Method for opcode 0: Initialize
    fn initialize(&self, pixel_id: u128, color_r: u128, color_g: u128, color_b: u128, pattern: u128, rarity: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        self.observe_initialization()?;
        
        // Set total supply to 1 (fixed for orbitals)
        self.set_total_supply(1);
        
        // Set pixel ID
        self.set_pixel_id(pixel_id as u64);
        
        // Create and store metadata
        let metadata = PixelMetadata {
            id: pixel_id as u64,
            color: [color_r as u8, color_g as u8, color_b as u8],
            pattern: pattern as u8,
            rarity: rarity as u8,
        };
        self.store_metadata(&metadata);
        
        // Set owner (default to caller)
        let caller_bytes: Vec<u8> = (&context.caller).into();
        self.set_owner(&caller_bytes);
        
        // Set collection ID (default to 0,0 - will be updated by collection)
        let collection_id = AlkaneId { block: 0, tx: 0 };
        self.set_collection(&collection_id);
        
        // Transfer the NFT to the owner
        response.alkanes.0.push(AlkaneTransfer {
            id: context.myself.clone(),
            value: 1u128,
        });
        
        Ok(response)
    }
    
    // Method for opcode 1: Transfer
    fn transfer(&self, recipient: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        // Check if the caller is the current owner
        let current_owner = self.get_owner();
        let caller_bytes: Vec<u8> = (&context.caller).into();
        
        if current_owner != caller_bytes {
            return Err(anyhow!("Only the owner can transfer this pixel"));
        }
        
        // Convert recipient to bytes
        let recipient_bytes = recipient.to_le_bytes().to_vec();
        
        // Update the owner
        self.set_owner(&recipient_bytes);
        
        Ok(response)
    }
    
    // Method for opcode 99: GetName
    fn get_name(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        response.data = self.name().into_bytes().to_vec();
        
        Ok(response)
    }
    
    // Method for opcode 100: GetSymbol
    fn get_symbol(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        response.data = self.symbol().into_bytes().to_vec();
        
        Ok(response)
    }
    
    // Method for opcode 101: GetTotalSupply
    fn get_total_supply(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        response.data = (&self.total_supply_pointer().get_value::<u128>().to_le_bytes()).to_vec();
        
        Ok(response)
    }
    
    // Method for opcode 200: GetPixelMetadata
    fn get_pixel_metadata(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        let metadata = self.get_metadata().ok_or_else(|| anyhow!("Metadata not found"))?;
        response.data = serde_json::to_vec(&metadata).unwrap();
        
        Ok(response)
    }
    
    // Method for opcode 201: GetImage
    fn get_image(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        let metadata = self.get_metadata().ok_or_else(|| anyhow!("Metadata not found"))?;
        response.data = self.generate_pixel_image(&metadata);
        
        Ok(response)
    }
    
    // Method for opcode 202: GetPixelOwner
    fn get_pixel_owner(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        response.data = self.get_owner();
        
        Ok(response)
    }
    
    // Method for opcode 203: GetPixelCollection
    fn get_pixel_collection(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        let collection_id = self.get_collection();
        response.data = <AlkaneId as Into<Vec<u8>>>::into(collection_id);
        
        Ok(response)
    }
    
    // Method for opcode 1000: GetData
    fn get_data(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        // Return metadata as data
        let metadata = self.get_metadata().ok_or_else(|| anyhow!("Metadata not found"))?;
        response.data = serde_json::to_vec(&metadata).unwrap();
        
        Ok(response)
    }
}

impl AlkaneResponder for PixelOrbital {
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
    impl AlkaneResponder for PixelOrbital {
        type Message = PixelOrbitalMessage;
    }
}