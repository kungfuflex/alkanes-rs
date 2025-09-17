use crate::tables::RuneTable;
use metashrew_support::environment::RuntimeEnvironment;
use anyhow::{anyhow, Result};
use bitcoin::{OutPoint, Txid};
use metashrew_support::index_pointer::AtomicPointer;


use metashrew_support::index_pointer::KeyValuePointer;
use std::{
    cmp::min,
    collections::{BTreeMap, BTreeSet},
    ops::Deref,
    sync::Arc,
};

use ordinals::Edict;

use protorune_support::balance_sheet::{BalanceSheet, BalanceSheetOperations, ProtoruneRuneId};

#[derive(Clone, Debug)]
pub struct Protoburn<E: RuntimeEnvironment> {
    pub tag: Option<u128>,
    pub pointer: Option<u32>,
    pub from: Option<Vec<u32>>,
	pub _phantom: std::marker::PhantomData<E>
}

impl<E: RuntimeEnvironment + Clone + Default> Protoburn<E> {
    pub fn process(
        &mut self,
        atomic: &mut AtomicPointer<E>,
        balance_sheet: BalanceSheet<AtomicPointer<E>>,
        proto_balances_by_output: &mut BTreeMap<u32, BalanceSheet<AtomicPointer<E>>>,
        outpoint: OutPoint,
    ) -> Result<()> {
        let table = RuneTable::<E>::for_protocol(self.tag.ok_or(anyhow!("no tag found"))?);
        for (rune, _balance) in balance_sheet.balances().into_iter() {
            let runeid: Arc<Vec<u8>> = (*rune).into();
            let name = RuneTable::<E>::new().RUNE_ID_TO_ETCHING.select(&runeid).get();
            atomic
                .derive(&table.RUNE_ID_TO_ETCHING.select(&runeid))
                .set(name.clone());
            atomic
                .derive(&table.ETCHING_TO_RUNE_ID.select(&name))
                .set(runeid);
            atomic
                .derive(&table.SPACERS.select(&name))
                .set(RuneTable::<E>::new().SPACERS.select(&name).get());
            atomic
                .derive(&table.DIVISIBILITY.select(&name))
                .set(RuneTable::<E>::new().DIVISIBILITY.select(&name).get());
            atomic
                .derive(&table.SYMBOL.select(&name))
                .set(RuneTable::<E>::new().SYMBOL.select(&name).get());
            atomic.derive(&table.ETCHINGS).append(name);
        }
        if !proto_balances_by_output.contains_key(&outpoint.vout) {
            proto_balances_by_output.insert(outpoint.vout, BalanceSheet::default());
        }
        balance_sheet.pipe(
            proto_balances_by_output
                .get_mut(&outpoint.vout)
                .ok_or("")
                .map_err(|_| anyhow!("outpoint vout not in proto_balances_by_output"))?,
        )?;
        Ok(())
    }
}

pub trait Protoburns<E: RuntimeEnvironment + Clone, T>: Deref<Target = [T]> {
    fn construct_burncycle(&self) -> Result<BurnCycle> {
        let length = u32::try_from(self.len())?;
        Ok(BurnCycle::new(length))
    }
    fn process(
        &mut self,
        atomic: &mut AtomicPointer<E>,
        runestone_edicts: Vec<Edict>,
        runestone_output_index: u32,
        balances_by_output: &BTreeMap<u32, BalanceSheet<AtomicPointer<E>>>,
        proto_balances_by_output: &mut BTreeMap<u32, BalanceSheet<AtomicPointer<E>>>,
        default_output: u32,
        txid: Txid,
    ) -> Result<()>;
}

