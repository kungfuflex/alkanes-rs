use crate::ir::*;
use crate::manifest::AlkanesManifest;
use anyhow::{anyhow, Result};
use std::path::Path;
use wit_parser::{Resolve, Type, TypeDefKind, UnresolvedPackage};

/// Parse a .wit file (or directory) and alkanes.toml into an AlkaneContractIR.
pub fn parse(wit_path: &Path, manifest_path: &Path) -> Result<AlkaneContractIR> {
    let manifest = AlkanesManifest::from_file(manifest_path)?;

    let mut resolve = Resolve::default();

    let pkg_id = if wit_path.is_dir() {
        let (id, _) = resolve.push_dir(wit_path)
            .map_err(|e| anyhow!("failed to parse WIT directory {}: {}", wit_path.display(), e))?;
        id
    } else {
        let pkg = UnresolvedPackage::parse_file(wit_path)
            .map_err(|e| anyhow!("failed to parse WIT file {}: {}", wit_path.display(), e))?;
        resolve
            .push(pkg)
            .map_err(|e| anyhow!("failed to resolve WIT package: {}", e))?
    };

    let package = &resolve.packages[pkg_id];

    // Collect custom types from this package
    let mut custom_types = Vec::new();
    for (_name, &iface_id) in &package.interfaces {
        let iface = &resolve.interfaces[iface_id];
        for (type_name, &type_id) in &iface.types {
            if let Some(typedef) = convert_typedef(&resolve, type_name, type_id) {
                custom_types.push(typedef);
            }
        }
    }

    // Find the world's exports (the contract interface) and imports
    let mut methods = Vec::new();
    let mut imports = Vec::new();

    for (_world_name, &world_id) in &package.worlds {
        let world = &resolve.worlds[world_id];

        // Process exports - these are the contract's own methods
        for (_export_key, export_item) in &world.exports {
            match export_item {
                wit_parser::WorldItem::Interface(iface_id) => {
                    let iface = &resolve.interfaces[*iface_id];
                    for (func_name, func) in &iface.functions {
                        let method =
                            convert_function(&resolve, func_name, func, &manifest, false)?;
                        methods.push(method);
                    }
                }
                wit_parser::WorldItem::Function(func) => {
                    let method =
                        convert_function(&resolve, &func.name, func, &manifest, false)?;
                    methods.push(method);
                }
                _ => {}
            }
        }

        // Process imports - these become cross-contract call clients
        for (import_key, import_item) in &world.imports {
            if let wit_parser::WorldItem::Interface(iface_id) = import_item {
                // Determine the interface name from the key
                let iface_name = match import_key {
                    wit_parser::WorldKey::Name(name) => name.clone(),
                    wit_parser::WorldKey::Interface(id) => {
                        let iface = &resolve.interfaces[*id];
                        iface
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("import_{}", id.index()))
                    }
                };

                let iface = &resolve.interfaces[*iface_id];
                let mut import_methods = Vec::new();
                for (func_name, func) in &iface.functions {
                    let opcode = manifest.get_import_opcode(&iface_name, func_name)?;
                    let mut method =
                        convert_function(&resolve, func_name, func, &manifest, true)?;
                    method.opcode = opcode;
                    import_methods.push(method);
                }
                imports.push(AlkaneImportIR {
                    interface_name: iface_name.clone(),
                    rust_client_name: format!(
                        "{}Client",
                        wit_name_to_pascal_case(&iface_name)
                    ),
                    methods: import_methods,
                });
            }
        }
    }

    // If no world was defined, look for a standalone interface
    if methods.is_empty() {
        for (_iface_name, &iface_id) in &package.interfaces {
            let iface = &resolve.interfaces[iface_id];
            for (func_name, func) in &iface.functions {
                let method = convert_function(&resolve, func_name, func, &manifest, false)?;
                methods.push(method);
            }
            if !methods.is_empty() {
                break;
            }
        }
    }

    Ok(AlkaneContractIR {
        name: manifest.contract.name.clone(),
        methods,
        custom_types,
        imports,
    })
}

