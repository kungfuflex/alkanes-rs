# fast-sha256-as

AssemblyScript port of fast-sha256-js.

## Usage

```js
import { sha256 } from "fast-sha256-as";

function hashSomeData(data: ArrayBuffer): ArrayBuffer {
  return sha256(data);
}
```

## Testing

```sh
git clone https://github.com/sandshrewmetaprotocols/fast-sha256-as
cd fast-sha256-as
yarn
yarn build
yarn test
```

## Author

Sandshrew Inc

## License

MIT