impl<E: RuntimeEnvironment + Clone + Default> Protoburns<E, Protoburn<E>> for Vec<Protoburn<E>> {
    fn process(
        &mut self,
        atomic: &mut AtomicPointer<E>,
        runestone_edicts: Vec<Edict>,
        runestone_output_index: u32,
        balances_by_output: &BTreeMap<u32, BalanceSheet<AtomicPointer<E>>>,
        proto_balances_by_output: &mut BTreeMap<u32, BalanceSheet<AtomicPointer<E>>>,
        default_output: u32,
        txid: Txid,
    ) -> Result<()> {
        let mut runestone_balance_sheet: BalanceSheet<AtomicPointer<E>> = BalanceSheet::new();
        if balances_by_output.contains_key(&runestone_output_index) {
            let sheet = balances_by_output
                .get(&runestone_output_index)
                .ok_or(anyhow!("cannot find balance sheet"))?;
            sheet.pipe(&mut runestone_balance_sheet)?;
        }
        let mut burn_cycles = self.construct_burncycle()?;
        let mut pull_set = BTreeMap::<u32, bool>::new();
        let mut burn_sheets = self
            .iter_mut()
            .map(|_a| BalanceSheet::new())
            .collect::<Vec<BalanceSheet<AtomicPointer<E>>>>();

        // from field in Protoburn is provided, which means the burn doesn't cycle through the inputs, just pulls the inputs from the "from" field and burns those
        for (i, burn) in self.iter_mut().enumerate() {
            if let Some(_from) = burn.clone().from {
                let from = _from.into_iter().collect::<BTreeSet<u32>>();
                for j in from {
                    pull_set.insert(j, true);
                    if runestone_edicts
                        .get(j as usize)
                        .ok_or(anyhow!("Index {} is out of bounds", j))?
                        .output
                        == runestone_output_index
                    {
                        let rune = runestone_edicts[j as usize].id;
                        let remaining = runestone_balance_sheet.get(&rune.into());
                        let to_apply = min(remaining, runestone_edicts[j as usize].amount);
                        if to_apply == 0 {
                            continue;
                        }
                        runestone_balance_sheet.decrease(&rune.clone().into(), to_apply);
                        burn_sheets[i].increase(&rune.into(), to_apply)?;
                    }
                }
            }
        }

        // go through remaining edicts and cycle through protoburns
        for (i, edict) in runestone_edicts.into_iter().enumerate() {
            if pull_set.contains_key(&(i as u32)) {
                continue;
            };
            if edict.output == runestone_output_index {
                let rune = edict.id;
                let cycle = burn_cycles.peek(&(rune.into()))?;
                let remaining = runestone_balance_sheet.get(&(rune.into()));
                let to_apply = min(remaining, edict.amount);
                if to_apply == 0 {
                    continue;
                };
                burn_cycles.next(&(rune.into()))?;
                runestone_balance_sheet.decrease(&rune.clone().into(), to_apply);
                burn_sheets[cycle as usize].increase(&rune.into(), to_apply)?;
            }
        }

        // the default output of the runestone (all leftover runes, or the mint runes go to this output)
        // equals the runestone OP_RETURN. This is a valid protoburn
        if runestone_output_index == default_output {
            for rune in runestone_balance_sheet.clone().balances().keys() {
                let cycle = burn_cycles.peek(rune)?;
                let to_apply = runestone_balance_sheet.get(rune);
                if to_apply == 0 {
                    continue;
                };
                burn_cycles.next(rune)?;
                runestone_balance_sheet.decrease(rune, to_apply);
                burn_sheets[cycle as usize].increase(rune, to_apply)?;
            }
        }

        for (i, burn) in self.iter_mut().enumerate() {
            let sheet = burn_sheets[i].clone();
            burn.process(
                atomic,
                sheet,
                proto_balances_by_output,
                OutPoint::new(txid, burn.pointer.ok_or(anyhow!("no vout on protoburn"))?),
            )?;
        }
        Ok(())
    }
}

pub struct BurnCycle {
    max: u32,
    cycles: BTreeMap<ProtoruneRuneId, i32>,
}

