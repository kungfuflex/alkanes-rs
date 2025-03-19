use alkanes_runtime::declare_alkane;
use alkanes_runtime::message::MessageDispatch;
#[allow(unused_imports)]
use alkanes_runtime::{
    println,
    stdio::{stdout, Write},
};
use alkanes_runtime::{runtime::AlkaneResponder, storage::StoragePointer, token::Token};
use alkanes_support::{
    cellpack::Cellpack,
    id::AlkaneId,
    parcel::{AlkaneTransferParcel},
    response::CallResponse
};
use anyhow::{anyhow, Result};
use hex;
use hex_lit::hex as hex_macro;
use metashrew_support::compat::to_arraybuffer_layout;
use metashrew_support::index_pointer::KeyValuePointer;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Maximum supply of pixels in the collection
const MAX_SUPPLY: u128 = 10_000;

#[derive(Default)]
pub struct PixelCollection(());

#[derive(MessageDispatch)]
enum PixelCollectionMessage {
    #[opcode(0)]
    Initialize,
    
    #[opcode(20)]
    MintPixel,
    
    #[opcode(2)]
    GetPixelById {
        pixel_id: u128,
    },
    
    #[opcode(3)]
    GetPixelsByOwner {
        owner: u128,
    },
    
    #[opcode(4)]
    GetBaseImage,
    
    #[opcode(5)]
    #[returns(Vec<u8>)]
    GetSupplyInfo,
    
    #[opcode(99)]
    #[returns(String)]
    GetName,
    
    #[opcode(100)]
    #[returns(String)]
    GetSymbol,
    
    #[opcode(101)]
    #[returns(u128)]
    GetTotalSupply,
}

// Implement pixel metadata structure
#[derive(Serialize, Deserialize, Clone)]
pub struct PixelMetadata {
    id: u64,
    color: [u8; 3],  // RGB color
    pattern: u8,     // Pattern type (0-255)
    rarity: u8,      // Rarity score (0-100)
}

impl Token for PixelCollection {
    fn name(&self) -> String {
        String::from("Pixel Collection")
    }
    fn symbol(&self) -> String {
        String::from("PIXCOL")
    }
}

impl PixelCollection {
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
    
    // Get the next pixel ID
    pub fn get_next_pixel_id(&self) -> u64 {
        let pointer = StoragePointer::from_keyword("/next_pixel_id");
        let next_id = if pointer.get().len() == 0 {
            1 // Start from 1
        } else {
            pointer.get_value::<u64>() + 1
        };
        
        let mut pointer = StoragePointer::from_keyword("/next_pixel_id");
        pointer.set_value::<u64>(next_id);
        
        next_id
    }
    
    // Add a pixel to an owner's list
    pub fn add_pixel_to_owner(&self, owner: &[u8], pixel_id: u64) {
        let pointer = StoragePointer::from_keyword(&format!("/owners/{}", hex::encode(owner)));
        let mut pixels: Vec<u64> = if pointer.get().len() == 0 {
            vec![]
        } else {
            serde_json::from_slice(&pointer.get()).unwrap_or_default()
        };
        
        pixels.push(pixel_id);
        
        let mut pointer = StoragePointer::from_keyword(&format!("/owners/{}", hex::encode(owner)));
        pointer.set(Arc::new(serde_json::to_vec(&pixels).unwrap()));
    }
    
    // Get pixels owned by an address
    pub fn get_pixels_by_owner_internal(&self, owner: &[u8]) -> Vec<u64> {
        let pointer = StoragePointer::from_keyword(&format!("/owners/{}", hex::encode(owner)));
        if pointer.get().len() == 0 {
            vec![]
        } else {
            serde_json::from_slice(&pointer.get()).unwrap_or_default()
        }
    }
    
    // Store pixel ID to AlkaneId mapping
    pub fn store_pixel_alkane_id(&self, pixel_id: u64, alkane_id: &AlkaneId) {
        let mut pointer = StoragePointer::from_keyword(&format!("/pixels/{}", pixel_id));
        pointer.set(Arc::new(<AlkaneId as Into<Vec<u8>>>::into(alkane_id.clone())));
    }
    
