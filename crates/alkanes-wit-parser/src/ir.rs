/// Intermediate representation of an alkanes contract parsed from WIT + alkanes.toml.

#[derive(Debug, Clone)]
pub struct AlkaneContractIR {
    pub name: String,
    pub methods: Vec<AlkaneMethodIR>,
    pub custom_types: Vec<AlkaneTypeDefIR>,
    pub imports: Vec<AlkaneImportIR>,
}

#[derive(Debug, Clone)]
pub struct AlkaneMethodIR {
    /// WIT name with dashes, e.g. "initialize-with-name-symbol"
    pub wit_name: String,
    /// Rust snake_case name, e.g. "initialize_with_name_symbol"
    pub rust_name: String,
    /// Opcode number for dispatch
    pub opcode: u128,
    /// Method parameters
    pub params: Vec<AlkaneParamIR>,
    /// Return type
    pub return_type: AlkaneReturnType,
    /// Whether this is a view (read-only) method
    pub is_view: bool,
}

#[derive(Debug, Clone)]
pub struct AlkaneParamIR {
    pub name: String,
    pub ty: AlkaneType,
}

#[derive(Debug, Clone)]
pub enum AlkaneReturnType {
    /// Returns CallResponse (default for mutating methods)
    CallResponse,
    /// Returns a specific type wrapped in CallResponse.data
    Typed(AlkaneType),
}

#[derive(Debug, Clone)]
pub enum AlkaneType {
    U128,
    U64,
    U32,
    U16,
    U8,
    Bool,
    String,
    /// list<u8> - raw bytes
    Bytes,
    /// AlkaneId
    AlkaneId,
    /// list<T> (not list<u8>)
    List(Box<AlkaneType>),
    /// option<T>
    Option(Box<AlkaneType>),
    /// A named record type
    Record(String),
    /// A named enum type
    Enum(String),
    /// A named variant type
    Variant(String),
}

#[derive(Debug, Clone)]
pub struct AlkaneTypeDefIR {
    pub name: String,
    pub kind: AlkaneTypeDefKind,
}

#[derive(Debug, Clone)]
pub enum AlkaneTypeDefKind {
    Record(Vec<AlkaneFieldIR>),
    Enum(Vec<String>),
    Variant(Vec<AlkaneVariantCaseIR>),
}

#[derive(Debug, Clone)]
pub struct AlkaneFieldIR {
    pub name: String,
    pub ty: AlkaneType,
}

#[derive(Debug, Clone)]
pub struct AlkaneVariantCaseIR {
    pub name: String,
    pub payload: Option<AlkaneType>,
}

/// Represents an imported contract interface (for cross-contract calls).
#[derive(Debug, Clone)]
pub struct AlkaneImportIR {
    pub interface_name: String,
    pub rust_client_name: String,
    pub methods: Vec<AlkaneMethodIR>,
}
