use alkanes_runtime::{declare_alkane, message::MessageDispatch, runtime::AlkaneResponder, storage::StoragePointer, token::Token};
use alkanes_support::{parcel::AlkaneTransfer, response::CallResponse, utils::shift_or_err};
use anyhow::{anyhow, Result};
use hex_lit::hex;
use metashrew_support::compat::to_arraybuffer_layout;
use metashrew_support::index_pointer::KeyValuePointer;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Maximum supply of pixels
const MAX_SUPPLY: u128 = 10_000;

#[derive(Default)]
pub struct AlkanePixel(());

impl Token for AlkanePixel {
    fn name(&self) -> String {
        self.name_internal()
    }
    fn symbol(&self) -> String {
        self.symbol_internal()
    }
}

// Implement pixel metadata structure
#[derive(Serialize, Deserialize, Clone)]
pub struct PixelMetadata {
    id: u64,
    color: [u8; 3],  // RGB color
    pattern: u8,     // Pattern type (0-255)
    rarity: u8,      // Rarity score (0-100)
}

impl AlkanePixel {
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
    
    // Get pixel metadata by ID
    pub fn get_pixel(&self, id: u64) -> Option<PixelMetadata> {
        let pointer = StoragePointer::from_keyword(&format!("/pixels/{}", id));
        if pointer.get().len() == 0 {
            None
        } else {
            serde_json::from_slice(&pointer.get()).ok()
        }
    }
    
