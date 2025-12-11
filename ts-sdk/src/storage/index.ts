/**
 * Storage and Backup Module for Alkanes SDK
 *
 * Provides:
 * - Local storage for keystore persistence
 * - Google Drive backup/restore for encrypted keystores
 * - Session storage for temporary wallet state
 *
 * @example
 * ```typescript
 * import { KeystoreStorage, GoogleDriveBackup } from '@alkanes/ts-sdk';
 *
 * // Local storage
 * const storage = new KeystoreStorage();
 * storage.saveKeystore(encryptedKeystore, 'mainnet');
 * const { keystore, network } = storage.loadKeystore();
 *
 * // Google Drive backup
 * const drive = new GoogleDriveBackup();
 * await drive.initialize();
 * await drive.backupWallet(encryptedKeystore, 'My Wallet', 'password hint');
 * const wallets = await drive.listWallets();
 * ```
 */

// Storage keys
const STORAGE_KEYS = {
  ENCRYPTED_KEYSTORE: 'alkanes_encrypted_keystore',
  WALLET_NETWORK: 'alkanes_wallet_network',
  SESSION_WALLET: 'alkanes_session_wallet',
} as const;

/**
 * Check if running in browser
 */
function isBrowser(): boolean {
  return typeof window !== 'undefined' && typeof localStorage !== 'undefined';
}

/**
 * Local keystore storage manager
 */
export class KeystoreStorage {
  /**
   * Save encrypted keystore to localStorage
   */
  saveKeystore(keystoreJson: string, network: string): void {
    if (!isBrowser()) {
      throw new Error('localStorage not available');
    }

    localStorage.setItem(STORAGE_KEYS.ENCRYPTED_KEYSTORE, keystoreJson);
    localStorage.setItem(STORAGE_KEYS.WALLET_NETWORK, network);
  }

  /**
   * Load encrypted keystore from localStorage
   */
  loadKeystore(): { keystore: string; network: string } | null {
    if (!isBrowser()) {
      return null;
    }

    const keystore = localStorage.getItem(STORAGE_KEYS.ENCRYPTED_KEYSTORE);
    const network = localStorage.getItem(STORAGE_KEYS.WALLET_NETWORK);

    if (!keystore) {
      return null;
    }

    return {
      keystore,
      network: network || 'mainnet',
    };
  }

  /**
   * Check if a keystore exists in localStorage
   */
  hasKeystore(): boolean {
    if (!isBrowser()) {
      return false;
    }
    return localStorage.getItem(STORAGE_KEYS.ENCRYPTED_KEYSTORE) !== null;
  }

  /**
   * Clear keystore from localStorage
   */
  clearKeystore(): void {
    if (!isBrowser()) {
      return;
    }

    localStorage.removeItem(STORAGE_KEYS.ENCRYPTED_KEYSTORE);
    localStorage.removeItem(STORAGE_KEYS.WALLET_NETWORK);
  }

  /**
   * Save wallet state to sessionStorage (survives page navigation, cleared on tab close)
   */
  saveSessionWallet(walletState: any): void {
    if (!isBrowser()) {
      return;
    }
    sessionStorage.setItem(STORAGE_KEYS.SESSION_WALLET, JSON.stringify(walletState));
  }

  /**
   * Load wallet state from sessionStorage
   */
  loadSessionWallet(): any | null {
    if (!isBrowser()) {
      return null;
    }

    const state = sessionStorage.getItem(STORAGE_KEYS.SESSION_WALLET);
    if (!state) {
      return null;
    }

    try {
      return JSON.parse(state);
    } catch {
      return null;
    }
  }

  /**
   * Clear session wallet state
   */
  clearSessionWallet(): void {
    if (!isBrowser()) {
      return;
    }
    sessionStorage.removeItem(STORAGE_KEYS.SESSION_WALLET);
  }
}

// ============================================================================
// Google Drive Backup
// ============================================================================

