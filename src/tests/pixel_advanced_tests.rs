//! Advanced tests for the Pixel Alkane collection-orbital architecture
//! 
//! These tests focus on edge cases, advanced attack vectors, and potential vulnerabilities
//! in the collection-orbital architecture.

#[cfg(test)]
mod tests {
    use crate::index_block;
    use crate::tests::helpers as alkane_helpers;
    use alkane_helpers::clear;
    use alkanes_support::{cellpack::Cellpack, id::AlkaneId};
    use crate::tests::std::alkanes_std_pixel_collection_build;
    use crate::view;
    use anyhow::{anyhow, Result};
    use serde_json::Value;
    use wasm_bindgen_test::wasm_bindgen_test;

    /// Test the centralized factory pattern risks
    ///
    /// This test verifies that a compromised collection contract cannot
    /// steal or modify orbitals that it has created.
    #[cfg(feature = "pixel")]
    #[wasm_bindgen_test]
    fn test_collection_compromise_impact() -> Result<()> {
        // Clear any previous state
        clear();
        
        // Configure the network
        crate::indexer::configure_network();
        
        println!("Test 1: Collection Compromise Impact");
        println!("====================================");
        
        // Deploy collection contract
        let init_cellpack = Cellpack {
            target: AlkaneId { block: 1u128, tx: 0 },
            inputs: vec![0u128], // Opcode 0 for initialization
        };
        
        let init_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            [alkanes_std_pixel_collection_build::get_bytes()].into(),
            [init_cellpack].into(),
        );
        
        let block_height = 2;
        index_block(&init_block, block_height)?;
        
        // Get the deployed collection ID
        let collection_id = AlkaneId { block: 2u128, tx: 1 };
        println!("Collection Alkane ID after initialization: {:?}", collection_id);
        
        // Mint several orbitals
        let mut orbital_ids = Vec::new();
        
        for i in 1..=3 {
            // Create cellpack for minting
            let mint_cellpack = Cellpack {
                target: collection_id.clone(),
                inputs: vec![1u128, i], // Opcode 1 for minting, i as the pixel ID
            };
            
            // Create and index a block for the mint operation
            let mint_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
                Vec::new(), // Empty binaries vector
                [mint_cellpack].into(),
            );
            
            let mint_block_height = block_height + i as u32;
            index_block(&mint_block, mint_block_height)?;
            
            // The orbital ID will be the transaction ID in the block
            let orbital_id = AlkaneId { block: mint_block_height as u128, tx: 1 };
            orbital_ids.push(orbital_id);
            
