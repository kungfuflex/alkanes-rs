export interface AlkanesApiClientOptions {
  baseUrl: string;
  timeout?: number;
}

export class AlkanesApiClient {
  private baseUrl: string;
  private timeout: number;

  constructor(options: AlkanesApiClientOptions) {
    this.baseUrl = options.baseUrl;
    this.timeout = options.timeout || 30000;
  }

  private async post<T>(endpoint: string, body: any): Promise<T> {
    const url = `${this.baseUrl}/api/v1/${endpoint}`;
    
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), this.timeout);

    try {
      const response = await fetch(url, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(body),
        signal: controller.signal,
      });

      if (!response.ok) {
        throw new Error(`API request failed: ${response.status}`);
      }

      return await response.json();
    } finally {
      clearTimeout(timeoutId);
    }
  }

  // Balance endpoints
  async getAddressBalances(
    address: string,
    includeOutpoints: boolean = false
  ): Promise<AddressBalancesResponse> {
    return this.post('get-address-balances', {
      address,
      include_outpoints: includeOutpoints,
    });
  }

  async getOutpointBalances(outpoint: string): Promise<OutpointBalancesResponse> {
    return this.post('get-outpoint-balances', { outpoint });
  }

  async getHolders(
    alkane: string,
    page: number = 1,
    limit: number = 100
  ): Promise<HoldersResponse> {
    return this.post('get-holders', { alkane, page, limit });
  }

  async getHoldersCount(alkane: string): Promise<HolderCountResponse> {
    return this.post('get-holders-count', { alkane });
  }

  async getAddressOutpoints(address: string): Promise<AddressOutpointsResponse> {
    return this.post('get-address-outpoints', { address });
  }

  // Storage endpoints
  async getKeys(
    alkane: string,
    options?: { prefix?: string; limit?: number }
  ): Promise<GetKeysResponse> {
    return this.post('get-keys', {
      alkane,
      prefix: options?.prefix,
      limit: options?.limit || 100,
    });
  }

  // AMM endpoints
  async getTrades(
    pool: string,
    options?: {
      startTime?: number;
      endTime?: number;
      limit?: number;
    }
  ): Promise<GetTradesResponse> {
    return this.post('get-trades', {
      pool,
      start_time: options?.startTime,
      end_time: options?.endTime,
      limit: options?.limit || 100,
    });
  }

  async getCandles(
    pool: string,
    interval: string,
    options?: {
      startTime?: number;
      endTime?: number;
      limit?: number;
    }
  ): Promise<GetCandlesResponse> {
    return this.post('get-candles', {
      pool,
      interval,
      start_time: options?.startTime,
      end_time: options?.endTime,
      limit: options?.limit || 500,
    });
  }

  async getReserves(pool: string): Promise<GetReservesResponse> {
    return this.post('get-reserves', { pool });
  }

  async pathfind(
    tokenIn: string,
    tokenOut: string,
    amountIn: string,
    maxHops: number = 3
  ): Promise<PathfindResponse> {
    return this.post('pathfind', {
      token_in: tokenIn,
      token_out: tokenOut,
      amount_in: amountIn,
      max_hops: maxHops,
    });
  }
}

// Response types
export interface AddressBalancesResponse {
  ok: boolean;
  address: string;
  balances: Record<string, string>;
  outpoints?: OutpointInfo[];
}

export interface OutpointInfo {
  outpoint: string;
  entries: BalanceEntry[];
}

export interface BalanceEntry {
  alkane: string;
  amount: string;
}

export interface OutpointBalancesResponse {
  ok: boolean;
  outpoint: string;
  items: OutpointItem[];
}

export interface OutpointItem {
  outpoint: string;
  address?: string;
  entries: BalanceEntry[];
}

export interface HoldersResponse {
  ok: boolean;
  alkane: string;
  page: number;
  limit: number;
  total: number;
  has_more: boolean;
  items: HolderInfo[];
}

export interface HolderInfo {
  address: string;
  amount: string;
}

export interface HolderCountResponse {
  ok: boolean;
  alkane: string;
  count: number;
}

export interface AddressOutpointsResponse {
  ok: boolean;
  address: string;
  outpoints: OutpointInfo[];
}

export interface GetKeysResponse {
  ok: boolean;
  alkane: string;
  keys: Record<string, KeyValue>;
}

export interface KeyValue {
  key: string;
  value: string;
  last_txid: string;
  last_vout: number;
  block_height: number;
  updated_at: string;
}

export interface GetTradesResponse {
  ok: boolean;
  pool: string;
  trades: TradeInfo[];
}

export interface TradeInfo {
  txid: string;
  vout: number;
  token0: string;
  token1: string;
  amount0_in: string;
  amount1_in: string;
  amount0_out: string;
  amount1_out: string;
  reserve0_after: string;
  reserve1_after: string;
  timestamp: string;
  block_height: number;
}

export interface GetCandlesResponse {
  ok: boolean;
  pool: string;
  interval: string;
  candles: CandleInfo[];
}

export interface CandleInfo {
  open_time: string;
  close_time: string;
  open: string;
  high: string;
  low: string;
  close: string;
  volume0: string;
  volume1: string;
  trade_count: number;
}

export interface GetReservesResponse {
  ok: boolean;
  pool: string;
  reserve0: string;
  reserve1: string;
  timestamp: string;
  block_height: number;
}

export interface PathfindResponse {
  ok: boolean;
  paths: PathInfo[];
}

export interface PathInfo {
  hops: string[];
  pools: string[];
  estimated_output: string;
}

// Export default instance creator
export function createAlkanesClient(baseUrl: string, timeout?: number): AlkanesApiClient {
  return new AlkanesApiClient({ baseUrl, timeout });
}
