use alkanes_runtime::declare_alkane;
use alkanes_runtime::{runtime::AlkaneResponder, storage::StoragePointer, token::Token};
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
        String::from("AlkanePixel")
    }
    fn symbol(&self) -> String {
        String::from("APXL")
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
    
    pub fn total_supply(&self) -> u128 {
        self.total_supply_pointer().get_value::<u128>()
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
    pub fn get_pixels_by_owner(&self, owner: &[u8]) -> Vec<u64> {
        let pointer = StoragePointer::from_keyword(&format!("/owners/{}", hex::encode(owner)));
        if pointer.get().len() == 0 {
            vec![]
        } else {
            serde_json::from_slice(&pointer.get()).unwrap_or_default()
        }
    }
    
    // Add pixel to owner's list
    pub fn add_pixel_to_owner(&self, owner: &[u8], pixel_id: u64) {
        let mut pixels = self.get_pixels_by_owner(owner);
        if !pixels.contains(&pixel_id) {
            pixels.push(pixel_id);
            let mut pointer = StoragePointer::from_keyword(&format!("/owners/{}", hex::encode(owner)));
            pointer.set(Arc::new(serde_json::to_vec(&pixels).unwrap()));
        }
    }
    
    // Remove pixel from owner's list
    pub fn remove_pixel_from_owner(&self, owner: &[u8], pixel_id: u64) {
        let mut pixels = self.get_pixels_by_owner(owner);
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
    
    // Generate random color
    pub fn generate_random_color(&self, context: &alkanes_runtime::runtime::Context) -> [u8; 3] {
        // Use transaction hash as a source of randomness
        let tx_hash = context.transaction.compute_txid();
        let tx_hash_bytes = tx_hash.as_byte_array();
        
        // Use different parts of the hash for different color components
        let r = tx_hash_bytes[0];
        let g = tx_hash_bytes[1];
        let b = tx_hash_bytes[2];
        
        [r, g, b]
    }
    
    // Generate random pattern
    pub fn generate_random_pattern(&self, context: &alkanes_runtime::runtime::Context) -> u8 {
        // Use transaction hash as a source of randomness
        let tx_hash = context.transaction.compute_txid();
        let tx_hash_bytes = tx_hash.as_byte_array();
        
        // Use a different part of the hash for the pattern
        tx_hash_bytes[3]
    }
}

impl AlkaneResponder for AlkanePixel {
    fn execute(&self) -> Result<CallResponse> {
        let context = self.context()?;
        let mut inputs = context.inputs.clone();
        let mut response = CallResponse::forward(&context.incoming_alkanes);
        
        match shift_or_err(&mut inputs)? {
            // Initialize the contract
            0 => {
                self.observe_initialization()?;
                self.set_total_supply(0);
                // No tokens are minted on initialization
            }
            
            // Mint a new pixel
            1 => {
                // Check if we've reached the maximum supply
                let current_supply = self.total_supply();
                if current_supply >= MAX_SUPPLY {
                    return Err(anyhow!("Maximum supply of {} pixels reached", MAX_SUPPLY));
                }
                
                // Generate random color and pattern
                let color = self.generate_random_color(&context);
                let pattern = self.generate_random_pattern(&context);
                
                // Get the next pixel ID
                let next_id = current_supply + 1;
                
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
            }
            
            // Transfer a pixel
            2 => {
                let pixel_id = shift_or_err(&mut inputs)? as u64;
                let to_address = inputs.clone();
                
                // Check if the pixel exists
                let pixel = self.get_pixel(pixel_id).ok_or_else(|| anyhow!("Pixel not found"))?;
                
                // Check if the sender owns the pixel
                let caller_bytes: Vec<u8> = (&context.caller).into();
                let sender_pixels = self.get_pixels_by_owner(&caller_bytes);
                if !sender_pixels.contains(&pixel_id) {
                    return Err(anyhow!("Sender does not own this pixel"));
                }
                
                // Remove from sender's pixels
                self.remove_pixel_from_owner(&caller_bytes, pixel_id);
                
                // Convert to_address to bytes
                let to_address_bytes = to_address.iter().flat_map(|&n| n.to_le_bytes()).collect::<Vec<u8>>();
                
                // Add to recipient's pixels
                self.add_pixel_to_owner(&to_address_bytes, pixel_id);
            }
            
            // Get pixel metadata
            3 => {
                let pixel_id = shift_or_err(&mut inputs)? as u64;
                let pixel = self.get_pixel(pixel_id).ok_or_else(|| anyhow!("Pixel not found"))?;
                
                response.data = serde_json::to_vec(&pixel).unwrap();
            }
            
            // Get pixel image
            4 => {
                let pixel_id = shift_or_err(&mut inputs)? as u64;
                let pixel = self.get_pixel(pixel_id).ok_or_else(|| anyhow!("Pixel not found"))?;
                
                response.data = self.generate_pixel_image(&pixel);
            }
            
            // Get pixels owned by an address
            5 => {
                let address: Vec<u8> = if inputs.is_empty() {
                    (&context.caller).into()
                } else {
                    inputs.iter().flat_map(|&n| n.to_le_bytes()).collect()
                };
                
                let pixels = self.get_pixels_by_owner(&address);
                response.data = serde_json::to_vec(&pixels).unwrap();
            }
            
            // Get total supply and max supply
            6 => {
                let total_supply = self.total_supply();
                let supply_info = serde_json::json!({
                    "totalSupply": total_supply,
                    "maxSupply": MAX_SUPPLY,
                    "remaining": MAX_SUPPLY.saturating_sub(total_supply)
                });
                response.data = serde_json::to_vec(&supply_info).unwrap();
            }
            
            // Standard token methods
            99 => {
                response.data = self.name().into_bytes().to_vec();
            }
            100 => {
                response.data = self.symbol().into_bytes().to_vec();
            }
            101 => {
                response.data = (&self.total_supply().to_le_bytes()).to_vec();
            }
            
            _ => return Err(anyhow!("unrecognized opcode")),
        }
        
        Ok(response)
    }
}

declare_alkane! {AlkanePixel}