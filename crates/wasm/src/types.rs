//! derived from Cranelift/wasm-types
//!
//! Internal dependency of Wasmtime and c2zk that defines types for
//! WebAssembly.

use derive_more::From;
pub use wasmparser;

use std::convert::{TryFrom, TryInto};
use std::fmt;

use crate::error::{WasmError, WasmResult};

/// WebAssembly value type -- equivalent of `wasmparser`'s Type.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum WasmType {
    /// I32 type
    I32,
    /// I64 type
    I64,
    /// F32 type
    F32,
    /// F64 type
    F64,
    /// V128 type
    V128,
    /// FuncRef type
    FuncRef,
    /// ExternRef type
    ExternRef,
}

impl TryFrom<wasmparser::ValType> for WasmType {
    type Error = WasmError;
    fn try_from(ty: wasmparser::ValType) -> Result<Self, Self::Error> {
        use wasmparser::ValType::*;
        match ty {
            I32 => Ok(WasmType::I32),
            I64 => Ok(WasmType::I64),
            F32 => Ok(WasmType::F32),
            F64 => Ok(WasmType::F64),
            V128 => Ok(WasmType::V128),
            FuncRef => Ok(WasmType::FuncRef),
            ExternRef => Ok(WasmType::ExternRef),
        }
    }
}

impl From<WasmType> for wasmparser::ValType {
    fn from(ty: WasmType) -> wasmparser::ValType {
        match ty {
            WasmType::I32 => wasmparser::ValType::I32,
            WasmType::I64 => wasmparser::ValType::I64,
            WasmType::F32 => wasmparser::ValType::F32,
            WasmType::F64 => wasmparser::ValType::F64,
            WasmType::V128 => wasmparser::ValType::V128,
            WasmType::FuncRef => wasmparser::ValType::FuncRef,
            WasmType::ExternRef => wasmparser::ValType::ExternRef,
        }
    }
}

impl fmt::Display for WasmType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WasmType::I32 => write!(f, "i32"),
            WasmType::I64 => write!(f, "i64"),
            WasmType::F32 => write!(f, "f32"),
            WasmType::F64 => write!(f, "f64"),
            WasmType::V128 => write!(f, "v128"),
            WasmType::ExternRef => write!(f, "externref"),
            WasmType::FuncRef => write!(f, "funcref"),
        }
    }
}

/// WebAssembly function type -- equivalent of `wasmparser`'s FuncType.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct WasmFuncType {
    params: Box<[WasmType]>,
    externref_params_count: usize,
    returns: Box<[WasmType]>,
    externref_returns_count: usize,
}

impl WasmFuncType {
    #[inline]
    pub fn new(params: Box<[WasmType]>, returns: Box<[WasmType]>) -> Self {
        let externref_params_count = params.iter().filter(|p| **p == WasmType::ExternRef).count();
        let externref_returns_count = returns
            .iter()
            .filter(|r| **r == WasmType::ExternRef)
            .count();
        WasmFuncType {
            params,
            externref_params_count,
            returns,
            externref_returns_count,
        }
    }

    /// Function params types.
    #[allow(dead_code)]
    #[inline]
    pub fn params(&self) -> &[WasmType] {
        &self.params
    }

    /// How many `externref`s are in this function's params?
    #[allow(dead_code)]
    #[inline]
    pub fn externref_params_count(&self) -> usize {
        self.externref_params_count
    }

    /// Returns params types.
    #[allow(dead_code)]
    #[inline]
    pub fn returns(&self) -> &[WasmType] {
        &self.returns
    }

    /// How many `externref`s are in this function's returns?
    #[allow(dead_code)]
    #[inline]
    pub fn externref_returns_count(&self) -> usize {
        self.externref_returns_count
    }
}

impl TryFrom<wasmparser::FuncType> for WasmFuncType {
    type Error = WasmError;
    fn try_from(ty: wasmparser::FuncType) -> Result<Self, Self::Error> {
        let params = ty
            .params()
            .iter()
            .copied()
            .map(WasmType::try_from)
            .collect::<Result<_, Self::Error>>()?;
        let returns = ty
            .results()
            .iter()
            .copied()
            .map(WasmType::try_from)
            .collect::<Result<_, Self::Error>>()?;
        Ok(Self::new(params, returns))
    }
}

/// Index type of a function (imported or defined) inside the WebAssembly module.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, From)]
pub struct FuncIndex(u32);

/// Index type of a defined function inside the WebAssembly module.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, From)]
pub struct DefinedFuncIndex(u32);

/// Index type of a defined table inside the WebAssembly module.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, From)]
pub struct DefinedTableIndex(u32);

/// Index type of a defined memory inside the WebAssembly module.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, From)]
pub struct DefinedMemoryIndex(u32);

/// Index type of a defined memory inside the WebAssembly module.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, From)]
pub struct OwnedMemoryIndex(u32);

/// Index type of a defined global inside the WebAssembly module.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, From)]
pub struct DefinedGlobalIndex(u32);

/// Index type of a table (imported or defined) inside the WebAssembly module.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, From)]
pub struct TableIndex(u32);