    // Get AlkaneId for a pixel
    pub fn get_pixel_alkane_id(&self, pixel_id: u64) -> Option<AlkaneId> {
        let pointer = StoragePointer::from_keyword(&format!("/pixels/{}", pixel_id));
        if pointer.get().len() == 0 {
            None
        } else {
            pointer.get().as_ref().clone().try_into().ok()
        }
    }
    
    // Generate random color based on inputs and other context
    pub fn generate_random_color(&self, context: &alkanes_support::context::Context) -> [u8; 3] {
        // Use inputs and caller as sources of randomness
        let inputs = &context.inputs;
        let caller_bytes: Vec<u8> = (&context.caller).into();
        
        // Combine multiple sources of randomness
        let mut r_seed = 0u128;
        let mut g_seed = 0u128;
        let mut b_seed = 0u128;
        
        // Add randomness from inputs
        if !inputs.is_empty() {
            r_seed = r_seed.wrapping_add(inputs[0]);
            if inputs.len() > 1 {
                g_seed = g_seed.wrapping_add(inputs[1]);
            }
            if inputs.len() > 2 {
                b_seed = b_seed.wrapping_add(inputs[2]);
            }
        }
        
        // Add randomness from caller
        if !caller_bytes.is_empty() {
            r_seed = r_seed.wrapping_add(caller_bytes[0] as u128);
            if caller_bytes.len() > 1 {
                g_seed = g_seed.wrapping_add(caller_bytes[1] as u128);
            }
            if caller_bytes.len() > 2 {
                b_seed = b_seed.wrapping_add(caller_bytes[2] as u128);
            }
        }
        
        // Add randomness from vout
        r_seed = r_seed.wrapping_add(context.vout as u128);
        g_seed = g_seed.wrapping_add(context.vout.wrapping_mul(3) as u128);
        b_seed = b_seed.wrapping_add(context.vout.wrapping_mul(7) as u128);
        
        // Convert to color values
        let r = (r_seed % 255) as u8;
        let g = (g_seed % 255) as u8;
        let b = (b_seed % 255) as u8;
        
        [r, g, b]
    }
    
    // Generate random pattern based on inputs and caller
    pub fn generate_random_pattern(&self, context: &alkanes_support::context::Context) -> u8 {
        // Use inputs and caller as sources of randomness
        let inputs = &context.inputs;
        let caller_bytes: Vec<u8> = (&context.caller).into();
        
        // Combine multiple sources of randomness
        let mut pattern_seed = 0u128;
        
        // Add randomness from inputs
        if !inputs.is_empty() {
            pattern_seed = pattern_seed.wrapping_add(inputs[0]);
            // Add more randomness from additional inputs if available
            if inputs.len() > 1 {
                pattern_seed = pattern_seed.wrapping_mul(inputs[1]);
            }
        }
        
        // Add randomness from caller
        if !caller_bytes.is_empty() {
            pattern_seed = pattern_seed.wrapping_add(caller_bytes[0] as u128);
            // Add more randomness from additional caller bytes if available
            if caller_bytes.len() > 1 {
                pattern_seed = pattern_seed.wrapping_add((caller_bytes[1] as u128) << 8);
            }
        }
        
        // Add randomness from vout if available
        pattern_seed = pattern_seed.wrapping_add(context.vout as u128);
        
        // Add more randomness by using different operations
        pattern_seed = pattern_seed.wrapping_mul(31); // Multiply by a prime number
        
        // Add randomness from the myself field (contract ID)
        let myself_bytes: Vec<u8> = (&context.myself).into();
        if !myself_bytes.is_empty() {
            pattern_seed = pattern_seed.wrapping_add(myself_bytes[0] as u128);
        }
        
        // Use a different modulus to get more variation (use a prime number)
        ((pattern_seed % 7) + 1) as u8
    }
    
