/// Helper function to deploy the pixel orbital binary
fn deploy_pixel_orbital_binary(pixel_orbital_id: &AlkaneId, block_height: u32) -> Result<()> {
    println!("Deploying pixel orbital binary for [block: {}, tx: {}]",
             pixel_orbital_id.block, pixel_orbital_id.tx);
    
    // Check if the binary already exists
    let binary = metashrew::index_pointer::IndexPointer::from_keyword("/alkanes/")
        .select(&pixel_orbital_id.into())
        .get();
    
    if binary.len() > 0 {
        println!("Pixel orbital binary already exists");
        return Ok(());
    }
    
    // Create a block with the pixel orbital binary
    let deploy_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
        [alkanes_std_pixel_orbital_build::get_bytes()].into(),
        Vec::new(), // No cellpacks needed for deployment
    );
    
    // Index the deployment block
    println!("Indexing deployment block at height {}", block_height);
    match index_block(&deploy_block, block_height) {
        Ok(_) => println!("Deployment block indexed successfully"),
        Err(e) => {
            println!("Failed to index deployment block: {}", e);
            return Err(e);
        }
    }
    
    // Store the binary in the storage
    let mut pointer = metashrew::index_pointer::IndexPointer::from_keyword("/alkanes/")
        .select(&pixel_orbital_id.into());
    
    pointer.set(std::sync::Arc::new(alkanes_support::gz::compress(alkanes_std_pixel_orbital_build::get_bytes())?));
    
    println!("Pixel orbital binary deployed successfully");
    
    Ok(())
}