/// Index type of a global variable (imported or defined) inside the WebAssembly module.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, From)]
pub struct GlobalIndex(u32);

/// Index type of a linear memory (imported or defined) inside the WebAssembly module.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, From)]
pub struct MemoryIndex(u32);

/// Index type of a signature (imported or defined) inside the WebAssembly module.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, From)]
pub struct SignatureIndex(u32);

/// Index type of a passive data segment inside the WebAssembly module.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, From)]
pub struct DataIndex(u32);

/// Index type of a passive element segment inside the WebAssembly module.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, From)]
pub struct ElemIndex(u32);

/// Index type of a type inside the WebAssembly module.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, From)]
pub struct TypeIndex(u32);

/// Index type of an event inside the WebAssembly module.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, From)]
pub struct TagIndex(u32);

/// An index of an entity.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum EntityIndex {
    /// Function index.
    Function(FuncIndex),
    /// Table index.
    Table(TableIndex),
    /// Memory index.
    Memory(MemoryIndex),
    /// Global index.
    Global(GlobalIndex),
}

impl From<FuncIndex> for EntityIndex {
    fn from(idx: FuncIndex) -> EntityIndex {
        EntityIndex::Function(idx)
    }
}

impl From<TableIndex> for EntityIndex {
    fn from(idx: TableIndex) -> EntityIndex {
        EntityIndex::Table(idx)
    }
}

impl From<MemoryIndex> for EntityIndex {
    fn from(idx: MemoryIndex) -> EntityIndex {
        EntityIndex::Memory(idx)
    }
}

impl From<GlobalIndex> for EntityIndex {
    fn from(idx: GlobalIndex) -> EntityIndex {
        EntityIndex::Global(idx)
    }
}

/// A type of an item in a wasm module where an item is typically something that
/// can be exported.
#[allow(missing_docs)]
#[derive(Clone, Debug)]
pub enum EntityType {
    /// A global variable with the specified content type
    Global(Global),
    /// A linear memory with the specified limits
    Memory(Memory),
    /// An event definition.
    Tag(Tag),
    /// A table with the specified element type and limits
    Table(Table),
    /// A function type where the index points to the type section and records a
    /// function signature.
    Function(SignatureIndex),
}

/// A WebAssembly global.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct Global {
    /// The Wasm type of the value stored in the global.
    pub wasm_ty: WasmType,
    /// A flag indicating whether the value may change at runtime.
    pub mutability: bool,
    /// The source of the initial value.
    pub initializer: GlobalInit,
}

/// Globals are initialized via the `const` operators or by referring to another import.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum GlobalInit {
    /// An `i32.const`.
    I32Const(i32),
    /// An `i64.const`.
    I64Const(i64),
    /// An `f32.const`.
    F32Const(u32),
    /// An `f64.const`.
    F64Const(u64),
    /// A `vconst`.
    V128Const(u128),
    /// A `global.get` of another global.
    GetGlobal(GlobalIndex),
    /// A `ref.null`.
    RefNullConst,
    /// A `ref.func <index>`.
    RefFunc(FuncIndex),
    ///< The global is imported from, and thus initialized by, a different module.
    Import,
}

impl Global {
    /// Creates a new `Global` type from wasmparser's representation.
    #[allow(dead_code)]
    pub fn new(ty: wasmparser::GlobalType, initializer: GlobalInit) -> WasmResult<Global> {
        Ok(Global {
            wasm_ty: ty.content_type.try_into()?,
            mutability: ty.mutable,
            initializer,
        })
    }
}

/// WebAssembly table.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct Table {
    /// The table elements' Wasm type.
    pub wasm_ty: WasmType,
    /// The minimum number of elements in the table.
    pub minimum: u32,
    /// The maximum number of elements in the table.
    pub maximum: Option<u32>,
}

impl TryFrom<wasmparser::TableType> for Table {
    type Error = WasmError;

    fn try_from(ty: wasmparser::TableType) -> WasmResult<Table> {
        Ok(Table {
            wasm_ty: ty.element_type.try_into()?,
            minimum: ty.initial,
            maximum: ty.maximum,
        })
    }
}

/// WebAssembly linear memory.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct Memory {
    /// The minimum number of pages in the memory.
    pub minimum: u64,
    /// The maximum number of pages in the memory.
    pub maximum: Option<u64>,
    /// Whether the memory may be shared between multiple threads.
    pub shared: bool,
    /// Whether or not this is a 64-bit memory
    pub memory64: bool,
}

impl From<wasmparser::MemoryType> for Memory {
    fn from(ty: wasmparser::MemoryType) -> Memory {
        Memory {
            minimum: ty.initial,
            maximum: ty.maximum,
            shared: ty.shared,
            memory64: ty.memory64,
        }
    }
}

/// WebAssembly event.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct Tag {
    /// The event signature type.
    pub ty: TypeIndex,
}

impl From<wasmparser::TagType> for Tag {
    fn from(ty: wasmparser::TagType) -> Tag {
        match ty.kind {
            wasmparser::TagKind::Exception => Tag {
                ty: TypeIndex(ty.func_type_idx),
            },
        }
    }
}
