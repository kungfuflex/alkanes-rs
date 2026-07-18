// Global type declarations for WebAssembly
declare namespace WebAssembly {
  interface Memory {
    buffer: ArrayBuffer;
  }

  interface Instance {
    exports: any;
  }

  interface Module {}

  interface Imports {
    [key: string]: any;
  }

  function instantiate(
    bytes: BufferSource,
    importObject?: Imports
  ): Promise<{ instance: Instance; module: Module }>;
}