fn convert_function(
    resolve: &Resolve,
    func_name: &str,
    func: &wit_parser::Function,
    manifest: &AlkanesManifest,
    is_import: bool,
) -> Result<AlkaneMethodIR> {
    let opcode = if is_import {
        0 // will be set by caller for imports
    } else {
        manifest.get_opcode(func_name)?
    };

    let params: Vec<AlkaneParamIR> = func
        .params
        .iter()
        .map(|(name, ty)| {
            Ok(AlkaneParamIR {
                name: wit_name_to_snake_case(name),
                ty: convert_type(resolve, ty)?,
            })
        })
        .collect::<Result<_>>()?;

    let return_type = convert_return_type(resolve, &func.results)?;

    Ok(AlkaneMethodIR {
        wit_name: func_name.to_string(),
        rust_name: wit_name_to_snake_case(func_name),
        opcode,
        params,
        return_type,
        is_view: manifest.is_view(func_name),
    })
}

fn convert_type(resolve: &Resolve, ty: &Type) -> Result<AlkaneType> {
    match ty {
        Type::Bool => Ok(AlkaneType::Bool),
        Type::U8 => Ok(AlkaneType::U8),
        Type::U16 => Ok(AlkaneType::U16),
        Type::U32 => Ok(AlkaneType::U32),
        Type::U64 => Ok(AlkaneType::U64),
        Type::String => Ok(AlkaneType::String),
        Type::Id(id) => {
            let typedef = &resolve.types[*id];
            match &typedef.kind {
                TypeDefKind::Record(_record) => {
                    let name = typedef
                        .name
                        .as_ref()
                        .ok_or_else(|| anyhow!("anonymous record types not supported"))?;
                    if is_alkane_id_record(name) {
                        return Ok(AlkaneType::AlkaneId);
                    }
                    if is_call_response_record(name) {
                        return Ok(AlkaneType::U128); // placeholder, handled at return type level
                    }
                    Ok(AlkaneType::Record(wit_name_to_pascal_case(name)))
                }
                TypeDefKind::Enum(_e) => {
                    let name = typedef
                        .name
                        .as_ref()
                        .ok_or_else(|| anyhow!("anonymous enum types not supported"))?;
                    Ok(AlkaneType::Enum(wit_name_to_pascal_case(name)))
                }
                TypeDefKind::Variant(_v) => {
                    let name = typedef
                        .name
                        .as_ref()
                        .ok_or_else(|| anyhow!("anonymous variant types not supported"))?;
                    Ok(AlkaneType::Variant(wit_name_to_pascal_case(name)))
                }
                TypeDefKind::Option(inner) => {
                    Ok(AlkaneType::Option(Box::new(convert_type(resolve, inner)?)))
                }
                TypeDefKind::List(inner) => {
                    if matches!(inner, Type::U8) {
                        Ok(AlkaneType::Bytes)
                    } else {
                        Ok(AlkaneType::List(Box::new(convert_type(resolve, inner)?)))
                    }
                }
                TypeDefKind::Type(aliased) => convert_type(resolve, aliased),
                TypeDefKind::Tuple(tuple) => {
                    if tuple.types.len() == 2
                        && matches!(tuple.types[0], Type::U64)
                        && matches!(tuple.types[1], Type::U64)
                    {
                        Ok(AlkaneType::U128)
                    } else {
                        Err(anyhow!(
                            "unsupported tuple type (only tuple<u64, u64> for u128 is supported)"
                        ))
                    }
                }
                TypeDefKind::Result(result) => {
                    if let Some(ok_type) = &result.ok {
                        convert_type(resolve, ok_type)
                    } else {
                        Ok(AlkaneType::U128)
                    }
                }
                other => Err(anyhow!("unsupported WIT type kind: {:?}", other)),
            }
        }
        other => Err(anyhow!("unsupported WIT primitive type: {:?}", other)),
    }
}

fn convert_return_type(
    resolve: &Resolve,
    results: &wit_parser::Results,
) -> Result<AlkaneReturnType> {
    match results {
        wit_parser::Results::Named(params) => {
            if params.is_empty() {
                Ok(AlkaneReturnType::CallResponse)
            } else if params.len() == 1 {
                let (_, ty) = &params[0];
                convert_single_return_type(resolve, ty)
            } else {
                Err(anyhow!("multiple named return values not supported"))
            }
        }
        wit_parser::Results::Anon(ty) => convert_single_return_type(resolve, ty),
    }
}