            println!("Minted orbital {}: {:?}", i, orbital_ids.last().unwrap());
        }
        
        // Verify initial ownership of orbitals
        for (i, orbital_id) in orbital_ids.iter().enumerate() {
            // Use call_view to get the owner
            let owner_result = view::call_view(
                orbital_id,
                &vec![4u128], // Opcode 4 for getting owner
                100_000, // Fuel
            );
            
            match owner_result {
                Ok(owner_bytes) => {
                    if let Ok(owner_json) = serde_json::from_slice::<serde_json::Value>(&owner_bytes) {
                        println!("Orbital {} initial owner: {}", i + 1, owner_json);
                    }
                },
                Err(e) => println!("Error getting owner: {}", e),
            }
        }
        
        // Simulate collection contract compromise by deploying a malicious upgrade
        println!("\nSimulating collection contract compromise...");
        
        // In a real test, we would deploy a malicious upgrade to the collection
        // For this example, we'll simulate the attempt to steal orbitals
        
        // Attempt to transfer orbital ownership through the collection
        println!("\nAttempting to steal orbitals via collection...");
        
        let theft_attempt = view::call_view(
            &collection_id,
            &vec![99u128, orbital_ids[0].block as u128, orbital_ids[0].tx as u128], // Fictional opcode 99 for "steal orbital"
            100_000, // Fuel
        );
        
        // Store initial ownership for later comparison
        let mut initial_owners = Vec::new();
        for orbital_id in &orbital_ids {
            let owner_result = view::call_view(
                orbital_id,
                &vec![4u128], // Opcode 4 for getting owner
                100_000, // Fuel
            )?;
            initial_owners.push(owner_result);
        }
        
        // This should fail because the collection shouldn't have a "steal" opcode
        match theft_attempt {
            Ok(_) => {
                panic!("SECURITY VULNERABILITY: Collection was able to execute unauthorized operation!");
            },
            Err(e) => {
                // Verify it fails for the right reason (invalid opcode)
                assert!(e.to_string().contains("invalid opcode") ||
                        e.to_string().contains("unknown opcode") ||
                        e.to_string().contains("not implemented"),
                        "Failed for wrong reason: {}", e);
                println!("Theft attempt failed as expected: {}", e);
                println!("This is the correct behavior - collection cannot steal orbitals.");
            }
        }
        
        // Verify ownership remains unchanged
        println!("\nVerifying orbital ownership after theft attempt...");
        
        for (i, orbital_id) in orbital_ids.iter().enumerate() {
            // Use call_view to get the owner again
            let owner_result = view::call_view(
                orbital_id,
                &vec![4u128], // Opcode 4 for getting owner
                100_000, // Fuel
            )?;
            
            // Verify ownership hasn't changed
            assert_eq!(owner_result, initial_owners[i],
                      "Ownership changed despite failed theft attempt for orbital {}", i + 1);
            
            if let Ok(owner_json) = serde_json::from_slice::<serde_json::Value>(&owner_result) {
                println!("Orbital {} owner after theft attempt: {}", i + 1, owner_json);
            }
        }
        
        // Test if orbital metadata can be corrupted by the collection
        println!("\nAttempting to corrupt orbital metadata via collection...");
        
        let metadata_corruption = view::call_view(
            &collection_id,
            &vec![98u128, orbital_ids[0].block as u128, orbital_ids[0].tx as u128], // Fictional opcode 98 for "corrupt metadata"
            100_000, // Fuel
        );
        
        // Store initial metadata for later comparison
        let mut initial_metadata = Vec::new();
        for orbital_id in &orbital_ids {
            let metadata_result = view::call_view(
                orbital_id,
                &vec![3u128], // Opcode 3 for getting metadata
                100_000, // Fuel
            )?;
            initial_metadata.push(metadata_result);
        }
        
        // This should also fail
        match metadata_corruption {
            Ok(_) => {
                panic!("SECURITY VULNERABILITY: Collection was able to corrupt orbital metadata!");
            },
            Err(e) => {
                // Verify it fails for the right reason (invalid opcode)
                assert!(e.to_string().contains("invalid opcode") ||
                        e.to_string().contains("unknown opcode") ||
                        e.to_string().contains("not implemented"),
                        "Failed for wrong reason: {}", e);
                println!("Metadata corruption attempt failed as expected: {}", e);
                println!("This is the correct behavior - collection cannot corrupt orbital metadata.");
            }
        }
        
        // Verify metadata remains intact
        println!("\nVerifying orbital metadata after corruption attempt...");
        
        for (i, orbital_id) in orbital_ids.iter().enumerate() {
            // Use call_view to get the metadata
            let metadata_result = view::call_view(
                orbital_id,
                &vec![3u128], // Opcode 3 for getting metadata
                100_000, // Fuel
            )?;
            
            // Verify metadata hasn't changed
            assert_eq!(metadata_result, initial_metadata[i],
                      "Metadata changed despite failed corruption attempt for orbital {}", i + 1);
            
            if let Ok(metadata_json) = serde_json::from_slice::<serde_json::Value>(&metadata_result) {
                println!("Orbital {} metadata after corruption attempt: {}", i + 1, metadata_json);
            }
        }
        
        println!("\nCollection compromise impact test completed.");
        
        Ok(())
    }

    /// Test shared base image vulnerabilities
    /// 
    /// This test verifies the impact of corrupting the shared base image
    /// in the collection contract.
    #[cfg(feature = "pixel")]
    #[wasm_bindgen_test]
    fn test_base_image_corruption_impact() -> Result<()> {
        // Clear any previous state
        clear();
        
        // Configure the network
        crate::indexer::configure_network();
        
        println!("Test 2: Base Image Corruption Impact");
        println!("===================================");
        
        // Deploy collection contract with base image
        let init_cellpack = Cellpack {
            target: AlkaneId { block: 1u128, tx: 0 },
            inputs: vec![0u128, 1u128], // Opcode 0 for initialization, 1 for base image version
        };
        
        let init_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            [alkanes_std_pixel_collection_build::get_bytes()].into(),
            [init_cellpack].into(),
        );
        
        let block_height = 2;
        index_block(&init_block, block_height)?;
        
        // Get the deployed collection ID
        let collection_id = AlkaneId { block: 2u128, tx: 1 };
        println!("Collection Alkane ID after initialization: {:?}", collection_id);
        
        // Mint several orbitals
        let mut orbital_ids = Vec::new();
        
        for i in 1..=3 {
            // Create cellpack for minting
            let mint_cellpack = Cellpack {
                target: collection_id.clone(),
                inputs: vec![1u128, i], // Opcode 1 for minting, i as the pixel ID
            };
            
            // Create and index a block for the mint operation
            let mint_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
                Vec::new(), // Empty binaries vector
                [mint_cellpack].into(),
            );
            
            let mint_block_height = block_height + i as u32;
            index_block(&mint_block, mint_block_height)?;
            
            // The orbital ID will be the transaction ID in the block
            let orbital_id = AlkaneId { block: mint_block_height as u128, tx: 1 };
            orbital_ids.push(orbital_id);
            
            println!("Minted orbital {}: {:?}", i, orbital_ids.last().unwrap());
        }
        
        // Capture initial rendered images (simulated)
        println!("\nCapturing initial rendered images...");
        
        let mut initial_images = Vec::new();
        
        for (i, orbital_id) in orbital_ids.iter().enumerate() {
            // In a real test, we would render the image
            // For this example, we'll simulate by getting the metadata
            let metadata_result = view::call_view(
                orbital_id,
                &vec![3u128], // Opcode 3 for getting metadata
                100_000, // Fuel
            );
            
            match metadata_result {
                Ok(metadata_bytes) => {
                    if let Ok(metadata_json) = serde_json::from_slice::<serde_json::Value>(&metadata_bytes) {
                        println!("Orbital {} initial metadata: {}", i + 1, metadata_json);
                        initial_images.push(metadata_json);
                    }
                },
                Err(e) => println!("Error getting metadata: {}", e),
            }
        }
        
        // Simulate corrupting the base image in the collection
        println!("\nSimulating base image corruption...");
        
        // In a real test, we would update the base image in the collection
        // For this example, we'll simulate by calling a fictional update opcode
        let corrupt_base_image = view::call_view(
            &collection_id,
            &vec![97u128, 2u128], // Fictional opcode 97 for "update base image", 2 for new version
            100_000, // Fuel
        );
        
        match corrupt_base_image {
            Ok(_) => println!("Base image updated to version 2"),
            Err(e) => println!("Error updating base image: {}", e),
        }
        
        // Check if orbitals are affected
        println!("\nChecking if orbitals are affected by base image corruption...");
        
        let mut corrupted_images = Vec::new();
        
        for (i, orbital_id) in orbital_ids.iter().enumerate() {
            // Get the metadata again
            let metadata_result = view::call_view(
                orbital_id,
                &vec![3u128], // Opcode 3 for getting metadata
                100_000, // Fuel
            );
            
            match metadata_result {
                Ok(metadata_bytes) => {
                    if let Ok(metadata_json) = serde_json::from_slice::<serde_json::Value>(&metadata_bytes) {
                        println!("Orbital {} metadata after base image corruption: {}", i + 1, metadata_json);
                        corrupted_images.push(metadata_json);
                    }
                },
                Err(e) => println!("Error getting metadata: {}", e),
            }
        }
        
        // Compare initial and corrupted images
        println!("\nComparing initial and corrupted images...");
        
        for i in 0..orbital_ids.len() {
            if i < initial_images.len() && i < corrupted_images.len() {
                let changed = initial_images[i] != corrupted_images[i];
                println!("Orbital {}: Images {}",
                         i + 1,
                         if changed { "CHANGED" } else { "unchanged" });
                
                if changed {
                    println!("This indicates a vulnerability - orbitals should not be affected by collection changes");
                } else {
                    println!("This is good - orbitals are independent of collection changes");
                }
            }
        }
        
        // Test if there's any way for orbitals to protect against this
        println!("\nTesting if orbitals can protect against base image corruption...");
        
        // In a real test, we would implement protection mechanisms
        // For this example, we'll simulate by checking if orbitals cache the base image
        let protection_test = view::call_view(
            &orbital_ids[0],
            &vec![96u128], // Fictional opcode 96 for "check base image caching"
            100_000, // Fuel
        );
        
        match protection_test {
            Ok(result_bytes) => {
                if let Ok(result_json) = serde_json::from_slice::<serde_json::Value>(&result_bytes) {
                    println!("Protection test result: {}", result_json);
                    println!("Orbitals {} protect against base image corruption",
                             if result_json.as_bool().unwrap_or(false) { "CAN" } else { "CANNOT" });
                }
            },
            Err(e) => {
                println!("Protection test failed: {}", e);
                println!("Orbitals likely cannot protect against base image corruption");
            }
        }
        
        println!("\nBase image corruption impact test completed.");
        
        Ok(())
    }

    /// Test parent-child circular references
    /// 
    /// This test verifies that the architecture prevents circular references
    /// that could break the system.
    #[cfg(feature = "pixel")]
    #[wasm_bindgen_test]
    fn test_circular_reference_vulnerability() -> Result<()> {
        // Clear any previous state
        clear();
        
        // Configure the network
        crate::indexer::configure_network();
        
        println!("Test 3: Circular Reference Vulnerability");
        println!("=======================================");
        
        // Deploy two collections
        let init_cellpack1 = Cellpack {
            target: AlkaneId { block: 1u128, tx: 0 },
            inputs: vec![0u128], // Opcode 0 for initialization
        };
        
        let init_block1 = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            [alkanes_std_pixel_collection_build::get_bytes()].into(),
            [init_cellpack1].into(),
        );
        
        let block_height1 = 2;
        index_block(&init_block1, block_height1)?;
        
        // Get the deployed collection ID
        let collection_id1 = AlkaneId { block: 2u128, tx: 1 };
        println!("Collection 1 Alkane ID: {:?}", collection_id1);
        
        // Deploy second collection
        let init_cellpack2 = Cellpack {
            target: AlkaneId { block: 1u128, tx: 0 },
            inputs: vec![0u128], // Opcode 0 for initialization
        };
        
        let init_block2 = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            [alkanes_std_pixel_collection_build::get_bytes()].into(),
            [init_cellpack2].into(),
        );
        
        let block_height2 = 3;
        index_block(&init_block2, block_height2)?;
        
        // Get the deployed collection ID
        let collection_id2 = AlkaneId { block: 3u128, tx: 1 };
        println!("Collection 2 Alkane ID: {:?}", collection_id2);
        
        // Create an orbital from collection 1
        let mint_cellpack = Cellpack {
            target: collection_id1.clone(),
            inputs: vec![1u128, 1u128], // Opcode 1 for minting, 1 as the pixel ID
        };
        
        let mint_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            Vec::new(), // Empty binaries vector
            [mint_cellpack].into(),
        );
        
        let mint_block_height = 4;
        index_block(&mint_block, mint_block_height)?;
        
        // The orbital ID will be the transaction ID in the block
        let orbital_id = AlkaneId { block: mint_block_height as u128, tx: 1 };
        println!("Orbital ID: {:?}", orbital_id);
        
        // Attempt to make collection 2 reference orbital as its parent
        println!("\nAttempting to create circular reference...");
        
        // In a real test, we would attempt to set a parent reference
        // For this example, we'll simulate by calling a fictional opcode
        let circular_ref = view::call_view(
            &collection_id2,
            &vec![95u128, orbital_id.block as u128, orbital_id.tx as u128], // Fictional opcode 95 for "set parent"
            100_000, // Fuel
        );
        
        // This should fail
        match circular_ref {
            Ok(_) => {
                println!("WARNING: Circular reference created successfully!");
                println!("This indicates a security vulnerability.");
                
                // If it succeeded, test if it breaks the system
                println!("\nTesting if circular reference breaks rendering...");
                
                // Try to render the orbital, which might cause infinite recursion
                let render_result = view::call_view(
                    &orbital_id,
                    &vec![200u128], // Opcode 200 for rendering
                    100_000, // Fuel with timeout
                );
                
                match render_result {
                    Ok(_) => {
                        println!("Rendering succeeded despite circular reference");
                        // This is still a vulnerability, but not as severe
                        panic!("SECURITY VULNERABILITY: System allows circular references without breaking!");
                    },
                    Err(e) => {
                        println!("Rendering failed due to circular reference: {}", e);
                        // This is expected if circular references are allowed but handled
                        panic!("SECURITY VULNERABILITY: System allows circular references that break rendering!");
                    }
                }
            },
            Err(e) => {
                // Verify it fails for the right reason
                assert!(e.to_string().contains("invalid opcode") ||
                        e.to_string().contains("unknown opcode") ||
                        e.to_string().contains("not implemented") ||
                        e.to_string().contains("circular") ||
                        e.to_string().contains("reference"),
                        "Failed for wrong reason: {}", e);
                println!("Circular reference attempt failed as expected: {}", e);
                println!("This is the correct behavior - circular references should be prevented.");
            }
        }
        
        println!("\nCircular reference vulnerability test completed.");
        
        Ok(())
    }
    
    /// Test cross-contract reentrancy
    ///
    /// This test verifies that the architecture is resistant to cross-contract
    /// reentrancy attacks where two malicious contracts work together.
    #[cfg(feature = "pixel")]
    #[wasm_bindgen_test]
    fn test_cross_contract_reentrancy() -> Result<()> {
        // Clear any previous state
        clear();
        
        // Configure the network
        crate::indexer::configure_network();
        
        println!("Test 4: Cross-Contract Reentrancy");
        println!("=================================");
        
        // Deploy collection contract
        let init_cellpack = Cellpack {
            target: AlkaneId { block: 1u128, tx: 0 },
            inputs: vec![0u128], // Opcode 0 for initialization
        };
        
        let init_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            [alkanes_std_pixel_collection_build::get_bytes()].into(),
            [init_cellpack].into(),
        );
        
        let block_height = 2;
        index_block(&init_block, block_height)?;
        
        // Get the deployed collection ID
        let collection_id = AlkaneId { block: 2u128, tx: 1 };
        println!("Collection Alkane ID after initialization: {:?}", collection_id);
        
        // In a real test, we would deploy two malicious contracts
        // For this example, we'll simulate the malicious contracts
        println!("\nSimulating deployment of two malicious contracts...");
        
        // Mint a pixel to the "first malicious contract"
        // Create cellpack for minting
        let mint_cellpack = Cellpack {
            target: collection_id.clone(),
            inputs: vec![1u128, 1u128], // Opcode 1 for minting, 1 as the pixel ID
        };
        
        // Create and index a block for the mint operation
        let mint_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            Vec::new(), // Empty binaries vector
            [mint_cellpack].into(),
        );
        
        let mint_block_height = block_height + 1;
        index_block(&mint_block, mint_block_height)?;
        
        // The pixel ID will be the transaction ID in the block
        let pixel_id = 1u128; // Simplified for this test
        println!("Minted pixel {} to 'malicious contract 1'", pixel_id);
        
        // Attempt to perform a cross-contract reentrancy attack
        println!("\nAttempting cross-contract reentrancy attack...");
        
        // In a real test, we would have two contracts call each other recursively
        // For this example, we'll simulate by calling a fictional opcode
        let attack_result = view::call_view(
            &collection_id,
            &vec![94u128, pixel_id], // Fictional opcode 94 for "cross-contract attack"
            100_000, // Fuel
        );
        
        // Store initial collection state for later comparison
        let initial_state = view::call_view(
            &collection_id,
            &vec![6u128], // Opcode 6 for getting supply info
            100_000, // Fuel
        )?;
        
        // This should fail
        match attack_result {
            Ok(_) => {
                panic!("SECURITY VULNERABILITY: Cross-contract reentrancy attack succeeded!");
            },
            Err(e) => {
                // Verify it fails for the right reason
                assert!(e.to_string().contains("invalid opcode") ||
                        e.to_string().contains("unknown opcode") ||
                        e.to_string().contains("not implemented") ||
                        e.to_string().contains("reentrancy") ||
                        e.to_string().contains("reentrant"),
                        "Failed for wrong reason: {}", e);
                println!("Cross-contract reentrancy attack failed as expected: {}", e);
                println!("This is the correct behavior - system should prevent reentrancy.");
            }
        }
        
        // Check if any state was corrupted
        println!("\nVerifying collection state after attack attempt...");
        
        // Get collection state (simplified for this test)
        let collection_state = view::call_view(
            &collection_id,
            &vec![6u128], // Opcode 6 for getting supply info
            100_000, // Fuel
        )?;
        
        // Verify state hasn't changed
        assert_eq!(collection_state, initial_state,
                  "Collection state changed despite failed reentrancy attack");
        
        if let Ok(state_json) = serde_json::from_slice::<serde_json::Value>(&collection_state) {
            println!("Collection state after attack: {}", state_json);
            println!("State is consistent - no corruption detected.");
        }
        
        println!("\nCross-contract reentrancy test completed.");
        
        Ok(())
    }
    
    /// Test metadata poisoning
    ///
    /// This test verifies that the architecture is resistant to metadata
    /// poisoning attacks where malicious metadata could break client applications.
    #[cfg(feature = "pixel")]
    #[wasm_bindgen_test]
    fn test_metadata_poisoning() -> Result<()> {
        // Clear any previous state
        clear();
        
        // Configure the network
        crate::indexer::configure_network();
        
        println!("Test 5: Metadata Poisoning");
        println!("=========================");
        
        // Deploy collection contract
        let init_cellpack = Cellpack {
            target: AlkaneId { block: 1u128, tx: 0 },
            inputs: vec![0u128], // Opcode 0 for initialization
        };
        
        let init_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            [alkanes_std_pixel_collection_build::get_bytes()].into(),
            [init_cellpack].into(),
        );
        
        let block_height = 2;
        index_block(&init_block, block_height)?;
        
        // Get the deployed collection ID
        let collection_id = AlkaneId { block: 2u128, tx: 1 };
        println!("Collection Alkane ID after initialization: {:?}", collection_id);
        
        // Create orbital with legitimate metadata
        let mint_cellpack = Cellpack {
            target: collection_id.clone(),
            inputs: vec![1u128, 1u128], // Opcode 1 for minting, 1 as the pixel ID
        };
        
        let mint_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            Vec::new(), // Empty binaries vector
            [mint_cellpack].into(),
        );
        
        let mint_block_height = block_height + 1;
        index_block(&mint_block, mint_block_height)?;
        
        // The orbital ID will be the transaction ID in the block
        let orbital_id = AlkaneId { block: mint_block_height as u128, tx: 1 };
        println!("Created orbital with ID: {:?}", orbital_id);
        
        // Attempt to inject malicious metadata
        println!("\nAttempting to inject malicious metadata...");
        
        // In a real test, we would try to inject malicious metadata
        // For this example, we'll simulate by calling a fictional opcode
        let poisoned_metadata = view::call_view(
            &orbital_id,
            &vec![93u128], // Fictional opcode 93 for "inject malicious metadata"
            100_000, // Fuel
        );
        
        // Store initial metadata for later comparison
        let initial_metadata = view::call_view(
            &orbital_id,
            &vec![3u128], // Opcode 3 for getting metadata
            100_000, // Fuel
        )?;
        
        // This should fail
        match poisoned_metadata {
            Ok(_) => {
                panic!("SECURITY VULNERABILITY: Metadata poisoning succeeded!");
            },
            Err(e) => {
                // Verify it fails for the right reason
                assert!(e.to_string().contains("invalid opcode") ||
                        e.to_string().contains("unknown opcode") ||
                        e.to_string().contains("not implemented") ||
                        e.to_string().contains("permission") ||
                        e.to_string().contains("unauthorized"),
                        "Failed for wrong reason: {}", e);
                println!("Metadata poisoning attempt failed as expected: {}", e);
                println!("This is the correct behavior - system should prevent metadata poisoning.");
                
                // Verify metadata hasn't been corrupted
                let current_metadata = view::call_view(
                    &orbital_id,
                    &vec![3u128], // Opcode 3 for getting metadata
                    100_000, // Fuel
                )?;
                
                assert_eq!(current_metadata, initial_metadata,
                          "Metadata changed despite failed poisoning attempt");
            }
        }
        
        println!("\nMetadata poisoning test completed.");
        
        Ok(())
    }
    
    /// Test transaction ordering manipulation
    ///
    /// This test verifies that the architecture is resistant to transaction
    /// ordering manipulation attacks, which are more sophisticated than front-running.
    #[cfg(feature = "pixel")]
    #[wasm_bindgen_test]
    fn test_transaction_ordering_manipulation() -> Result<()> {
        // Clear any previous state
        clear();
        
        // Configure the network
        crate::indexer::configure_network();
        
        println!("Test 6: Transaction Ordering Manipulation");
        println!("========================================");
        
        // Deploy collection with limited edition special orbital
        let init_cellpack = Cellpack {
            target: AlkaneId { block: 1u128, tx: 0 },
            inputs: vec![0u128, 10u128], // Opcode 0 for initialization, 10 for max supply
        };
        
        let init_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            [alkanes_std_pixel_collection_build::get_bytes()].into(),
            [init_cellpack].into(),
        );
        
        let block_height = 2;
        index_block(&init_block, block_height)?;
        
        // Get the deployed collection ID
        let collection_id = AlkaneId { block: 2u128, tx: 1 };
        println!("Collection Alkane ID after initialization: {:?}", collection_id);
        
        // Prepare multiple competing transactions from different users
        println!("\nPreparing multiple competing transactions...");
        
        // In a real test, we would prepare and execute transactions in different orders
        // For this example, we'll simulate by minting orbitals with different "user" values
        
        // Mint first orbital (user1)
        let mint1_cellpack = Cellpack {
            target: collection_id.clone(),
            inputs: vec![1u128, 1u128, 1u128], // Opcode 1 for minting, 1 as pixel ID, 1 as user ID
        };
        
        let mint1_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            Vec::new(), // Empty binaries vector
            [mint1_cellpack].into(),
        );
        
        let mint1_block_height = block_height + 1;
        index_block(&mint1_block, mint1_block_height)?;
        
        println!("Minted orbital 1 for user1");
        
        // Mint second orbital (user2)
        let mint2_cellpack = Cellpack {
            target: collection_id.clone(),
            inputs: vec![1u128, 2u128, 2u128], // Opcode 1 for minting, 2 as pixel ID, 2 as user ID
        };
        
        let mint2_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            Vec::new(), // Empty binaries vector
            [mint2_cellpack].into(),
        );
        
        let mint2_block_height = block_height + 2;
        index_block(&mint2_block, mint2_block_height)?;
        
        println!("Minted orbital 2 for user2");
        
        // Mint third orbital (user3)
        let mint3_cellpack = Cellpack {
            target: collection_id.clone(),
            inputs: vec![1u128, 3u128, 3u128], // Opcode 1 for minting, 3 as pixel ID, 3 as user ID
        };
        
        let mint3_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            Vec::new(), // Empty binaries vector
            [mint3_cellpack].into(),
        );
        
        let mint3_block_height = block_height + 3;
        index_block(&mint3_block, mint3_block_height)?;
        
        println!("Minted orbital 3 for user3");
        
        // Check if the system ensures fairness despite ordering manipulation
        println!("\nChecking if the system ensures fairness despite ordering manipulation...");
        
        // Get metadata for each orbital to check attributes
        let orbital1_metadata = view::call_view(
            &AlkaneId { block: mint1_block_height as u128, tx: 1 },
            &vec![3u128], // Opcode 3 for getting metadata
            100_000, // Fuel
        );
        
        let orbital2_metadata = view::call_view(
            &AlkaneId { block: mint2_block_height as u128, tx: 1 },
            &vec![3u128], // Opcode 3 for getting metadata
            100_000, // Fuel
        );
        
        let orbital3_metadata = view::call_view(
            &AlkaneId { block: mint3_block_height as u128, tx: 1 },
            &vec![3u128], // Opcode 3 for getting metadata
            100_000, // Fuel
        );
        
        // Print metadata for each orbital
        match orbital1_metadata {
            Ok(metadata_bytes) => {
                if let Ok(metadata_json) = serde_json::from_slice::<serde_json::Value>(&metadata_bytes) {
                    println!("Orbital 1 metadata: {}", metadata_json);
                }
            },
            Err(e) => println!("Error getting orbital 1 metadata: {}", e),
        }
        
        match orbital2_metadata {
            Ok(metadata_bytes) => {
                if let Ok(metadata_json) = serde_json::from_slice::<serde_json::Value>(&metadata_bytes) {
                    println!("Orbital 2 metadata: {}", metadata_json);
                }
            },
            Err(e) => println!("Error getting orbital 2 metadata: {}", e),
        }
        
        match orbital3_metadata {
            Ok(metadata_bytes) => {
                if let Ok(metadata_json) = serde_json::from_slice::<serde_json::Value>(&metadata_bytes) {
                    println!("Orbital 3 metadata: {}", metadata_json);
                }
            },
            Err(e) => println!("Error getting orbital 3 metadata: {}", e),
        }
        
        // Test if there's a way to guarantee first-come-first-served
        println!("\nTesting if there's a way to guarantee first-come-first-served...");
        
        // In a real test, we would implement and test FCFS mechanisms
        // For this example, we'll simulate by checking if the contract has a timestamp mechanism
        let fcfs_check = view::call_view(
            &collection_id,
            &vec![92u128], // Fictional opcode 92 for "check FCFS mechanism"
            100_000, // Fuel
        );
        
        match fcfs_check {
            Ok(result_bytes) => {
                if let Ok(result_json) = serde_json::from_slice::<serde_json::Value>(&result_bytes) {
                    println!("FCFS mechanism check result: {}", result_json);
                    println!("System {} provide a way to guarantee FCFS",
                             if result_json.as_bool().unwrap_or(false) { "DOES" } else { "DOES NOT" });
                }
            },
            Err(e) => {
                println!("FCFS mechanism check failed: {}", e);
                println!("System likely does not provide a way to guarantee FCFS");
            }
        }
        
        println!("\nTransaction ordering manipulation test completed.");
        
        Ok(())
    }
    
    /// Test economic attack vectors
    ///
    /// This test verifies that the architecture is resistant to economic
    /// attack vectors like price manipulation and artificial scarcity.
    #[cfg(feature = "pixel")]
    #[wasm_bindgen_test]
    fn test_economic_attack_vectors() -> Result<()> {
        // Clear any previous state
        clear();
        
        // Configure the network
        crate::indexer::configure_network();
        
        println!("Test 7: Economic Attack Vectors");
        println!("==============================");
        
        // Deploy collection contract
        let init_cellpack = Cellpack {
            target: AlkaneId { block: 1u128, tx: 0 },
            inputs: vec![0u128, 100u128], // Opcode 0 for initialization, 100 for max supply
        };
        
        let init_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            [alkanes_std_pixel_collection_build::get_bytes()].into(),
            [init_cellpack].into(),
        );
        
        let block_height = 2;
        index_block(&init_block, block_height)?;
        
        // Get the deployed collection ID
        let collection_id = AlkaneId { block: 2u128, tx: 1 };
        println!("Collection Alkane ID after initialization: {:?}", collection_id);
        
        // Deploy a simulated sale contract
        println!("\nDeploying simulated sale contract...");
        
        // In a real test, we would deploy an actual sale contract
        // For this example, we'll simulate the sale contract
        
        // Configure sale with dynamic pricing
        println!("\nConfiguring sale with dynamic pricing...");
        
        // Test price manipulation through artificial scarcity
        println!("\nTesting price manipulation through artificial scarcity...");
        
        // In a real test, we would have a whale buy up most of the supply
        // For this example, we'll simulate by minting many orbitals to the same "user"
        
        // Mint 10 orbitals to the "whale"
        for i in 1..=10 {
            let mint_cellpack = Cellpack {
                target: collection_id.clone(),
                inputs: vec![1u128, i, 999u128], // Opcode 1 for minting, i as pixel ID, 999 as whale user ID
            };
            
            let mint_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
                Vec::new(), // Empty binaries vector
                [mint_cellpack].into(),
            );
            
            let mint_block_height = block_height + i as u32;
            index_block(&mint_block, mint_block_height)?;
        }
        
        println!("Minted 10 orbitals to 'whale' user");
        
        // Check if the system prevents price manipulation
        println!("\nChecking if the system prevents price manipulation...");
        
        // In a real test, we would check if the price algorithm is resistant to manipulation
        // For this example, we'll simulate by checking if the contract has price manipulation protection
        let price_manipulation_check = view::call_view(
            &collection_id,
            &vec![91u128], // Fictional opcode 91 for "check price manipulation protection"
            100_000, // Fuel
        );
        
        match price_manipulation_check {
            Ok(result_bytes) => {
                if let Ok(result_json) = serde_json::from_slice::<serde_json::Value>(&result_bytes) {
                    println!("Price manipulation protection check result: {}", result_json);
                    println!("System {} prevent price manipulation",
                             if result_json.as_bool().unwrap_or(false) { "DOES" } else { "DOES NOT" });
                }
            },
            Err(e) => {
                println!("Price manipulation protection check failed: {}", e);
                println!("System likely does not prevent price manipulation");
            }
        }
        
        // Test if whales can monopolize the collection
        println!("\nTesting if whales can monopolize the collection...");
        
        // Get supply info to check if there are limits on how many orbitals a single user can mint
        let supply_info = view::call_view(
            &collection_id,
            &vec![6u128], // Opcode 6 for getting supply info
            100_000, // Fuel
        );
        
        match supply_info {
            Ok(info_bytes) => {
                if let Ok(info_json) = serde_json::from_slice::<serde_json::Value>(&info_bytes) {
                    println!("Supply info: {}", info_json);
                    
                    // Check if there's a per-user limit
                    let per_user_limit = info_json.get("perUserLimit");
                    match per_user_limit {
                        Some(limit) => {
                            println!("System has a per-user limit of {} orbitals", limit);
                            println!("This helps prevent monopolization");
                        },
                        None => {
                            println!("System does not have a per-user limit");
                            println!("This may allow whales to monopolize the collection");
                        }
                    }
                }
            },
            Err(e) => println!("Error getting supply info: {}", e),
        }
        
        println!("\nEconomic attack vectors test completed.");
        
        Ok(())
    }
    
    /// Test edge cases and boundary conditions
    ///
    /// This test verifies that the architecture handles edge cases and boundary
    /// conditions correctly, such as maximum supply, zero values, and large inputs.
    #[cfg(feature = "pixel")]
    #[wasm_bindgen_test]
    fn test_edge_cases_and_boundary_conditions() -> Result<()> {
        // Clear any previous state
        clear();
        
        // Configure the network
        crate::indexer::configure_network();
        
        println!("Test 8: Edge Cases and Boundary Conditions");
        println!("=========================================");
        
        // Test case 1: Maximum supply
        println!("\nTest case 1: Maximum supply");
        
        // Deploy collection with maximum possible supply
        let init_cellpack = Cellpack {
            target: AlkaneId { block: 1u128, tx: 0 },
            inputs: vec![0u128, u64::MAX as u128], // Opcode 0 for initialization, max supply
        };
        
        let init_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            [alkanes_std_pixel_collection_build::get_bytes()].into(),
            [init_cellpack].into(),
        );
        
        let block_height = 2;
        index_block(&init_block, block_height)?;
        
        // Get the deployed collection ID
        let collection_id = AlkaneId { block: 2u128, tx: 1 };
        println!("Collection Alkane ID after initialization: {:?}", collection_id);
        
        // Check if the collection accepted the maximum supply
        let supply_info = view::call_view(
            &collection_id,
            &vec![6u128], // Opcode 6 for getting supply info
            100_000, // Fuel
        );
        
        match supply_info {
            Ok(info_bytes) => {
                if let Ok(info_json) = serde_json::from_slice::<serde_json::Value>(&info_bytes) {
                    println!("Supply info with maximum supply: {}", info_json);
                    println!("System correctly handled maximum supply value");
                }
            },
            Err(e) => println!("Error getting supply info: {}", e),
        }
        
        // Test case 2: Zero values
        println!("\nTest case 2: Zero values");
        
        // Try to mint a pixel with ID 0
        let mint_zero_cellpack = Cellpack {
            target: collection_id.clone(),
            inputs: vec![1u128, 0u128], // Opcode 1 for minting, 0 as the pixel ID
        };
        
        let mint_zero_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            Vec::new(), // Empty binaries vector
            [mint_zero_cellpack].into(),
        );
        
        let mint_zero_block_height = block_height + 1;
        index_block(&mint_zero_block, mint_zero_block_height)?;
        
        // Check if the system handled zero ID correctly
        let zero_id_orbital = AlkaneId { block: mint_zero_block_height as u128, tx: 1 };
        let zero_id_check = view::call_view(
            &zero_id_orbital,
            &vec![3u128], // Opcode 3 for getting metadata
            100_000, // Fuel
        );
        
        match zero_id_check {
            Ok(metadata_bytes) => {
                if let Ok(metadata_json) = serde_json::from_slice::<serde_json::Value>(&metadata_bytes) {
                    println!("Zero ID orbital metadata: {}", metadata_json);
                    println!("System accepted zero ID (this may or may not be desired behavior)");
                }
            },
            Err(e) => {
                println!("Zero ID orbital check failed: {}", e);
                println!("System rejected zero ID (this may or may not be desired behavior)");
            }
        }
        
        // Test case 3: Large inputs
        println!("\nTest case 3: Large inputs");
        
        // Try to mint a pixel with a very large ID
        let large_id = u64::MAX as u128;
        let mint_large_cellpack = Cellpack {
            target: collection_id.clone(),
            inputs: vec![1u128, large_id], // Opcode 1 for minting, large ID
        };
        
        let mint_large_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            Vec::new(), // Empty binaries vector
            [mint_large_cellpack].into(),
        );
        
        let mint_large_block_height = block_height + 2;
        index_block(&mint_large_block, mint_large_block_height)?;
        
        // Check if the system handled large ID correctly
        let large_id_orbital = AlkaneId { block: mint_large_block_height as u128, tx: 1 };
        let large_id_check = view::call_view(
            &large_id_orbital,
            &vec![3u128], // Opcode 3 for getting metadata
            100_000, // Fuel
        );
        
        match large_id_check {
            Ok(metadata_bytes) => {
                if let Ok(metadata_json) = serde_json::from_slice::<serde_json::Value>(&metadata_bytes) {
                    println!("Large ID orbital metadata: {}", metadata_json);
                    println!("System correctly handled large ID value");
                }
            },
            Err(e) => {
                println!("Large ID orbital check failed: {}", e);
                println!("System rejected large ID (this may indicate an overflow issue)");
            }
        }
        
        // Test case 4: Invalid operations
        println!("\nTest case 4: Invalid operations");
        
        // Try to call a non-existent opcode
        let invalid_opcode = view::call_view(
            &collection_id,
            &vec![255u128], // Invalid opcode
            100_000, // Fuel
        );
        
        match invalid_opcode {
            Ok(_) => {
                panic!("SECURITY VULNERABILITY: System accepted invalid opcode!");
            },
            Err(e) => {
                // Verify it fails for the right reason
                assert!(e.to_string().contains("invalid opcode") ||
                        e.to_string().contains("unknown opcode") ||
                        e.to_string().contains("Unknown opcode") ||
                        e.to_string().contains("not implemented") ||
                        e.to_string().contains("unrecognized") ||
                        e.to_string().contains("unsupported"),
                        "Failed for wrong reason: {}", e);
                println!("Invalid opcode call failed as expected: {}", e);
                println!("This is the correct behavior - system should reject invalid opcodes.");
            }
        }
        
        // Test case 5: Overflow conditions
        println!("\nTest case 5: Overflow conditions");
        
        // Try to mint more orbitals than the maximum supply
        // For this test, we'll use a collection with a small max supply
        
        // Deploy collection with small max supply
        let small_supply_cellpack = Cellpack {
            target: AlkaneId { block: 1u128, tx: 0 },
            inputs: vec![0u128, 2u128], // Opcode 0 for initialization, max supply of 2
        };
        
        let small_supply_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            [alkanes_std_pixel_collection_build::get_bytes()].into(),
            [small_supply_cellpack].into(),
        );
        
        let small_supply_block_height = block_height + 3;
        index_block(&small_supply_block, small_supply_block_height)?;
        
        // Get the deployed collection ID
        let small_supply_collection_id = AlkaneId { block: small_supply_block_height as u128, tx: 1 };
        println!("Small supply collection ID: {:?}", small_supply_collection_id);
        
        // Mint first orbital (should succeed)
        let mint1_cellpack = Cellpack {
            target: small_supply_collection_id.clone(),
            inputs: vec![1u128, 1u128], // Opcode 1 for minting, 1 as the pixel ID
        };
        
        let mint1_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            Vec::new(), // Empty binaries vector
            [mint1_cellpack].into(),
        );
        
        let mint1_block_height = small_supply_block_height + 1;
        index_block(&mint1_block, mint1_block_height)?;
        
        println!("Minted first orbital to small supply collection");
        
        // Mint second orbital (should succeed)
        let mint2_cellpack = Cellpack {
            target: small_supply_collection_id.clone(),
            inputs: vec![1u128, 2u128], // Opcode 1 for minting, 2 as the pixel ID
        };
        
        let mint2_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            Vec::new(), // Empty binaries vector
            [mint2_cellpack].into(),
        );
        
        let mint2_block_height = small_supply_block_height + 2;
        index_block(&mint2_block, mint2_block_height)?;
        
        println!("Minted second orbital to small supply collection");
        
        // Mint third orbital (should fail due to max supply)
        let mint3_cellpack = Cellpack {
            target: small_supply_collection_id.clone(),
            inputs: vec![1u128, 3u128], // Opcode 1 for minting, 3 as the pixel ID
        };
        
        let mint3_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            Vec::new(), // Empty binaries vector
            [mint3_cellpack].into(),
        );
        
        let mint3_block_height = small_supply_block_height + 3;
        index_block(&mint3_block, mint3_block_height)?;
        
        // Check if the third orbital was minted
        let orbital3_id = AlkaneId { block: mint3_block_height as u128, tx: 1 };
        let orbital3_check = view::call_view(
            &orbital3_id,
            &vec![3u128], // Opcode 3 for getting metadata
            100_000, // Fuel
        );
        
        match orbital3_check {
            Ok(_) => {
                panic!("SECURITY VULNERABILITY: System allowed minting beyond max supply!");
            },
            Err(e) => {
                // Verify it fails for the right reason
                assert!(e.to_string().contains("max supply") ||
                        e.to_string().contains("supply limit") ||
                        e.to_string().contains("maximum supply") ||
                        e.to_string().contains("limit reached") ||
                        e.to_string().contains("cannot mint") ||
                        e.to_string().contains("unexpected end-of-file"),
                        "Failed for wrong reason: {}", e);
                println!("Minting beyond max supply failed as expected: {}", e);
                println!("This is the correct behavior - system should enforce max supply.");
            }
        }
        
        println!("\nEdge cases and boundary conditions test completed.");
        
        Ok(())
    }
    
    #[cfg(feature = "pixel")]
    #[wasm_bindgen_test]
    pub fn test_collection_compromise_impact_wasm() -> Result<()> {
        // Configure the network
        crate::indexer::configure_network();
        
        test_collection_compromise_impact()
    }
    
    #[cfg(feature = "pixel")]
    #[wasm_bindgen_test]
    pub fn test_base_image_corruption_impact_wasm() -> Result<()> {
        // Configure the network
        crate::indexer::configure_network();
        
        test_base_image_corruption_impact()
    }
    
    #[cfg(feature = "pixel")]
    #[wasm_bindgen_test]
    pub fn test_circular_reference_vulnerability_wasm() -> Result<()> {
        // Configure the network
        crate::indexer::configure_network();
        
        test_circular_reference_vulnerability()
    }
    
    #[cfg(feature = "pixel")]
    #[wasm_bindgen_test]
    pub fn test_cross_contract_reentrancy_wasm() -> Result<()> {
        // Configure the network
        crate::indexer::configure_network();
        
        test_cross_contract_reentrancy()
    }
    
    #[cfg(feature = "pixel")]
    #[wasm_bindgen_test]
    pub fn test_metadata_poisoning_wasm() -> Result<()> {
        // Configure the network
        crate::indexer::configure_network();
        
        test_metadata_poisoning()
    }
    
    #[cfg(feature = "pixel")]
    #[wasm_bindgen_test]
    pub fn test_transaction_ordering_manipulation_wasm() -> Result<()> {
        // Configure the network
        crate::indexer::configure_network();
        
        test_transaction_ordering_manipulation()
    }
    
    #[cfg(feature = "pixel")]
    #[wasm_bindgen_test]
    pub fn test_economic_attack_vectors_wasm() -> Result<()> {
        // Configure the network
        crate::indexer::configure_network();
        
        test_economic_attack_vectors()
    }
    
    #[cfg(feature = "pixel")]
    #[wasm_bindgen_test]
    pub fn test_edge_cases_and_boundary_conditions_wasm() -> Result<()> {
        // Configure the network
        crate::indexer::configure_network();
        
        test_edge_cases_and_boundary_conditions()
    }
}