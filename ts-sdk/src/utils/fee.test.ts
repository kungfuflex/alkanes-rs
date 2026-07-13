import { describe, it, expect } from 'vitest';
import {
  DUST_THRESHOLD,
  INPUT_VSIZE,
  OUTPUT_VSIZE,
  TX_OVERHEAD_VSIZE,
  MIN_RELAY_FEE_RATE,
  computeSendFee,
  estimateSelectionFee,
  estimateTxSize,
  calculateFee,
} from './index';

describe('Fee estimation constants', () => {
  it('DUST_THRESHOLD is 546', () => {
    expect(DUST_THRESHOLD).toBe(546);
  });

  it('MIN_RELAY_FEE_RATE is 1.1 (a hair above bitcoind min-relay so txs relay)', () => {
    expect(MIN_RELAY_FEE_RATE).toBe(1.1);
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
  it('returns 2-output result when change > dust threshold (rate floored to 1.1)', () => {
    // 1 segwit input, sending 50k sats, total 100k sats, feeRate 1 → floored to 1.1
    // vsize2 = 1*68 + 31 + 31 + 10.5 = 140.5 → fee = ceil(140.5*1.1) = 155
    // change = 100000 - 50000 - 155 = 49845 > 546
    const result = computeSendFee({
      inputCount: 1,
      sendAmount: 50000,
      totalInputValue: 100000,
      feeRate: 1,
    });

    expect(result.numOutputs).toBe(2);
    expect(result.fee).toBe(155);
    expect(result.change).toBe(49845);
    expect(result.vsize).toBe(140.5);
    expect(result.effectiveFeeRate).toBe(1.1);
    expect(result.sufficient).toBe(true);
  });

  it('floors sub-1.1 rates at the min-relay rate, never below', () => {
    // feeRate 0.4 → floored to 1.1, identical to the feeRate:1 case above
    const result = computeSendFee({
      inputCount: 1,
      sendAmount: 50000,
      totalInputValue: 100000,
      feeRate: 0.4,
    });
    expect(result.fee).toBe(155);
    expect(result.effectiveFeeRate).toBe(1.1);
    expect(result.sufficient).toBe(true);
  });

  it('honors a rate above the floor (no min-relay effect)', () => {
    // feeRate 5 (> 1.1): vsize2 = 140.5 → fee = ceil(140.5*5) = 703
    const result = computeSendFee({
      inputCount: 1,
      sendAmount: 50000,
      totalInputValue: 100000,
      feeRate: 5,
    });
    expect(result.fee).toBe(703);
    expect(result.effectiveFeeRate).toBe(5);
    expect(result.sufficient).toBe(true);
  });

  it('minFeeRate override can disable the floor', () => {
    // feeRate 1, minFeeRate 0 → charge the raw rate. fee = ceil(140.5*1) = 141
    const result = computeSendFee({
      inputCount: 1,
      sendAmount: 50000,
      totalInputValue: 100000,
      feeRate: 1,
      minFeeRate: 0,
    });
    expect(result.fee).toBe(141);
    expect(result.effectiveFeeRate).toBe(1);
    expect(result.sufficient).toBe(true);
  });

  it('absorbs only the sub-dust remainder into the fee (near-max send stays affordable)', () => {
    // 1 segwit input, sending 99500 of 100000, feeRate 1 → 1.1
    // vsize2 = 140.5 → fee2 = 155; change = 100000-99500-155 = 345 <= 546 (dust)
    // vsize1 = 1*68 + 31 + 10.5 = 109.5 → minFee1 = ceil(109.5*1.1) = 121
    // remainder = 100000-99500 = 500 >= 121 → fee = 500 (conservation), sufficient
    const result = computeSendFee({
      inputCount: 1,
      sendAmount: 99500,
      totalInputValue: 100000,
      feeRate: 1,
    });

    expect(result.numOutputs).toBe(1);
    expect(result.change).toBe(0);
    expect(result.fee).toBe(500); // forced by conservation, NOT ballooned beyond it
    expect(result.vsize).toBe(109.5);
    expect(result.effectiveFeeRate).toBeCloseTo(500 / 109.5, 5);
    expect(result.effectiveFeeRate).toBeGreaterThan(1.1);
    expect(result.sufficient).toBe(true); // affordable — must NOT be blocked
  });

  it('flags sufficient=false when inputs cannot cover the minimum 1-output fee', () => {
    // 1 segwit input, sending 99999 of 100000, feeRate 10
    // vsize1 = 109.5 → minFee1 = ceil(109.5*10) = 1095; remainder = 1 < 1095
    const result = computeSendFee({
      inputCount: 1,
      sendAmount: 99999,
      totalInputValue: 100000,
      feeRate: 10,
    });

    expect(result.numOutputs).toBe(1);
    expect(result.change).toBe(0);
    expect(result.fee).toBe(1095); // the minimum required (exceeds available)
    expect(result.effectiveFeeRate).toBe(10);
    expect(result.sufficient).toBe(false); // caller branches on THIS, not fee>balance
  });

  it('works with taproot inputs', () => {
    // 2 taproot inputs, 50k send, 200k total, 5 sat/vB (> floor)
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
    expect(result.sufficient).toBe(true);
  });

  it('supports mixed recipient and change types (rate floored to 1.1)', () => {
    // 1 segwit input, legacy recipient, taproot change
    // vsize2 = 1*68 + 34 + 43 + 10.5 = 155.5 → fee = ceil(155.5*1.1) = 172
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
    expect(result.fee).toBe(172);
    expect(result.numOutputs).toBe(2);
    expect(result.sufficient).toBe(true);
  });

  it('defaults changeType to inputType when not specified', () => {
    const result = computeSendFee({
      inputCount: 1,
      sendAmount: 50000,
      totalInputValue: 200000,
      feeRate: 1,
      inputType: 'taproot',
      recipientType: 'segwit',
    });
    // vsize2 = 1*57.5 + 31 (segwit recipient) + 43 (taproot change) + 10.5 = 142
    expect(result.vsize).toBe(142);
  });

  it('supports custom dust threshold', () => {
    // feeRate 1 → 1.1 throughout.
    // 1 segwit input, sending 99500, total 100000, dust 1000
    // vsize2 = 140.5, fee2 = 155, change = 345 <= 1000 → absorbed
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

    // High custom threshold forces 1 output where default would give 2.
    const result2 = computeSendFee({
      inputCount: 1,
      sendAmount: 50000,
      totalInputValue: 51200,
      feeRate: 1,
      dustThreshold: 1500,
    });
    // vsize2 = 140.5, fee2 = 155, change = 51200-50000-155 = 1045 <= 1500 → absorbed
    expect(result2.numOutputs).toBe(1);

    // Same scenario, default threshold → 2 outputs.
    const result3 = computeSendFee({
      inputCount: 1,
      sendAmount: 50000,
      totalInputValue: 51200,
      feeRate: 1,
    });
    // change = 51200-50000-155 = 1045 > 546 → 2 outputs
    expect(result3.numOutputs).toBe(2);
    expect(result3.fee).toBe(155);
    expect(result3.change).toBe(1045);
  });

  it('reproduces SendModal behavior for segwit sends (rate 2, above floor)', () => {
    const numInputs = 3;
    const amountSats = 500000;
    const totalInputValue = 600000;
    const feeRateNum = 2; // > 1.1, so the floor does not change anything

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
    expect(result.sufficient).toBe(true);
  });
});

describe('estimateSelectionFee', () => {
  it('applies the same 1.1 min-relay floor', () => {
    // 3 segwit inputs, 2 segwit outputs, feeRate 1 → 1.1
    // vsize = 3*68 + 2*31 + 10.5 = 276.5 → fee = ceil(276.5*1.1) = 305
    expect(estimateSelectionFee(3, 1)).toBe(305);
  });

  it('honors a rate above the floor (taproot)', () => {
    // 2 taproot inputs, 2 segwit outputs, 5 sat/vB
    // vsize = 2*57.5 + 2*31 + 10.5 = 187.5 → fee = ceil(187.5*5) = 938
    expect(estimateSelectionFee(2, 5, 'taproot')).toBe(938);
  });

  it('supports custom output count and type (floored)', () => {
    // 1 segwit input, 1 taproot output, feeRate 1 → 1.1
    // vsize = 1*68 + 1*43 + 10.5 = 121.5 → fee = ceil(121.5*1.1) = 134
    expect(estimateSelectionFee(1, 1, 'segwit', 1, 'taproot')).toBe(134);
  });

  it('returns consistent results with computeSendFee vsize (2-output case)', () => {
    const fee = estimateSelectionFee(2, 3, 'segwit', 2, 'segwit');
    // vsize = 2*68 + 2*31 + 10.5 = 208.5 → fee = ceil(208.5*3) = 626 (rate 3 > floor)
    expect(fee).toBe(626);

    const result = computeSendFee({
      inputCount: 2,
      sendAmount: 100000,
      totalInputValue: 200000,
      feeRate: 3,
    });
    expect(result.fee).toBe(fee);
  });
});

describe('backward compatibility', () => {
  it('estimateTxSize still works unchanged', () => {
    expect(estimateTxSize(1, 2, 'segwit')).toBe(10 + 1 * 68 + 2 * 34);
    expect(estimateTxSize(2, 1, 'taproot')).toBe(10 + 2 * 57.5 + 1 * 34);
    expect(estimateTxSize(1, 1, 'legacy')).toBe(10 + 1 * 148 + 1 * 34);
  });

  it('calculateFee still works unchanged (raw size*rate primitive, no floor)', () => {
    expect(calculateFee(200, 5)).toBe(1000);
    expect(calculateFee(140.5, 1)).toBe(141);
  });
});
