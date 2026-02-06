/**
 * Tests for WalletConnector class and wallet icons
 *
 * These tests verify that:
 * - WalletConnector methods work correctly
 * - All wallet icons are valid data URIs
 * - BROWSER_WALLETS array is properly configured
 */

import { describe, it, expect, beforeEach } from 'vitest';
import {
  WalletConnector,
  BROWSER_WALLETS,
  getWalletById,
  BrowserWalletInfo,
} from './index';
import { WALLET_ICONS, getWalletIcon } from './icons';

describe('WalletConnector', () => {
  let connector: WalletConnector;

  beforeEach(() => {
    connector = new WalletConnector();
  });

  describe('static getSupportedWallets', () => {
    it('should return all supported wallets', () => {
      const wallets = WalletConnector.getSupportedWallets();
      expect(Array.isArray(wallets)).toBe(true);
      expect(wallets.length).toBeGreaterThan(0);
    });

    it('should return the same wallets as BROWSER_WALLETS', () => {
      const wallets = WalletConnector.getSupportedWallets();
      expect(wallets).toEqual(BROWSER_WALLETS);
    });

    it('should include unisat wallet', () => {
      const wallets = WalletConnector.getSupportedWallets();
      const unisat = wallets.find(w => w.id === 'unisat');
      expect(unisat).toBeDefined();
      expect(unisat?.name).toBe('Unisat Wallet');
    });

    it('should include xverse wallet', () => {
      const wallets = WalletConnector.getSupportedWallets();
      const xverse = wallets.find(w => w.id === 'xverse');
      expect(xverse).toBeDefined();
      expect(xverse?.name).toBe('Xverse Wallet');
    });
  });

  describe('getWalletInfo', () => {
    it('should return wallet info for valid wallet id', () => {
      const info = connector.getWalletInfo('unisat');
      expect(info).toBeDefined();
      expect(info?.id).toBe('unisat');
      expect(info?.name).toBe('Unisat Wallet');
    });

    it('should return undefined for invalid wallet id', () => {
      const info = connector.getWalletInfo('nonexistent-wallet');
      expect(info).toBeUndefined();
    });

    it('should return correct info for all supported wallets', () => {
      const walletIds = ['unisat', 'xverse', 'phantom', 'okx', 'leather', 'magic-eden', 'wizz', 'orange', 'keplr'];

      for (const id of walletIds) {
        const info = connector.getWalletInfo(id);
        expect(info).toBeDefined();
        expect(info?.id).toBe(id);
      }
    });
  });

  describe('isWalletInstalled', () => {
    it('should return false for invalid wallet id', () => {
      const result = connector.isWalletInstalled('nonexistent-wallet');
      expect(result).toBe(false);
    });

    // In test environment without browser, wallets won't be installed
    it('should return false in non-browser environment', () => {
      const result = connector.isWalletInstalled('unisat');
      expect(result).toBe(false);
    });
  });

  describe('isConnected', () => {
    it('should return false when no wallet is connected', () => {
      expect(connector.isConnected()).toBe(false);
    });
  });

  describe('getConnectedWallet', () => {
    it('should return null when no wallet is connected', () => {
      expect(connector.getConnectedWallet()).toBeNull();
    });
  });

  describe('detectWallets', () => {
    it('should return empty array in non-browser environment', async () => {
      const wallets = await connector.detectWallets();
      expect(Array.isArray(wallets)).toBe(true);
      expect(wallets.length).toBe(0);
    });
  });
});

describe('BROWSER_WALLETS', () => {
  it('should have at least 5 wallets', () => {
    expect(BROWSER_WALLETS.length).toBeGreaterThanOrEqual(5);
  });

  it('should have valid BrowserWalletInfo structure for each wallet', () => {
    for (const wallet of BROWSER_WALLETS) {
      expect(typeof wallet.id).toBe('string');
      expect(typeof wallet.name).toBe('string');
      expect(typeof wallet.icon).toBe('string');
      expect(typeof wallet.website).toBe('string');
      expect(typeof wallet.injectionKey).toBe('string');
      expect(typeof wallet.supportsPsbt).toBe('boolean');
      expect(typeof wallet.supportsTaproot).toBe('boolean');
      expect(typeof wallet.supportsOrdinals).toBe('boolean');
      expect(typeof wallet.mobileSupport).toBe('boolean');
    }
  });

  it('should have unique ids', () => {
    const ids = BROWSER_WALLETS.map(w => w.id);
    const uniqueIds = [...new Set(ids)];
    expect(ids.length).toBe(uniqueIds.length);
  });

  it('should have data URI icons instead of file paths', () => {
    for (const wallet of BROWSER_WALLETS) {
      expect(wallet.icon.startsWith('data:image/svg+xml;base64,')).toBe(true);
    }
  });
});

