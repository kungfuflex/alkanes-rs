// WalletConnect re-export for @alkanes/ts-sdk.
//
// The actual TS surface (WalletConnectSession, DappRelay, Plaintext
// shapes, etc.) lives upstream at @subfrost/walletconnect. We re-export
// it here so consumers can `import { WalletConnectSession } from
// '@alkanes/ts-sdk/walletconnect'` without needing to remember the
// underlying package name.
//
// The Rust side of this story is fully self-contained in
// `vendor/subfrost-wc/` and is consumed by `alkanes-cli wc pair|
// accounts|sign-psbt|revoke`. This TS surface is what the browser-
// facing pieces of @alkanes/ts-sdk would import when they want to
// route signing through a paired Subfrost mobile wallet — there is no
// equivalent end-to-end JS flow in the CLI itself.
//
// Pairing flow (dapp role — typical for headless services / CLIs that
// pre-pair and then sign):
//
//   import {
//     WalletConnectSession,
//     DappRelay,
//   } from '@alkanes/ts-sdk/walletconnect';
//
//   const { session, init } = await WalletConnectSession.create({
//     relayUrl: 'wss://wc.subfrost.io/',
//     origin:   'https://my-app.example',
//   });
//   console.log(`Scan: ${init.uri}  code: ${init.pairingCode}`);
//
//   const relay = new DappRelay(session);
//   await relay.open();
//   const mobilePub = await relay.awaitAccepted();
//   session.attachMobilePub(mobilePub);
//
//   // Sign PSBT:
//   const req = await session.signPsbt(psbtHex, addresses);
//   const env = await relay.sendRequest(req);
//   const decoded = session.decode(env);
//   if (decoded.type === 'result') return decoded.result;  // signed PSBT hex
//
// Note: the upstream @subfrost/walletconnect package is currently only
// published from `subfrost/subfrost` (private repo). Until it is
// mirrored on a public registry, this submodule is a documentation
// stub — consumers should import from `@subfrost/walletconnect`
// directly while the bridge is being set up. Removing the
// `export * from` line lets @alkanes/ts-sdk CI publish without a
// `file:` dependency that doesn't resolve on GitHub Actions runners.