fn convert_single_return_type(resolve: &Resolve, ty: &Type) -> Result<AlkaneReturnType> {
    if let Type::Id(id) = ty {
        let typedef = &resolve.types[*id];
        if let TypeDefKind::Result(result) = &typedef.kind {
            if let Some(ok_type) = &result.ok {
                return convert_single_return_type(resolve, ok_type);
            } else {
                return Ok(AlkaneReturnType::CallResponse);
            }
        }
        if let Some(name) = &typedef.name {
            if is_call_response_record(name) {
                return Ok(AlkaneReturnType::CallResponse);
            }
        }
    }

    let alkane_type = convert_type(resolve, ty)?;
    Ok(AlkaneReturnType::Typed(alkane_type))
}

fn convert_typedef(
    resolve: &Resolve,
    name: &str,
    type_id: wit_parser::TypeId,
) -> Option<AlkaneTypeDefIR> {
    let typedef = &resolve.types[type_id];

    // Skip well-known types
    if is_alkane_id_record(name)
        || is_call_response_record(name)
        || name == "alkane-transfer"
    {
        return None;
    }

    match &typedef.kind {
        TypeDefKind::Record(record) => {
            let fields = record
                .fields
                .iter()
                .filter_map(|field| {
                    let ty = convert_type(resolve, &field.ty).ok()?;
                    Some(AlkaneFieldIR {
                        name: wit_name_to_snake_case(&field.name),
                        ty,
                    })
                })
                .collect();
            Some(AlkaneTypeDefIR {
                name: wit_name_to_pascal_case(name),
                kind: AlkaneTypeDefKind::Record(fields),
            })
        }
        TypeDefKind::Enum(e) => {
            let cases = e
                .cases
                .iter()
                .map(|c| wit_name_to_pascal_case(&c.name))
                .collect();
            Some(AlkaneTypeDefIR {
                name: wit_name_to_pascal_case(name),
                kind: AlkaneTypeDefKind::Enum(cases),
            })
        }
        TypeDefKind::Variant(v) => {
            let cases = v
                .cases
                .iter()
                .filter_map(|c| {
                    let payload = c.ty.as_ref().and_then(|t| convert_type(resolve, t).ok());
                    Some(AlkaneVariantCaseIR {
                        name: wit_name_to_pascal_case(&c.name),
                        payload,
                    })
                })
                .collect();
            Some(AlkaneTypeDefIR {
                name: wit_name_to_pascal_case(name),
                kind: AlkaneTypeDefKind::Variant(cases),
            })
        }
        _ => None,
    }
}

fn is_alkane_id_record(name: &str) -> bool {
    name == "alkane-id" || name == "alkane_id" || name == "AlkaneId"
}

fn is_call_response_record(name: &str) -> bool {
    name == "call-response" || name == "call_response" || name == "CallResponse"
}

/// Convert a WIT kebab-case name to Rust snake_case.
pub fn wit_name_to_snake_case(name: &str) -> String {
    name.replace('-', "_")
}

/// Convert a WIT kebab-case name to Rust PascalCase.
pub fn wit_name_to_pascal_case(name: &str) -> String {
    name.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let mut s = c.to_uppercase().to_string();
                    s.extend(chars);
                    s
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wit_name_to_snake_case() {
        assert_eq!(wit_name_to_snake_case("get-name"), "get_name");
        assert_eq!(
            wit_name_to_snake_case("initialize-with-name-symbol"),
            "initialize_with_name_symbol"
        );
        assert_eq!(wit_name_to_snake_case("mint"), "mint");
    }

    #[test]
    fn test_wit_name_to_pascal_case() {
        assert_eq!(wit_name_to_pascal_case("owned-token"), "OwnedToken");
        assert_eq!(wit_name_to_pascal_case("token-ref"), "TokenRef");
        assert_eq!(wit_name_to_pascal_case("amm"), "Amm");
    }
}