const DRIVE_FOLDER_NAME = '__ALKANES_WALLETS';
const DRIVE_SCOPES = 'https://www.googleapis.com/auth/drive.file';

/**
 * Wallet backup information from Google Drive
 */
export interface WalletBackupInfo {
  folderId: string;
  folderName: string;
  walletLabel: string;
  timestamp: string;
  createdDate: string;
  hasPasswordHint: boolean;
  folderUrl: string;
}

/**
 * Restored wallet data from Google Drive
 */
export interface RestoreWalletResult {
  encryptedKeystore: string;
  passwordHint: string | null;
  walletLabel: string;
  timestamp: string;
}

/**
 * Google Drive backup manager
 *
 * Client-side only - no backend involvement.
 * OAuth tokens are kept in memory only for security.
 */
export class GoogleDriveBackup {
  private clientId: string;
  private accessToken: string | null = null;
  private gapiInited = false;
  private gsiInited = false;

  constructor(clientId?: string) {
    this.clientId = clientId || (typeof process !== 'undefined' ? process.env.NEXT_PUBLIC_GOOGLE_DRIVE_CLIENT_ID || '' : '');
  }

  /**
   * Check if Google Drive backup is configured
   */
  isConfigured(): boolean {
    return !!this.clientId;
  }

  /**
   * Initialize Google API and Identity Services
   * Must be called before any Drive operations
   */
  async initialize(): Promise<void> {
    if (!this.isConfigured()) {
      throw new Error('Google Drive client ID not configured');
    }

    if (!isBrowser()) {
      throw new Error('Google Drive backup only available in browser');
    }

    await this.initGapi();
    this.initGsi();
  }

  /**
   * Request OAuth access token from user (opens popup)
   */
  async requestAccess(): Promise<string> {
    if (!this.gapiInited) {
      await this.initGapi();
    }
    if (!this.gsiInited) {
      this.initGsi();
    }

    if (this.accessToken) {
      return this.accessToken;
    }

    return new Promise((resolve, reject) => {
      const google = (window as any).google;

      const tokenClient = google.accounts.oauth2.initTokenClient({
        client_id: this.clientId,
        scope: DRIVE_SCOPES,
        callback: (response: any) => {
          if (response.error) {
            reject(new Error(response.error));
          } else {
            this.accessToken = response.access_token;
            resolve(response.access_token);
          }
        },
      });

      tokenClient.requestAccessToken({ prompt: 'consent' });
    });
  }

  /**
   * Clear access token and revoke with Google
   */
  clearAccess(): void {
    const token = this.accessToken;
    this.accessToken = null;

    if (isBrowser() && token) {
      const google = (window as any).google;
      if (google?.accounts?.oauth2?.revoke) {
        google.accounts.oauth2.revoke(token, () => {});
      }
    }
  }

  /**
   * Backup wallet to Google Drive
   */
  async backupWallet(
    encryptedKeystore: string,
    walletLabel?: string,
    passwordHint?: string
  ): Promise<{ folderId: string; folderName: string; timestamp: string; folderUrl: string }> {
    const token = await this.requestAccess();
    const gapi = (window as any).gapi;

    const rootFolderId = await this.getOrCreateRootFolder();

    // Create timestamp folder
    const timestamp = new Date().toISOString();
    const folderName = timestamp.replace(/[:.]/g, '-').replace(/Z$/, 'Z');

    const folder = await gapi.client.drive.files.create({
      resource: {
        name: folderName,
        mimeType: 'application/vnd.google-apps.folder',
        parents: [rootFolderId],
      },
      fields: 'id, webViewLink',
    });

    const folderId = folder.result.id;
    const folderUrl = folder.result.webViewLink;

    // Create keystore.json
    const keystoreData = {
      version: '1.0',
      timestamp,
      encryptedKeystore,
      walletLabel: walletLabel || 'My Bitcoin Wallet',
      backupMethod: 'google-drive-client-side',
    };

    await this.uploadFile(
      token,
      'keystore.json',
      JSON.stringify(keystoreData, null, 2),
      'application/json',
      folderId
    );

    // Create password_hint.txt if provided
    if (passwordHint) {
      await this.uploadFile(token, 'password_hint.txt', passwordHint, 'text/plain', folderId);
    }

    return { folderId, folderName, timestamp, folderUrl };
  }