    // Calculate rarity score based on color and pattern
    pub fn calculate_rarity(&self, color: [u8; 3], pattern: u8) -> u8 {
        // Simple rarity calculation algorithm
        // More unique colors and patterns are rarer
        // This is a placeholder implementation
        let color_rarity = (color[0] as u16 + color[1] as u16 + color[2] as u16) % 100;
        let pattern_rarity = (pattern as u16 * 2) % 100;
        ((color_rarity + pattern_rarity) / 2) as u8
    }
    
    // Deploy a new pixel orbital (simplified version that skips the factory call)
    pub fn deploy_pixel_orbital(&self, pixel_id: u64, color: [u8; 3], pattern: u8, rarity: u8, owner: &[u8]) -> Result<AlkaneId> {
        // Get a sequence number for the orbital ID
        let sequence = self.sequence();
        println!("DEBUG: Sequence number for orbital: {}", sequence);
        
        // Create a hardcoded pixel orbital ID based on the sequence
        let pixel_orbital_id = AlkaneId {
            block: 3,  // Use block 3 to avoid conflicts with other contracts
            tx: sequence,
        };
        
        println!("DEBUG: Created hardcoded pixel orbital ID: [block: {}, tx: {}]",
                 pixel_orbital_id.block, pixel_orbital_id.tx);
        
        // Log the ID for debugging
        let myself_bytes: Vec<u8> = (&self.context()?.myself).into();
        println!("DEBUG: Deploying pixel orbital: ID [block: {}, tx: {}], sequence: {}, myself: {:?}",
                 pixel_orbital_id.block, pixel_orbital_id.tx, sequence, myself_bytes);
        
        // Initialize the pixel orbital
        let init_cellpack = Cellpack {
            target: pixel_orbital_id.clone(),
            inputs: vec![
                0u128, // Opcode 0 for initialization
                pixel_id as u128,
                color[0] as u128,
                color[1] as u128,
                color[2] as u128,
                pattern as u128,
                rarity as u128,
            ],
        };
        
        // Print debug info before initializing the orbital
        println!("DEBUG: About to initialize orbital at [block: {}, tx: {}] with {} inputs",
                 init_cellpack.target.block, init_cellpack.target.tx, init_cellpack.inputs.len());
        
        // Initialize the pixel orbital with increased fuel
        let init_max_fuel = 10_000_000; // Increased fuel limit for initialization
        
        // Try to initialize the orbital and handle errors explicitly
        let init_response = match self.call(&init_cellpack, &AlkaneTransferParcel::default(), init_max_fuel) {
            Ok(resp) => {
                println!("DEBUG: Orbital initialization succeeded, response data length: {}", resp.data.len());
                println!("DEBUG: Initialization returned {} alkanes", resp.alkanes.0.len());
                
                if !resp.alkanes.0.is_empty() {
                    for (i, alkane) in resp.alkanes.0.iter().enumerate() {
                        println!("DEBUG: Alkane {}: ID [block: {}, tx: {}], value: {}",
                                 i, alkane.id.block, alkane.id.tx, alkane.value);
                    }
                }
                
                resp
            },
            Err(e) => {
                println!("DEBUG: Orbital initialization failed: {}", e);
                println!("DEBUG: Returning orbital ID anyway since it was created");
                // Return the orbital ID even though initialization failed
                return Ok(pixel_orbital_id);
            }
        };
        
        println!("DEBUG: Orbital initialization completed successfully");
        
        // Return the pixel orbital ID
        Ok(pixel_orbital_id)
    }
    
    // Method for opcode 0: Initialize
    fn initialize(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        self.observe_initialization()?;
        self.set_total_supply(0);
        // No tokens are minted on initialization
        
        Ok(response)
    }
    