    // Store pixel metadata
    pub fn store_pixel(&self, metadata: &PixelMetadata) {
        let mut pointer = StoragePointer::from_keyword(&format!("/pixels/{}", metadata.id));
        pointer.set(Arc::new(serde_json::to_vec(metadata).unwrap()));
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
    
    // Add pixel to owner's list
    pub fn add_pixel_to_owner(&self, owner: &[u8], pixel_id: u64) {
        let mut pixels = self.get_pixels_by_owner_internal(owner);
        if !pixels.contains(&pixel_id) {
            pixels.push(pixel_id);
            let mut pointer = StoragePointer::from_keyword(&format!("/owners/{}", hex::encode(owner)));
            pointer.set(Arc::new(serde_json::to_vec(&pixels).unwrap()));
        }
    }
    
    // Remove pixel from owner's list
    pub fn remove_pixel_from_owner(&self, owner: &[u8], pixel_id: u64) {
        let mut pixels = self.get_pixels_by_owner_internal(owner);
        pixels.retain(|&id| id != pixel_id);
        let mut pointer = StoragePointer::from_keyword(&format!("/owners/{}", hex::encode(owner)));
        pointer.set(Arc::new(serde_json::to_vec(&pixels).unwrap()));
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
    
    // Calculate rarity score based on color and pattern
    pub fn calculate_rarity(&self, color: [u8; 3], pattern: u8) -> u8 {
        // Simple rarity calculation algorithm
        // More unique colors and patterns are rarer
        // This is a placeholder implementation
        let color_rarity = (color[0] as u16 + color[1] as u16 + color[2] as u16) % 100;
        let pattern_rarity = (pattern as u16 * 2) % 100;
        ((color_rarity + pattern_rarity) / 2) as u8
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
}
impl AlkanePixel {
    // Method for opcode 0: Initialize
    fn initialize(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        self.observe_initialization()?;
        self.set_total_supply(0);
        // No tokens are minted on initialization
        
        Ok(response)
    }
    
    // Method for opcode 1: Mint
    fn mint(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        // Check if we've reached the maximum supply
        let current_supply = self.total_supply_internal();
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
        
        // Get the next pixel ID (always auto-increment)
        let next_id = current_supply + 1;
        
        // Final safety check to prevent overflow
        if next_id > MAX_SUPPLY {
            return Err(anyhow!("Pixel ID exceeds maximum supply"));
        }
        
        // Calculate rarity
        let rarity = self.calculate_rarity(color, pattern);
        
        // Create pixel metadata
        let metadata = PixelMetadata {
            id: next_id as u64,
            color,
            pattern,
            rarity,
        };
        
        // Store the pixel
        self.store_pixel(&metadata);
        
        // Add to owner's pixels
        let caller_bytes: Vec<u8> = (&context.caller).into();
        self.add_pixel_to_owner(&caller_bytes, next_id as u64);
        
        // Update total supply
        self.set_total_supply(next_id);
        
        // Transfer the NFT to the caller
        response.alkanes.0.push(AlkaneTransfer {
            id: context.myself.clone(),
            value: 1u128,
        });
        
        // Return the pixel ID and metadata
        let metadata_json = serde_json::to_vec(&metadata).unwrap_or_default();
        response.data = metadata_json;
        
        Ok(response)
    }
    
    // Method for opcode 2: Transfer
    fn transfer(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        let mut inputs = context.inputs.clone();
        
        // Skip the opcode
        inputs.remove(0);
        
        // Validate that we have at least one input (the pixel ID)
        if inputs.is_empty() {
            return Err(anyhow!("Missing pixel ID in transfer"));
        }
        
        let pixel_id = shift_or_err(&mut inputs)? as u64;
        
        // Validate pixel ID is not zero
        if pixel_id == 0 {
            return Err(anyhow!("Invalid pixel ID: cannot be zero"));
        }
        
        // Validate pixel ID is within reasonable bounds
        if pixel_id > MAX_SUPPLY as u64 {
            return Err(anyhow!("Pixel ID exceeds maximum supply"));
        }
        
        let to_address = inputs.clone();
        
        // Validate that the recipient address is not empty
        if to_address.is_empty() {
            return Err(anyhow!("Recipient address cannot be empty"));
        }
        
        // Check if the pixel exists
        let _pixel = self.get_pixel(pixel_id).ok_or_else(|| anyhow!("Pixel not found"))?;
        
        // Check if the sender owns the pixel
        let caller_bytes: Vec<u8> = (&context.caller).into();
        let sender_pixels = self.get_pixels_by_owner_internal(&caller_bytes);
        if !sender_pixels.contains(&pixel_id) {
            return Err(anyhow!("Sender does not own this pixel"));
        }
        
        // Remove from sender's pixels
        self.remove_pixel_from_owner(&caller_bytes, pixel_id);
        
        // Convert to_address to bytes
        let to_address_bytes = to_address.iter().flat_map(|&n| n.to_le_bytes()).collect::<Vec<u8>>();
        
        // Add to recipient's pixels
        self.add_pixel_to_owner(&to_address_bytes, pixel_id);
        
        Ok(response)
    }
    
    // Method for opcode 3: GetMetadata
    fn get_metadata(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        let mut inputs = context.inputs.clone();
        
        // Skip the opcode
        inputs.remove(0);
        
        // Validate that we have at least one input (the pixel ID)
        if inputs.is_empty() {
            return Err(anyhow!("Missing pixel ID in get_metadata"));
        }
        
        let pixel_id = shift_or_err(&mut inputs)? as u64;
        
        // Validate pixel ID is not zero
        if pixel_id == 0 {
            return Err(anyhow!("Invalid pixel ID: cannot be zero"));
        }
        
        // Validate pixel ID is within reasonable bounds
        if pixel_id > MAX_SUPPLY as u64 {
            return Err(anyhow!("Pixel ID exceeds maximum supply"));
        }
        
        let pixel = self.get_pixel(pixel_id).ok_or_else(|| anyhow!("Pixel not found"))?;
        
        response.data = serde_json::to_vec(&pixel).unwrap();
        
        Ok(response)
    }
    
    // Method for opcode 4: GetImage
    fn get_image(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        let mut inputs = context.inputs.clone();
        
        // Skip the opcode
        inputs.remove(0);
        
        // Validate that we have at least one input (the pixel ID)
        if inputs.is_empty() {
            return Err(anyhow!("Missing pixel ID in get_image"));
        }
        
        let pixel_id = shift_or_err(&mut inputs)? as u64;
        
        // Validate pixel ID is not zero
        if pixel_id == 0 {
            return Err(anyhow!("Invalid pixel ID: cannot be zero"));
        }
        
        // Validate pixel ID is within reasonable bounds
        if pixel_id > MAX_SUPPLY as u64 {
            return Err(anyhow!("Pixel ID exceeds maximum supply"));
        }
        
        let pixel = self.get_pixel(pixel_id).ok_or_else(|| anyhow!("Pixel not found"))?;
        
        response.data = self.generate_pixel_image(&pixel);
        
        Ok(response)
    }
    
    // Method for opcode 5: GetPixelsByOwner
    fn get_pixels_by_owner(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        let inputs = context.inputs.clone();
        
        // Skip the opcode
        let inputs = if inputs.len() > 1 { &inputs[1..] } else { &[] };
        
        let address: Vec<u8> = if inputs.is_empty() {
            (&context.caller).into()
        } else {
            inputs.iter().flat_map(|&n| n.to_le_bytes()).collect()
        };
        
        let pixels = self.get_pixels_by_owner_internal(&address);
        response.data = serde_json::to_vec(&pixels).unwrap();
        
        Ok(response)
    }
    
    // Method for opcode 6: GetSupplyInfo
    fn get_supply_info(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        let total_supply = self.total_supply_internal();
        let supply_info = serde_json::json!({
            "totalSupply": total_supply,
            "maxSupply": MAX_SUPPLY,
            "remaining": MAX_SUPPLY.saturating_sub(total_supply)
        });
        response.data = serde_json::to_vec(&supply_info).unwrap();
        
        Ok(response)
    }
    
    // Method for opcode 99: Name
    fn name(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        response.data = self.name_internal().into_bytes().to_vec();
        
        Ok(response)
    }
    
    // Renamed the original method to avoid name conflict
    fn name_internal(&self) -> String {
        String::from("AlkanePixel")
    }
    
    // Method for opcode 100: Symbol
    fn symbol(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        response.data = self.symbol_internal().into_bytes().to_vec();
        
        Ok(response)
    }
    
    // Renamed the original method to avoid name conflict
    fn symbol_internal(&self) -> String {
        String::from("APXL")
    }
    
    // Method for opcode 101: TotalSupply
    fn total_supply(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        response.data = (&self.total_supply_internal().to_le_bytes()).to_vec();
        
        Ok(response)
    }
    
    // Renamed the original method to avoid name conflict
    fn total_supply_internal(&self) -> u128 {
        self.total_supply_pointer().get_value::<u128>()
    }
}

impl AlkaneResponder for AlkanePixel {
    fn execute(&self) -> Result<CallResponse> {
        // The opcode extraction and dispatch logic is now handled by the declare_alkane macro
        // This method is still required by the AlkaneResponder trait, but we can just return an error
        // indicating that it should not be called directly
        Err(anyhow!(
            "This method should not be called directly. Use the declare_alkane macro instead."
        ))
    }
}

// Define the message enum for the pixel alkane
#[derive(MessageDispatch)]
enum AlkanePixelMessage {
    #[opcode(0)]
    Initialize,
    
    #[opcode(1)]
    Mint,
    
    #[opcode(2)]
    Transfer,
    
    #[opcode(3)]
    #[returns(Vec<u8>)]
    GetMetadata,
    
    #[opcode(4)]
    #[returns(Vec<u8>)]
    GetImage,
    
    #[opcode(5)]
    #[returns(Vec<u8>)]
    GetPixelsByOwner,
    
    #[opcode(6)]
    #[returns(Vec<u8>)]
    GetSupplyInfo,
    
    #[opcode(99)]
    #[returns(Vec<u8>)]
    Name,
    
    #[opcode(100)]
    #[returns(Vec<u8>)]
    Symbol,
    
    #[opcode(101)]
    #[returns(Vec<u8>)]
    TotalSupply,
}

declare_alkane! {
    impl AlkaneResponder for AlkanePixel {
        type Message = AlkanePixelMessage;
    }
}