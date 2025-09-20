#[cfg(test)]
mod tests {
    use crate::indexer::index_block;
    use crate::tests::helpers as alkane_helpers;
    use crate::tests::std::alkanes_std_test_build;
    use crate::tests::test_runtime::TestRuntime;
    use alkanes_support::cellpack::Cellpack;
    use alkanes_support::gz::{compress, decompress};
    use alkanes_support::id::AlkaneId;
    use anyhow::Result;
    

    #[test]
    pub fn test_compression() -> Result<()> {
        let buffer = alkanes_std_test_build::get_bytes();
        let compressed = compress(buffer.clone())?;
        assert_eq!(decompress(compressed)?, buffer.clone());
        Ok(())
    }
    #[test]
    fn test_extcall() -> Result<()> {
        crate::indexer::configure_network();
        let mut env = TestRuntime::default();
        let block_height = 0;

        let test_cellpacks = [
            //create alkane
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![1],
            },
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![0],
            },
        ];
        let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            [alkanes_std_test_build::get_bytes()].into(),
            test_cellpacks.into(),
        );

        index_block::<TestRuntime>(&mut env, &test_block, block_height as u32)?;
        Ok(())
    }
    #[test]
    fn test_transaction() -> Result<()> {
        crate::indexer::configure_network();
        let mut env = TestRuntime::default();
        let block_height = 0;

        let test_cellpacks = [
            //create alkane
            Cellpack {
                target: AlkaneId {
                    block: 3,
                    tx: 10001,
                },
                inputs: vec![0, 0],
            },
            Cellpack {
                target: AlkaneId {
                    block: 4,
                    tx: 10001,
                },
                inputs: vec![50],
            },
        ];

        let test_block = alkane_helpers::init_with_multiple_cellpacks_with_tx(
            [alkanes_std_test_build::get_bytes(), vec![]].into(),
            test_cellpacks.into(),
        );
        index_block::<TestRuntime>(&mut env, &test_block, block_height as u32)?;
        Ok(())
    }
    #[test]
    fn test_benchmark() -> Result<()> {
        crate::indexer::configure_network();
        let mut env = TestRuntime::default();
        let block_height = 0;

        let test_cellpacks = [
            //create alkane
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![78],
            },
            /*
            //create second alkane
            Cellpack {
                target: AlkaneId { block: 1, tx: 0 },
                inputs: vec![0],
            },
            //target second alkane to be called with custom opcode
            Cellpack {
                target: AlkaneId { block: 2, tx: 0 },
                inputs: vec![1, 1],
            },
            */
        ];

        let test_block = alkane_helpers::init_with_multiple_cellpacks(
            alkanes_std_test_build::get_bytes(),
            test_cellpacks.to_vec(),
        );

        index_block::<TestRuntime>(&mut env, &test_block, block_height as u32)?;
        Ok(())
    }

    // #[test]
    // async fn test_base_std_functionality() -> Result<()> {
    //     let test_target = AlkaneId { block: 3, tx: 15 };
    //     let test_stored_target = AlkaneId { block: 4, tx: 15 };
    //     let input_cellpack = Cellpack {
    //         target: test_target,
    //         inputs: vec![0u128],
    //     };

    //     let test_block = alkane_helpers::init_test_with_cellpack(input_cellpack);

    //     index_block(&test_block, 840000 as u32)?;
    //     /*
    //     println!("{}", hex::encode(IndexPointer::from_keyword("/alkanes/")
    //             .select(&test_stored_target.into())
    //             .get()
    //             .as_ref()));
    //         */
    //     assert_eq!(
    //         IndexPointer::from_keyword("/alkanes/")
    //             .select(&test_stored_target.into())
    //             .get()
    //             .as_ref()
    //             .clone(),
    //         compress(alkanes_std_test_build::get_bytes())?
    //     );

    //     Ok(())
    // }
}