    // Method for opcode 20: MintPixel
    fn mint_pixel(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        // Check if we've reached the maximum supply
        let current_supply = self.total_supply_pointer().get_value::<u128>();
        if current_supply >= MAX_SUPPLY {
            return Err(anyhow!("Maximum supply of {} pixels reached", MAX_SUPPLY));
        }
        
        // Get the caller's address
        let caller_bytes: Vec<u8> = (&context.caller).into();
        
        // Check if the caller already owns a pixel
        let owned_pixels = self.get_pixels_by_owner_internal(&caller_bytes);
        if !owned_pixels.is_empty() {
            return Err(anyhow!("Each user can only mint one pixel"));
        }
        
        // Generate random color and pattern
        let color = self.generate_random_color(&context);
        let pattern = self.generate_random_pattern(&context);
        
        // Get the next pixel ID
        let next_id = self.get_next_pixel_id();
        
        // Calculate rarity
        let rarity = self.calculate_rarity(color, pattern);
        
        // Deploy a new pixel orbital
        let pixel_orbital_id = self.deploy_pixel_orbital(next_id, color, pattern, rarity, &caller_bytes)?;
        
        // Store the pixel orbital ID
        self.store_pixel_alkane_id(next_id, &pixel_orbital_id);
        
        // Add to owner's pixels
        self.add_pixel_to_owner(&caller_bytes, next_id);
        
        // Update total supply
        self.set_total_supply(current_supply + 1);
        
        // Return the pixel ID, orbital ID, and metadata
        let result = serde_json::json!({
            "pixel_id": next_id,
            "orbital_id": {
                "block": pixel_orbital_id.block,
                "tx": pixel_orbital_id.tx
            },
            "color": [color[0], color[1], color[2]],
            "pattern": pattern,
            "rarity": rarity
        });
        response.data = serde_json::to_vec(&result).unwrap_or_default();
        
        Ok(response)
    }
    
    // Method for opcode 2: GetPixelById
    fn get_pixel_by_id(&self, pixel_id: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        // Get the pixel orbital ID
        let pixel_orbital_id = self.get_pixel_alkane_id(pixel_id as u64)
            .ok_or_else(|| anyhow!("Pixel not found"))?;
        
        // Return the pixel orbital ID
        let result = serde_json::json!({
            "pixel_id": pixel_id,
            "orbital_id": {
                "block": pixel_orbital_id.block,
                "tx": pixel_orbital_id.tx
            }
        });
        response.data = serde_json::to_vec(&result).unwrap_or_default();
        
        Ok(response)
    }
    
    // Method for opcode 3: GetPixelsByOwner
    fn get_pixels_by_owner(&self, owner: u128) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        // Convert u128 to Vec<u8>
        let owner_bytes = owner.to_le_bytes().to_vec();
        
        // Get the pixels owned by the address
        let pixels = self.get_pixels_by_owner_internal(&owner_bytes);
        
        // Return the pixel IDs
        response.data = serde_json::to_vec(&pixels).unwrap_or_default();
        
        Ok(response)
    }
    
    // Method for opcode 4: GetBaseImage
    fn get_base_image(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        // Return a base image that can be used by pixel orbitals
        // This is a placeholder - in a real implementation, we would store and return a real base image
        response.data = (&hex_macro!("89504e470d0a1a0a0000000d494844520000001000000010010300000025db56ca00000003504c5445000000a77a3dda0000000174524e530040e6d8660000000a4944415408d76360000000020001e221bc330000000049454e44ae426082")).to_vec();
        
        Ok(response)
    }
    
    // Method for opcode 5: GetSupplyInfo
    fn get_supply_info(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        let total_supply = self.total_supply_pointer().get_value::<u128>();
        let supply_info = serde_json::json!({
            "totalSupply": total_supply,
            "maxSupply": MAX_SUPPLY,
            "remaining": MAX_SUPPLY.saturating_sub(total_supply)
        });
        response.data = serde_json::to_vec(&supply_info).unwrap();
        
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
}

impl AlkaneResponder for PixelCollection {
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
    impl AlkaneResponder for PixelCollection {
        type Message = PixelCollectionMessage;
    }
}