  /**
   * List all wallet backups from Google Drive
   */
  async listWallets(): Promise<WalletBackupInfo[]> {
    const token = await this.requestAccess();
    const gapi = (window as any).gapi;

    const rootFolderId = await this.getOrCreateRootFolder();

    // List all subfolders
    const response = await gapi.client.drive.files.list({
      q: `'${rootFolderId}' in parents and mimeType='application/vnd.google-apps.folder' and trashed=false`,
      fields: 'files(id, name, createdTime, webViewLink)',
      orderBy: 'createdTime desc',
    });

    const wallets: WalletBackupInfo[] = [];

    for (const folder of response.result.files || []) {
      try {
        // Get keystore.json
        const keystoreList = await gapi.client.drive.files.list({
          q: `'${folder.id}' in parents and name='keystore.json' and trashed=false`,
          fields: 'files(id)',
        });

        if (!keystoreList.result.files?.length) {
          continue;
        }

        const keystoreFileId = keystoreList.result.files[0].id;
        const keystoreResponse = await fetch(
          `https://www.googleapis.com/drive/v3/files/${keystoreFileId}?alt=media`,
          { headers: { Authorization: `Bearer ${token}` } }
        );

        if (!keystoreResponse.ok) continue;

        const keystoreData = await keystoreResponse.json();

        // Check for password hint
        const hintList = await gapi.client.drive.files.list({
          q: `'${folder.id}' in parents and name='password_hint.txt' and trashed=false`,
          fields: 'files(id)',
        });

        wallets.push({
          folderId: folder.id,
          folderName: folder.name,
          walletLabel: keystoreData.walletLabel || 'My Wallet',
          timestamp: keystoreData.timestamp || folder.createdTime,
          createdDate: folder.createdTime,
          hasPasswordHint: !!(hintList.result.files?.length),
          folderUrl: folder.webViewLink,
        });
      } catch (error) {
        console.warn(`Error processing folder ${folder.name}:`, error);
      }
    }

    return wallets;
  }

  /**
   * Restore wallet from Google Drive
   */
  async restoreWallet(folderId: string): Promise<RestoreWalletResult> {
    const token = await this.requestAccess();
    const gapi = (window as any).gapi;

    // Get keystore.json
    const keystoreList = await gapi.client.drive.files.list({
      q: `'${folderId}' in parents and name='keystore.json' and trashed=false`,
      fields: 'files(id)',
    });

    if (!keystoreList.result.files?.length) {
      throw new Error('Keystore file not found in backup');
    }

    const keystoreFileId = keystoreList.result.files[0].id;
    const keystoreResponse = await fetch(
      `https://www.googleapis.com/drive/v3/files/${keystoreFileId}?alt=media`,
      { headers: { Authorization: `Bearer ${token}` } }
    );

    if (!keystoreResponse.ok) {
      throw new Error('Failed to download keystore from Google Drive');
    }

    const keystoreData = await keystoreResponse.json();

    // Try to get password hint
    let passwordHint: string | null = null;
    try {
      const hintList = await gapi.client.drive.files.list({
        q: `'${folderId}' in parents and name='password_hint.txt' and trashed=false`,
        fields: 'files(id)',
      });

      if (hintList.result.files?.length) {
        const hintFileId = hintList.result.files[0].id;
        const hintResponse = await fetch(
          `https://www.googleapis.com/drive/v3/files/${hintFileId}?alt=media`,
          { headers: { Authorization: `Bearer ${token}` } }
        );

        if (hintResponse.ok) {
          passwordHint = await hintResponse.text();
        }
      }
    } catch {
      // Continue without hint
    }

    return {
      encryptedKeystore: keystoreData.encryptedKeystore,
      passwordHint,
      walletLabel: keystoreData.walletLabel || 'My Wallet',
      timestamp: keystoreData.timestamp,
    };
  }

