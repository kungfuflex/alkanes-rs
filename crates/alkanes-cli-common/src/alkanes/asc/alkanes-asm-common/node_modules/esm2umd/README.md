# esm2umd

Transforms ESM to UMD, i.e. to use ESM by default with UMD as a legacy fallback.

[![Build Status](https://img.shields.io/github/actions/workflow/status/dcodeIO/esm2umd/test.yml?branch=main&label=test&logo=github)](https://github.com/dcodeIO/esm2umd/actions/workflows/test.yml) [![Publish Status](https://img.shields.io/github/actions/workflow/status/dcodeIO/esm2umd/publish.yml?branch=main&label=publish&logo=github)](https://github.com/dcodeIO/esm2umd/actions/workflows/publish.yml) [![npm](https://img.shields.io/npm/v/esm2umd.svg?label=npm&color=007acc&logo=npm)](https://www.npmjs.com/package/esm2umd)

## Usage

```
$> npm install --save-dev esm2umd
```

```
npx esm2umd MyModule esmFile.js > umdFile.js
```

`MyModule` is used as the name of the vanilla JS global.

If the module has a `default` export, it is transformed to a whole-module export.

## API

```js
import esm2umd from "esm2umd";

const esmCode = "...";
const umdCode = esm2umd("ModuleName", esmCode);
```

## Examples

Common outline of a hybrid module with legacy fallback, either exporting a class or a namespace:

**package.json**

```json
{
  "type": "module",
  "main": "./umd/index.js",
  "types": "./umd/index.d.ts",
  "exports": {
    ".": {
      "import": {
        "types": "./index.d.ts",
        "default": "./index.js"
      },
      "require": {
        "types": "./umd/index.d.ts",
        "default": "./umd/index.js"
      }
    }
  },
  "scripts": {
    "build": "npx esm2umd MyModule index.js > umd/index.js && cp types.d.ts umd/types.d.ts"
  }
}
```

**umd/package.json**

```json
{
  "type": "commonjs"
}
```

**.gitignore**

```
umd/index.js
umd/types.d.ts
```

### Class export

As used by [long.js](https://github.com/dcodeIO/long.js):

**index.d.ts**

```ts
import { MyClass } from "./types.js";
export default MyClass;
```

**types.d.ts**

```ts
export declare class MyClass {
  // ...
}
```

**umd/index.d.ts**

```ts
import { MyClass } from "./types.js";
export = MyClass;
export as namespace MyClass;
```

**umd/types.d.ts**

Copy of types.d.ts.

### Namespace export

As used by [bcrypt.js](https://github.com/dcodeIO/bcrypt.js):

**index.d.ts**

```ts
import * as myNamespace from "./types.js";
export * from "./types.js";
export default myNamespace;
```

**types.d.ts**

```ts
export declare function myFunction(): void;
// ...
```

**umd/index.d.ts**

```ts
import * as myNamespace from "./types.js";
export = myNamespace;
export as namespace myNamespace;
```

**umd/types.d.ts**

Copy of types.d.ts.

## Building

Building the UMD fallback:

```
$> npm run build
```

Running the [tests](./tests):

```
$> npm test
```
