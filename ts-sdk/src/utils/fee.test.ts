import { describe, it, expect } from 'vitest';
import {
  DUST_THRESHOLD,
  INPUT_VSIZE,
  OUTPUT_VSIZE,
  TX_OVERHEAD_VSIZE,
  computeSendFee,
  estimateSelectionFee,
  estimateTxSize,
  calculateFee,
} from './index';

describe('Fee estimation constants', () => {
  it('DUST_THRESHOLD is 546', () => {
    expect(DUST_THRESHOLD).toBe(546);
  });

  it('INPUT_VSIZE has correct values', () => {
    expect(INPUT_VSIZE.legacy).toBe(148);
    expect(INPUT_VSIZE.segwit).toBe(68);
    expect(INPUT_VSIZE.taproot).toBe(57.5);
  });

  it('OUTPUT_VSIZE has correct values', () => {
    expect(OUTPUT_VSIZE.legacy).toBe(34);
    expect(OUTPUT_VSIZE.segwit).toBe(31);
    expect(OUTPUT_VSIZE.taproot).toBe(43);
  });

  it('TX_OVERHEAD_VSIZE is 10.5', () => {
    expect(TX_OVERHEAD_VSIZE).toBe(10.5);
  });
});

describe('computeSendFee', () => {
  it('returns 2-output result when change > dust threshold', () => {
    // 1 segwit input, sending 50k sats, total 100k sats, 1 sat/vB
    // vsize2 = 1*68 + 31 + 31 + 10.5 = 140.5 → fee = 141
    // change = 100000 - 50000 - 141 = 49859 > 546
    const result = computeSendFee({
      inputCount: 1,
      sendAmount: 50000,
      totalInputValue: 100000,
      feeRate: 1,
    });

    expect(result.numOutputs).toBe(2);
    expect(result.fee).toBe(141);
    expect(result.change).toBe(49859);
    expect(result.vsize).toBe(140.5);
    expect(result.effectiveFeeRate).toBe(1);
  });

  it('absorbs dust into fee when change <= dust threshold', () => {
    // 1 segwit input, sending 99500 sats, total 100000 sats, 1 sat/vB
    // vsize2 = 1*68 + 31 + 31 + 10.5 = 140.5 → fee2 = 141
    // change = 100000 - 99500 - 141 = 359 <= 546 (dust)
    // vsize1 = 1*68 + 31 + 10.5 = 109.5 → minFee1 = 110
    // remainder = 100000 - 99500 = 500 >= 110
    // fee = 500 (all remainder), effectiveRate = 500/109.5 ≈ 4.566
    const result = computeSendFee({
      inputCount: 1,
      sendAmount: 99500,
      totalInputValue: 100000,
      feeRate: 1,
    });

    expect(result.numOutputs).toBe(1);
    expect(result.change).toBe(0);
    expect(result.fee).toBe(500);
    expect(result.vsize).toBe(109.5);
    expect(result.effectiveFeeRate).toBeCloseTo(500 / 109.5, 5);
    expect(result.effectiveFeeRate).toBeGreaterThan(1); // Higher than requested
  });

  it('returns insufficient fee when remainder < 1-output minimum fee', () => {
    // 1 segwit input, sending 99999 sats, total 100000 sats, 10 sat/vB
    // vsize1 = 1*68 + 31 + 10.5 = 109.5 → minFee1 = ceil(109.5*10) = 1095
    // remainder = 100000 - 99999 = 1 < 1095
    const result = computeSendFee({
      inputCount: 1,
      sendAmount: 99999,
      totalInputValue: 100000,
      feeRate: 10,
    });

    expect(result.numOutputs).toBe(1);
    expect(result.change).toBe(0);
    expect(result.fee).toBe(1095); // Minimum fee even though insufficient
    expect(result.effectiveFeeRate).toBe(10);
  });

  it('works with taproot inputs', () => {
    // 2 taproot inputs, 50k send, 200k total, 5 sat/vB
    // vsize2 = 2*57.5 + 31 + 43 + 10.5 = 199.5 → fee2 = ceil(199.5*5) = 998
    // change = 200000 - 50000 - 998 = 149002 > 546
    const result = computeSendFee({
      inputCount: 2,
      sendAmount: 50000,
      totalInputValue: 200000,
      feeRate: 5,
      inputType: 'taproot',
      changeType: 'taproot',
    });

    expect(result.numOutputs).toBe(2);
    expect(result.fee).toBe(998);
    expect(result.change).toBe(149002);
    expect(result.vsize).toBe(199.5);
  });

  it('supports mixed recipient and change types', () => {
    // 1 segwit input, legacy recipient, taproot change
    // vsize2 = 1*68 + 34 (legacy recipient) + 43 (taproot change) + 10.5 = 155.5
    const result = computeSendFee({
      inputCount: 1,
      sendAmount: 50000,
      totalInputValue: 200000,
      feeRate: 1,
      inputType: 'segwit',
      recipientType: 'legacy',
      changeType: 'taproot',
    });

    expect(result.vsize).toBe(155.5);
    expect(result.fee).toBe(156);
    expect(result.numOutputs).toBe(2);
  });

  it('defaults changeType to inputType when not specified', () => {
    // taproot input, no changeType specified → change output uses taproot size
    const result = computeSendFee({
      inputCount: 1,
      sendAmount: 50000,
      totalInputValue: 200000,
      feeRate: 1,
      inputType: 'taproot',
      recipientType: 'segwit',
      // changeType not specified → should use 'taproot' (same as inputType)
    });

    // vsize2 = 1*57.5 + 31 (segwit recipient) + 43 (taproot change) + 10.5 = 142
    expect(result.vsize).toBe(142);
  });

  it('supports custom dust threshold', () => {
    // With custom dust threshold of 1000
    // 1 segwit input, sending 99500, total 100000, 1 sat/vB
    // vsize2 = 140.5, fee2 = 141, change = 100000 - 99500 - 141 = 359
    // 359 <= 1000 (custom threshold) → absorbed
    const result = computeSendFee({
      inputCount: 1,
      sendAmount: 99500,
      totalInputValue: 100000,
      feeRate: 1,
      dustThreshold: 1000,
    });

    expect(result.numOutputs).toBe(1);
    expect(result.change).toBe(0);
    expect(result.fee).toBe(500); // remainder = 100000 - 99500

    // Without custom threshold, change=359 would be below default 546 too, but let's
    // test a case where default would give 2 outputs but custom gives 1
    const result2 = computeSendFee({
      inputCount: 1,
      sendAmount: 50000,
      totalInputValue: 51200,
      feeRate: 1,
      dustThreshold: 1500, // High custom threshold
    });

    // vsize2 = 140.5, fee2 = 141, change = 51200 - 50000 - 141 = 1059 <= 1500 → absorbed
    expect(result2.numOutputs).toBe(1);

    // Same scenario with default threshold
    const result3 = computeSendFee({
      inputCount: 1,
      sendAmount: 50000,
      totalInputValue: 51200,
      feeRate: 1,
    });

    // change = 51200 - 50000 - 141 = 1059 > 546 → 2 outputs
    expect(result3.numOutputs).toBe(2);
    expect(result3.change).toBe(1059);
  });

  it('reproduces SendModal behavior for segwit sends', () => {
    // This test reproduces the exact formula that was in SendModal's computeAccurateFee.
    // SendModal hardcoded: numInputs * 68 + 2 * 31 + 10.5 for 2-output, * 1 * 31 for 1-output.
    // computeSendFee with default segwit types should produce identical results.

    const numInputs = 3;
    const amountSats = 500000;
    const totalInputValue = 600000;
    const feeRateNum = 2;

    // Old SendModal formula (2-output):
    const oldVsize2 = numInputs * 68 + 2 * 31 + 10.5; // 276.5
    const oldFee2 = Math.ceil(oldVsize2 * feeRateNum);  // 553
    const oldChange = totalInputValue - amountSats - oldFee2; // 99447

    const result = computeSendFee({
      inputCount: numInputs,
      sendAmount: amountSats,
      totalInputValue,
      feeRate: feeRateNum,
    });

    expect(result.vsize).toBe(oldVsize2);
    expect(result.fee).toBe(oldFee2);
    expect(result.change).toBe(oldChange);
    expect(result.numOutputs).toBe(2);
  });
});