  /**
   * Delete a wallet backup from Google Drive
   */
  async deleteWallet(folderId: string): Promise<void> {
    await this.requestAccess();
    const gapi = (window as any).gapi;

    await gapi.client.drive.files.delete({ fileId: folderId });
  }

  // Private helpers

  private async initGapi(): Promise<void> {
    if (this.gapiInited) return;

    return new Promise((resolve, reject) => {
      const gapi = (window as any).gapi;
      if (!gapi) {
        reject(
          new Error(
            'Google API not loaded. Add <script src="https://apis.google.com/js/api.js"></script> to your page.'
          )
        );
        return;
      }

      gapi.load('client', async () => {
        try {
          await gapi.client.init({
            apiKey: '',
            discoveryDocs: ['https://www.googleapis.com/discovery/v1/apis/drive/v3/rest'],
          });
          this.gapiInited = true;
          resolve();
        } catch (error) {
          reject(error);
        }
      });
    });
  }

  private initGsi(): void {
    if (this.gsiInited) return;

    const google = (window as any).google;
    if (!google?.accounts) {
      throw new Error(
        'Google Identity Services not loaded. Add <script src="https://accounts.google.com/gsi/client"></script> to your page.'
      );
    }

    this.gsiInited = true;
  }

  private async getOrCreateRootFolder(): Promise<string> {
    const gapi = (window as any).gapi;

    const response = await gapi.client.drive.files.list({
      q: `name='${DRIVE_FOLDER_NAME}' and mimeType='application/vnd.google-apps.folder' and trashed=false`,
      fields: 'files(id, name)',
      spaces: 'drive',
    });

    if (response.result.files?.length) {
      return response.result.files[0].id;
    }

    const folder = await gapi.client.drive.files.create({
      resource: {
        name: DRIVE_FOLDER_NAME,
        mimeType: 'application/vnd.google-apps.folder',
      },
      fields: 'id',
    });

    return folder.result.id;
  }

  private async uploadFile(
    token: string,
    fileName: string,
    content: string,
    mimeType: string,
    parentFolderId: string
  ): Promise<string> {
    const metadata = {
      name: fileName,
      mimeType,
      parents: [parentFolderId],
    };

    const form = new FormData();
    form.append('metadata', new Blob([JSON.stringify(metadata)], { type: 'application/json' }));
    form.append('file', new Blob([content], { type: mimeType }));

    const response = await fetch(
      'https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart&fields=id',
      {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}` },
        body: form,
      }
    );

    if (!response.ok) {
      throw new Error(`Upload failed: ${await response.text()}`);
    }

    const result = await response.json();
    return result.id;
  }
}

// Utility functions for formatting

/**
 * Format timestamp for display
 */
export function formatBackupDate(timestamp: string): string {
  try {
    const date = new Date(timestamp);
    return date.toLocaleString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  } catch {
    return timestamp;
  }
}

/**
 * Get relative time string
 */
export function getRelativeTime(timestamp: string): string {
  try {
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    if (diffMins < 1) return 'Just now';
    if (diffMins < 60) return `${diffMins} minute${diffMins > 1 ? 's' : ''} ago`;
    if (diffHours < 24) return `${diffHours} hour${diffHours > 1 ? 's' : ''} ago`;
    if (diffDays < 7) return `${diffDays} day${diffDays > 1 ? 's' : ''} ago`;
    if (diffDays < 30) return `${Math.floor(diffDays / 7)} week${Math.floor(diffDays / 7) > 1 ? 's' : ''} ago`;
    return `${Math.floor(diffDays / 30)} month${Math.floor(diffDays / 30) > 1 ? 's' : ''} ago`;
  } catch {
    return 'Unknown';
  }
}
