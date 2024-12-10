#[link(wasm_import_module = "env")]
extern "C" {
    pub fn abort(a: i32, b: i32, c: i32, d: i32) -> i32;
    pub fn __load_storage(k: i32, v: i32) -> i32;
    pub fn __request_storage(k: i32) -> i32;
    pub fn __log(v: i32);
    pub fn __balance(who: i32, what: i32, output: i32);
    pub fn __request_context() -> i32;
    pub fn __load_context(output: i32) -> i32;
    pub fn __sequence(output: i32);
    pub fn __fuel(output: i32);
    pub fn __height(output: i32);
    pub fn __returndatacopy(output: i32);
    pub fn __request_transaction() -> i32;
    pub fn __load_transaction(output: i32);
    pub fn __request_block() -> i32;
    pub fn __load_block(output: i32);
    pub fn __call(cellpack: i32, incoming_alkanes: i32, checkpoint: i32, start_fuel: u64) -> i32;
    pub fn __staticcall(
        cellpack: i32,
        incoming_alkanes: i32,
        checkpoint: i32,
        start_fuel: u64,
    ) -> i32;
    pub fn __delegatecall(
        cellpack: i32,
        incoming_alkanes: i32,
        checkpoint: i32,
        start_fuel: u64,
    ) -> i32;
}