describe('estimateSelectionFee', () => {
  it('matches expected formula for segwit defaults', () => {
    // 3 segwit inputs, 2 segwit outputs, 1 sat/vB
    // vsize = 3*68 + 2*31 + 10.5 = 276.5 → fee = 277
    expect(estimateSelectionFee(3, 1)).toBe(277);
  });

  it('works with taproot inputs', () => {
    // 2 taproot inputs, 2 segwit outputs, 5 sat/vB
    // vsize = 2*57.5 + 2*31 + 10.5 = 187.5 → fee = ceil(187.5*5) = 938
    expect(estimateSelectionFee(2, 5, 'taproot')).toBe(938);
  });

  it('supports custom output count and type', () => {
    // 1 segwit input, 1 taproot output, 1 sat/vB
    // vsize = 1*68 + 1*43 + 10.5 = 121.5 → fee = 122
    expect(estimateSelectionFee(1, 1, 'segwit', 1, 'taproot')).toBe(122);
  });

  it('returns consistent results with computeSendFee vsize (2-output case)', () => {
    // estimateSelectionFee should produce the same fee as computeSendFee's 2-output vsize
    // when using same types and 2 outputs
    const fee = estimateSelectionFee(2, 3, 'segwit', 2, 'segwit');
    // vsize = 2*68 + 2*31 + 10.5 = 208.5 → fee = ceil(208.5*3) = 626
    expect(fee).toBe(626);

    const result = computeSendFee({
      inputCount: 2,
      sendAmount: 100000,
      totalInputValue: 200000,
      feeRate: 3,
    });
    // In 2-output case, computeSendFee fee should match
    expect(result.fee).toBe(fee);
  });
});

describe('backward compatibility', () => {
  it('estimateTxSize still works unchanged', () => {
    // Original formula: baseSize(10) + inputCount*inputSize + outputCount*34
    expect(estimateTxSize(1, 2, 'segwit')).toBe(10 + 1 * 68 + 2 * 34);
    expect(estimateTxSize(2, 1, 'taproot')).toBe(10 + 2 * 57.5 + 1 * 34);
    expect(estimateTxSize(1, 1, 'legacy')).toBe(10 + 1 * 148 + 1 * 34);
  });

  it('calculateFee still works unchanged', () => {
    expect(calculateFee(200, 5)).toBe(1000);
    expect(calculateFee(140.5, 1)).toBe(141);
  });
});