impl BurnCycle {
    pub fn new(max: u32) -> Self {
        BurnCycle {
            max,
            cycles: BTreeMap::<ProtoruneRuneId, i32>::new(),
        }
    }
    pub fn next(&mut self, rune: &ProtoruneRuneId) -> Result<i32> {
        if !self.cycles.contains_key(rune) {
            self.cycles.insert(rune.clone(), 0);
        }
        let cycles = self.cycles.clone();
        let cycle = cycles.get(rune).ok_or(anyhow!("no value found"))?;
        self.cycles
            .insert(rune.clone(), (cycle.clone() + 1) % (self.max as i32));
        Ok(cycle.clone())
    }
    pub fn peek(&mut self, rune: &ProtoruneRuneId) -> Result<i32> {
        if !self.cycles.contains_key(rune) {
            self.cycles.insert(rune.clone(), 0);
        }
        Ok(self
            .cycles
            .get(rune)
            .ok_or(anyhow!("value not found"))?
            .clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::hashes::Hash;
    use bitcoin::OutPoint;
    use metashrew_support::index_pointer::AtomicPointer;
    use ordinals::RuneId;
    use protorune_support::balance_sheet::ProtoruneRuneId;
    use std::collections::BTreeMap;
	use metashrew_test::TestRuntime;

	type TestPointer = AtomicPointer<TestRuntime>;

    #[test]
#[ignore]
    fn test_protoburn_process_success() {
        // Create a dummy Protoburn instance
        let mut protoburn = Protoburn {
            tag: Some(13),
            pointer: Some(0),
            from: None,
			_phantom: std::marker::PhantomData::<TestRuntime>,
        };

        // Create mock objects for dependencies
        let mut atomic = TestPointer::default();
        let balance_sheet: BalanceSheet<TestPointer> = BalanceSheet::from_pairs(
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![100 as u128, 200 as u128],
        );
        let mut proto_balances_by_output: BTreeMap<u32, BalanceSheet<TestPointer>> = BTreeMap::new();
        let outpoint = OutPoint {
            txid: Hash::from_byte_array([
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                23, 1, 1, 1, 1, 1, 1, 1, 1,
            ]),
            vout: 0,
        };

        // Call the process function
        let result = protoburn.process(
            &mut atomic,
            balance_sheet.clone(),
            &mut proto_balances_by_output,
            outpoint,
        );

        // Assert that the function executed without errors
        assert!(result.is_ok());

        // Verify that proto_balances_by_output contains the expected data
        assert!(proto_balances_by_output.contains_key(&outpoint.vout));

        assert_eq!(proto_balances_by_output[&outpoint.vout], balance_sheet);
    }

    #[test]
#[ignore]
    fn test_protoburn_process_no_tag() {
        // Create a Protoburn instance without a tag
        let mut protoburn = Protoburn {
            tag: None,
            pointer: Some(0),
            from: None,
			_phantom: std::marker::PhantomData::<TestRuntime>,
        };

        // Create mock objects for dependencies
        let mut atomic = TestPointer::default();
        let balance_sheet: BalanceSheet<TestPointer> = BalanceSheet::new();
        let mut proto_balances_by_output: BTreeMap<u32, BalanceSheet<TestPointer>> = BTreeMap::new();
        let outpoint = OutPoint {
            txid: Hash::from_byte_array([
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                23, 1, 1, 1, 1, 1, 1, 1, 1,
            ]),
            vout: 0,
        };

        // Call the process function
        let result = protoburn.process(
            &mut atomic,
            balance_sheet,
            &mut proto_balances_by_output,
            outpoint,
        );

        // Assert that the function returns an error due to missing tag
        assert!(result.is_err());
    }

    #[test]
#[ignore]
    fn test_protoburns_no_op() {
        // Create a Vec of Protoburns
        let mut protoburns: Vec<Protoburn<TestRuntime>> = vec![
            Protoburn {
                tag: Some(1),
                pointer: Some(0),
                from: None,
				_phantom: std::marker::PhantomData::<TestRuntime>,
            },
            Protoburn {
                tag: Some(2),
                pointer: Some(1),
                from: None,
				_phantom: std::marker::PhantomData::<TestRuntime>,
            },
        ];

        // Create mock objects for dependencies
        let mut atomic = TestPointer::default();
        let balances_by_output: BTreeMap<u32, BalanceSheet<TestPointer>> = BTreeMap::new();
        let mut proto_balances_by_output: BTreeMap<u32, BalanceSheet<TestPointer>> = BTreeMap::new();
        let txid = Hash::from_byte_array([
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            1, 1, 1, 1, 1, 1, 1, 1,
        ]);
        let edicts = Vec::new();

        // Call the process function
        let result = protoburns.process(
            &mut atomic,
            edicts,
            1,
            &balances_by_output,
            &mut proto_balances_by_output,
            0,
            txid,
        );

        // Assert that the function executed successfully
        assert!(result.is_ok());
        assert_eq!(proto_balances_by_output[&0], BalanceSheet::new());
        assert_eq!(proto_balances_by_output[&1], BalanceSheet::new());
    }

    #[test]
#[ignore]
    fn test_protoburns_default_goes_to_first_protoburn() {
        // Create a Vec of Protoburns
        let mut protoburns: Vec<Protoburn<TestRuntime>> = vec![
            Protoburn {
                tag: Some(1),
                pointer: Some(0),
                from: None,
				_phantom: std::marker::PhantomData::<TestRuntime>,
            },
            Protoburn {
                tag: Some(2),
                pointer: Some(1),
                from: None,
				_phantom: std::marker::PhantomData::<TestRuntime>,
            },
        ];

        // Create mock objects for dependencies
        let mut atomic = TestPointer::default();
        let balance_sheet_0: BalanceSheet<TestPointer> = BalanceSheet::from_pairs(
            // runestone output index is set as 1, so this should be ignored by protoburns since this is just a transfer of runes directly to an output instead of to the OP_RETURN
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![100 as u128, 200 as u128],
        );
        let balance_sheet_1: BalanceSheet<TestPointer> = BalanceSheet::from_pairs(
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![300 as u128, 400 as u128],
        );
        let balances_by_output: BTreeMap<u32, BalanceSheet<TestPointer>> =
            BTreeMap::from([(0, balance_sheet_0.clone()), (1, balance_sheet_1.clone())]);
        let mut proto_balances_by_output: BTreeMap<u32, BalanceSheet<TestPointer>> = BTreeMap::new();
        let txid = Hash::from_byte_array([
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            1, 1, 1, 1, 1, 1, 1, 1,
        ]);
        let edicts = Vec::new();

        // Call the process function
        let result = protoburns.process(
            &mut atomic,
            edicts,
            1,
            &balances_by_output,
            &mut proto_balances_by_output,
            1,
            txid,
        );

        // Assert that the function executed successfully
        assert!(result.is_ok());
        assert_eq!(proto_balances_by_output[&0], balance_sheet_1.clone());
        assert_eq!(proto_balances_by_output[&1], BalanceSheet::new());
    }

    #[test]
#[ignore]
    fn test_protoburns_edicts_cycle() {
        // Create a Vec of Protoburns
        let mut protoburns: Vec<Protoburn<TestRuntime>> = vec![
            Protoburn {
                tag: Some(13),
                pointer: Some(0),
                from: None,
				_phantom: std::marker::PhantomData::<TestRuntime>,
            },
            Protoburn {
                tag: Some(13),
                pointer: Some(1),
                from: None,
				_phantom: std::marker::PhantomData::<TestRuntime>,
            },
        ];

        let runestone_output_index = 1;

        // Create mock objects for dependencies
        let mut atomic = TestPointer::default();
        let balance_sheet_0: BalanceSheet<TestPointer> = BalanceSheet::from_pairs(
            // runestone output index is set as 1, so this should be ignored by protoburns since this is just a transfer of runes directly to an output instead of to the OP_RETURN
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![100 as u128, 200 as u128],
        );
        let balance_sheet_1: BalanceSheet<TestPointer> = BalanceSheet::from_pairs(
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![300 as u128, 400 as u128],
        );
        let balances_by_output: BTreeMap<u32, BalanceSheet<TestPointer>> = BTreeMap::from([
            (0, balance_sheet_0.clone()),
            (runestone_output_index, balance_sheet_1.clone()),
        ]);
        let mut proto_balances_by_output: BTreeMap<u32, BalanceSheet<TestPointer>> = BTreeMap::new();
        let txid = Hash::from_byte_array([
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            1, 1, 1, 1, 1, 1, 1, 1,
        ]);
        let edicts = vec![Edict {
            id: RuneId { block: 1, tx: 1 },
            amount: 10,
            output: runestone_output_index,
        }];

        // Call the process function
        let result = protoburns.process(
            &mut atomic,
            edicts,
            runestone_output_index,
            &balances_by_output,
            &mut proto_balances_by_output,
            runestone_output_index,
            txid,
        );

        // Assert that the function executed successfully
        assert!(result.is_ok());

        let expected_sheet_0 = BalanceSheet::from_pairs(
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![10 as u128, 400 as u128],
        );
        let expected_sheet_1 =
            BalanceSheet::from_pairs(vec![ProtoruneRuneId { block: 1, tx: 1 }], vec![290 as u128]);
        assert_eq!(proto_balances_by_output[&0], expected_sheet_0);
        assert_eq!(proto_balances_by_output[&1], expected_sheet_1);
    }

    #[test]
#[ignore]
    fn test_protoburns_edicts_cycle_two_runes() {
        // Create a Vec of Protoburns
        let mut protoburns: Vec<Protoburn<TestRuntime>> = vec![
            Protoburn {
                tag: Some(13),
                pointer: Some(0),
                from: None,
				_phantom: std::marker::PhantomData::<TestRuntime>,
            },
            Protoburn {
                tag: Some(13),
                pointer: Some(1),
                from: None,
				_phantom: std::marker::PhantomData::<TestRuntime>,
            },
        ];

        let runestone_output_index = 1;

        // Create mock objects for dependencies
        let mut atomic = TestPointer::default();
        let balance_sheet_0: BalanceSheet<TestPointer> = BalanceSheet::from_pairs(
            // runestone output index is set as 1, so this should be ignored by protoburns since this is just a transfer of runes directly to an output instead of to the OP_RETURN
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![100 as u128, 200 as u128],
        );
        let balance_sheet_1: BalanceSheet<TestPointer> = BalanceSheet::from_pairs(
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![300 as u128, 400 as u128],
        );
        let balances_by_output: BTreeMap<u32, BalanceSheet<TestPointer>> = BTreeMap::from([
            (0, balance_sheet_0.clone()),
            (runestone_output_index, balance_sheet_1.clone()),
        ]);
        let mut proto_balances_by_output: BTreeMap<u32, BalanceSheet<TestPointer>> = BTreeMap::new();
        let txid = Hash::from_byte_array([
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            1, 1, 1, 1, 1, 1, 1, 1,
        ]);
        let edicts = vec![
            Edict {
                id: RuneId { block: 1, tx: 1 },
                amount: 10,
                output: runestone_output_index,
            },
            Edict {
                id: RuneId { block: 2, tx: 2 },
                amount: 10,
                output: runestone_output_index,
            },
        ];

        // Call the process function
        let result = protoburns.process(
            &mut atomic,
            edicts,
            runestone_output_index,
            &balances_by_output,
            &mut proto_balances_by_output,
            runestone_output_index,
            txid,
        );

        // Assert that the function executed successfully
        assert!(result.is_ok());
        let expected_sheet_0 = BalanceSheet::from_pairs(
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![10 as u128, 10 as u128],
        );
        let expected_sheet_1 = BalanceSheet::from_pairs(
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![290 as u128, 390 as u128],
        );
        assert_eq!(proto_balances_by_output[&0], expected_sheet_0);
        assert_eq!(proto_balances_by_output[&1], expected_sheet_1);
    }

    #[test]
#[ignore]
    fn test_protoburns_edicts_cycle_loopback() {
        // Create a Vec of Protoburns
        let mut protoburns: Vec<Protoburn<TestRuntime>> = vec![
            Protoburn {
                tag: Some(13),
                pointer: Some(0),
                from: None,
				_phantom: std::marker::PhantomData::<TestRuntime>,
            },
            Protoburn {
                tag: Some(13),
                pointer: Some(1),
                from: None,
				_phantom: std::marker::PhantomData::<TestRuntime>,
            },
        ];

        let runestone_output_index = 1;

        // Create mock objects for dependencies
        let mut atomic = TestPointer::default();
        let balance_sheet_0: BalanceSheet<TestPointer> = BalanceSheet::from_pairs(
            // runestone output index is set as 1, so this should be ignored by protoburns since this is just a transfer of runes directly to an output instead of to the OP_RETURN
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![100 as u128, 200 as u128],
        );
        let balance_sheet_1: BalanceSheet<TestPointer> = BalanceSheet::from_pairs(
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![300 as u128, 400 as u128],
        );
        let balances_by_output: BTreeMap<u32, BalanceSheet<TestPointer>> = BTreeMap::from([
            (0, balance_sheet_0.clone()),
            (runestone_output_index, balance_sheet_1.clone()),
        ]);
        let mut proto_balances_by_output: BTreeMap<u32, BalanceSheet<TestPointer>> = BTreeMap::new();
        let txid = Hash::from_byte_array([
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            1, 1, 1, 1, 1, 1, 1, 1,
        ]);
        let edicts = vec![
            Edict {
                id: RuneId { block: 1, tx: 1 },
                amount: 10,
                output: runestone_output_index,
            },
            Edict {
                id: RuneId { block: 2, tx: 2 },
                amount: 10,
                output: runestone_output_index,
            },
            Edict {
                id: RuneId { block: 1, tx: 1 },
                amount: 20,
                output: runestone_output_index,
            },
        ];

        // Call the process function
        let result = protoburns.process(
            &mut atomic,
            edicts,
            runestone_output_index,
            &balances_by_output,
            &mut proto_balances_by_output,
            runestone_output_index,
            txid,
        );

        // Assert that the function executed successfully
        assert!(result.is_ok());
        let expected_sheet_0 = BalanceSheet::from_pairs(
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![280 as u128, 10 as u128],
        );
        let expected_sheet_1 = BalanceSheet::from_pairs(
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![20 as u128, 390 as u128],
        );
        assert_eq!(proto_balances_by_output[&0], expected_sheet_0);
        assert_eq!(proto_balances_by_output[&1], expected_sheet_1);
    }

    #[test]
#[ignore]
    fn test_protoburns_edicts_from_invalid() {
        // Create a Vec of Protoburns
        let mut protoburns: Vec<Protoburn<TestRuntime>> = vec![
            Protoburn {
                tag: Some(13),
                pointer: Some(0),
                from: Some(vec![5]),
				_phantom: std::marker::PhantomData::<TestRuntime>,
            },
            Protoburn {
                tag: Some(13),
                pointer: Some(1),
                from: None,
				_phantom: std::marker::PhantomData::<TestRuntime>,
            },
        ];

        let runestone_output_index = 1;

        // Create mock objects for dependencies
        let mut atomic = TestPointer::default();
        let balance_sheet_0: BalanceSheet<TestPointer> = BalanceSheet::from_pairs(
            // runestone output index is set as 1, so this should be ignored by protoburns since this is just a transfer of runes directly to an output instead of to the OP_RETURN
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![100 as u128, 200 as u128],
        );
        let balance_sheet_1: BalanceSheet<TestPointer> = BalanceSheet::from_pairs(
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![300 as u128, 400 as u128],
        );
        let balances_by_output: BTreeMap<u32, BalanceSheet<TestPointer>> = BTreeMap::from([
            (0, balance_sheet_0.clone()),
            (runestone_output_index, balance_sheet_1.clone()),
        ]);
        let mut proto_balances_by_output: BTreeMap<u32, BalanceSheet<TestPointer>> = BTreeMap::new();
        let txid = Hash::from_byte_array([
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            1, 1, 1, 1, 1, 1, 1, 1,
        ]);
        let edicts = vec![
            Edict {
                id: RuneId { block: 1, tx: 1 },
                amount: 10,
                output: runestone_output_index,
            },
            Edict {
                id: RuneId { block: 2, tx: 2 },
                amount: 10,
                output: runestone_output_index,
            },
            Edict {
                id: RuneId { block: 1, tx: 1 },
                amount: 20,
                output: runestone_output_index,
            },
        ];

        // Call the process function
        let result = protoburns.process(
            &mut atomic,
            edicts,
            runestone_output_index,
            &balances_by_output,
            &mut proto_balances_by_output,
            runestone_output_index,
            txid,
        );

        assert!(result.is_err());
    }

    #[test]
#[ignore]
    fn test_protoburns_edicts_from() {
        // Create a Vec of Protoburns
        let mut protoburns: Vec<Protoburn<TestRuntime>> = vec![
            Protoburn {
                tag: Some(13),
                pointer: Some(0),
                from: Some(vec![0, 1]),
				_phantom: std::marker::PhantomData::<TestRuntime>,
            },
            Protoburn {
                tag: Some(13),
                pointer: Some(1),
                from: Some(vec![2]),
				_phantom: std::marker::PhantomData::<TestRuntime>,
            },
        ];

        let runestone_output_index = 1;

        // Create mock objects for dependencies
        let mut atomic = TestPointer::default();
        let balance_sheet_0: BalanceSheet<TestPointer> = BalanceSheet::from_pairs(
            // runestone output index is set as 1, so this should be ignored by protoburns since this is just a transfer of runes directly to an output instead of to the OP_RETURN
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![100 as u128, 200 as u128],
        );
        let balance_sheet_1: BalanceSheet<TestPointer> = BalanceSheet::from_pairs(
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![300 as u128, 400 as u128],
        );
        let balances_by_output: BTreeMap<u32, BalanceSheet<TestPointer>> = BTreeMap::from([
            (0, balance_sheet_0.clone()),
            (runestone_output_index, balance_sheet_1.clone()),
        ]);
        let mut proto_balances_by_output: BTreeMap<u32, BalanceSheet<TestPointer>> = BTreeMap::new();
        let txid = Hash::from_byte_array([
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            1, 1, 1, 1, 1, 1, 1, 1,
        ]);
        let edicts = vec![
            Edict {
                id: RuneId { block: 1, tx: 1 },
                amount: 10,
                output: runestone_output_index,
            },
            Edict {
                id: RuneId { block: 2, tx: 2 },
                amount: 10,
                output: runestone_output_index,
            },
            Edict {
                id: RuneId { block: 1, tx: 1 },
                amount: 20,
                output: runestone_output_index,
            },
        ];

        // Call the process function
        let result = protoburns.process(
            &mut atomic,
            edicts,
            runestone_output_index,
            &balances_by_output,
            &mut proto_balances_by_output,
            runestone_output_index,
            txid,
        );

        assert!(result.is_ok());
        let expected_sheet_0 = BalanceSheet::from_pairs(
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![280 as u128, 400 as u128],
        );
        let expected_sheet_1 =
            BalanceSheet::from_pairs(vec![ProtoruneRuneId { block: 1, tx: 1 }], vec![20 as u128]);
        assert_eq!(proto_balances_by_output[&0], expected_sheet_0);
        assert_eq!(proto_balances_by_output[&1], expected_sheet_1);
    }

    #[test]
#[ignore]
    fn test_protoburns_edicts_from_cycle() {
        // Create a Vec of Protoburns
        let mut protoburns: Vec<Protoburn<TestRuntime>> = vec![
            Protoburn {
                tag: Some(13),
                pointer: Some(0),
                from: Some(vec![0]),
				_phantom: std::marker::PhantomData::<TestRuntime>,
            },
            Protoburn {
                tag: Some(13),
                pointer: Some(1),
                from: Some(vec![2]),
				_phantom: std::marker::PhantomData::<TestRuntime>,
            },
        ];

        let runestone_output_index = 1;

        // Create mock objects for dependencies
        let mut atomic = TestPointer::default();
        let balance_sheet_0: BalanceSheet<TestPointer> = BalanceSheet::from_pairs(
            // runestone output index is set as 1, so this should be ignored by protoburns since this is just a transfer of runes directly to an output instead of to the OP_RETURN
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![100 as u128, 200 as u128],
        );
        let balance_sheet_1: BalanceSheet<TestPointer> = BalanceSheet::from_pairs(
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![300 as u128, 400 as u128],
        );
        let balances_by_output: BTreeMap<u32, BalanceSheet<TestPointer>> = BTreeMap::from([
            (0, balance_sheet_0.clone()),
            (runestone_output_index, balance_sheet_1.clone()),
        ]);
        let mut proto_balances_by_output: BTreeMap<u32, BalanceSheet<TestPointer>> = BTreeMap::new();
        let txid = Hash::from_byte_array([
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
            1, 1, 1, 1, 1, 1, 1, 1,
        ]);
        let edicts = vec![
            Edict {
                id: RuneId { block: 1, tx: 1 },
                amount: 10,
                output: runestone_output_index,
            },
            Edict {
                id: RuneId { block: 2, tx: 2 },
                amount: 10,
                output: runestone_output_index,
            },
            Edict {
                id: RuneId { block: 1, tx: 1 },
                amount: 20,
                output: runestone_output_index,
            },
        ];

        // Call the process function
        let result = protoburns.process(
            &mut atomic,
            edicts,
            runestone_output_index,
            &balances_by_output,
            &mut proto_balances_by_output,
            runestone_output_index,
            txid,
        );

        assert!(result.is_ok());
        let expected_sheet_0 = BalanceSheet::from_pairs(
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![280 as u128, 10 as u128],
        );
        let expected_sheet_1 = BalanceSheet::from_pairs(
            vec![
                ProtoruneRuneId { block: 1, tx: 1 },
                ProtoruneRuneId { block: 2, tx: 2 },
            ],
            vec![20 as u128, 390 as u128],
        );
        assert_eq!(proto_balances_by_output[&0], expected_sheet_0);
        assert_eq!(proto_balances_by_output[&1], expected_sheet_1);
    }
}
