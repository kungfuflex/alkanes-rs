// Polyfills for browser build
import { Buffer } from 'buffer';
import { EventEmitter } from 'events';
import Stream from 'stream-browserify';

globalThis.Buffer = Buffer;
globalThis.process = globalThis.process || { env: {}, browser: true };
globalThis.Stream = Stream;
globalThis.EventEmitter = EventEmitter;