describe('getWalletById', () => {
  it('should return wallet info for valid id', () => {
    const wallet = getWalletById('unisat');
    expect(wallet).toBeDefined();
    expect(wallet?.id).toBe('unisat');
  });

  it('should return undefined for invalid id', () => {
    const wallet = getWalletById('nonexistent');
    expect(wallet).toBeUndefined();
  });
});

describe('WALLET_ICONS', () => {
  const expectedWalletIds = ['unisat', 'xverse', 'phantom', 'okx', 'leather', 'magic-eden', 'wizz', 'oyl', 'orange', 'keplr'];

  it('should have icons for all expected wallets', () => {
    for (const id of expectedWalletIds) {
      expect(WALLET_ICONS[id]).toBeDefined();
    }
  });

  it('should have valid base64 data URIs', () => {
    for (const [id, icon] of Object.entries(WALLET_ICONS)) {
      expect(icon.startsWith('data:image/svg+xml;base64,')).toBe(true);

      // Extract and validate base64 content
      const base64 = icon.replace('data:image/svg+xml;base64,', '');
      expect(base64.length).toBeGreaterThan(100); // Icons should have substantial content

      // Try to decode base64
      const decoded = Buffer.from(base64, 'base64').toString('utf-8');
      expect(decoded).toContain('<svg');
      expect(decoded).toContain('</svg>');
    }
  });

  it('should have valid SVG content when decoded', () => {
    for (const [id, icon] of Object.entries(WALLET_ICONS)) {
      const base64 = icon.replace('data:image/svg+xml;base64,', '');
      const decoded = Buffer.from(base64, 'base64').toString('utf-8');

      // SVG should have proper structure
      expect(decoded).toContain('xmlns');
      expect(decoded).toMatch(/<svg[^>]*>/);
    }
  });
});

describe('getWalletIcon', () => {
  it('should return icon for valid wallet id', () => {
    const icon = getWalletIcon('unisat');
    expect(icon.startsWith('data:image/svg+xml;base64,')).toBe(true);
  });

  it('should return unisat icon as fallback for unknown wallet', () => {
    const icon = getWalletIcon('unknown-wallet');
    expect(icon).toBe(WALLET_ICONS.unisat);
  });

  it('should return correct icon for each wallet', () => {
    const walletIds = ['unisat', 'xverse', 'phantom', 'okx', 'leather', 'magic-eden', 'wizz', 'orange', 'keplr'];

    for (const id of walletIds) {
      const icon = getWalletIcon(id);
      expect(icon).toBe(WALLET_ICONS[id as keyof typeof WALLET_ICONS]);
    }
  });
});

describe('Wallet icons match BROWSER_WALLETS', () => {
  it('should have matching icons in BROWSER_WALLETS and WALLET_ICONS', () => {
    for (const wallet of BROWSER_WALLETS) {
      const iconFromWallet = wallet.icon;
      const iconFromIcons = WALLET_ICONS[wallet.id as keyof typeof WALLET_ICONS];

      if (iconFromIcons) {
        expect(iconFromWallet).toBe(iconFromIcons);
      }
    }
  });
});

describe('Icon rendering compatibility', () => {
  it('should have icons usable in img src attribute', () => {
    for (const wallet of BROWSER_WALLETS) {
      // Data URIs starting with data:image/ are valid for img src
      expect(wallet.icon.startsWith('data:image/')).toBe(true);
    }
  });

  it('should have properly encoded SVG content', () => {
    for (const wallet of BROWSER_WALLETS) {
      const base64 = wallet.icon.replace('data:image/svg+xml;base64,', '');

      // Should not throw when decoding
      expect(() => {
        Buffer.from(base64, 'base64');
      }).not.toThrow();
    }
  });
});
