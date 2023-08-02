//! This module contains the definitions of the operations of the Wasm dialect.

#![allow(unused_imports)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]

use std::collections::HashMap;
use std::ops::Deref;

use apint::ApInt;
use derive_more::Display;
use intertrait::cast_to;
use ozk_ozk_dialect::attributes::apint_to_i32;
use ozk_ozk_dialect::attributes::i32_attr;
use ozk_ozk_dialect::attributes::u32_attr;
use ozk_ozk_dialect::types::i32_type;
use ozk_ozk_dialect::types::i64_type;
use ozk_ozk_dialect::types::u32_type_unwrapped;
use ozk_ozk_dialect::types::FuncSym;
use pliron::attribute;
use pliron::attribute::attr_cast;
use pliron::attribute::AttrObj;
use pliron::basic_block::BasicBlock;
use pliron::common_traits::DisplayWithContext;
use pliron::common_traits::Verify;
use pliron::context::Context;
use pliron::context::Ptr;
use pliron::declare_op;
use pliron::dialect::Dialect;
use pliron::dialects::builtin::attr_interfaces::TypedAttrInterface;
use pliron::dialects::builtin::attributes::FloatAttr;
use pliron::dialects::builtin::attributes::IntegerAttr;
use pliron::dialects::builtin::attributes::SmallDictAttr;
use pliron::dialects::builtin::attributes::StringAttr;
use pliron::dialects::builtin::attributes::TypeAttr;
use pliron::dialects::builtin::attributes::VecAttr;
use pliron::dialects::builtin::op_interfaces::OneRegionInterface;
use pliron::dialects::builtin::op_interfaces::SingleBlockRegionInterface;
use pliron::dialects::builtin::op_interfaces::SymbolOpInterface;
use pliron::dialects::builtin::types::FunctionType;
use pliron::dialects::builtin::types::IntegerType;
use pliron::dialects::builtin::types::Signedness;
use pliron::error::CompilerError;
use pliron::linked_list::ContainsLinkedList;
use pliron::op::Op;
use pliron::operation::Operation;
use pliron::operation::WalkOrder;
use pliron::operation::WalkResult;
use pliron::r#type::TypeObj;
use pliron::with_context::AttachContext;

use crate::types::FuncIndex;
use crate::types::GlobalIndex;
use crate::types::LocalIndex;
use crate::types::RelativeDepth;

declare_op!(
    /// Represents a Wasm module, a top level container operation.
    ///
    /// See MLIR's [builtin.module](https://mlir.llvm.org/docs/Dialects/Builtin/#builtinmodule-mlirmoduleop).
    /// It contains a single [SSACFG](super::op_interfaces::RegionKind::SSACFG)
    /// region containing a single block which can contain any operations and
    /// does not have a terminator.
    ///
    /// Attributes:
    ///
    /// | key | value |
    /// |-----|-------|
    /// | [ATTR_KEY_SYM_NAME](super::ATTR_KEY_SYM_NAME) | [StringAttr](super::attributes::StringAttr) |
    /// | [ATTR_KEY_START_FUNC_SYM](ModuleOp::ATTR_KEY_START_FUNC_SYM) | [StringAttr](super::attributes::StringAttr) |
    ModuleOp,
    "module",
    "wasm"
);

impl DisplayWithContext for ModuleOp {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let region = self.get_region(ctx).with_ctx(ctx).to_string();
        write!(
            f,
            "{} {{\n{}}}",
            self.get_opid().with_ctx(ctx),
            indent::indent_all_by(2, region),
        )
    }
}

impl Verify for ModuleOp {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        // TODO: check that the start function is defined.
        self.verify_interfaces(ctx)?;
        self.get_region(ctx).deref(ctx).verify(ctx)
    }
}

impl ModuleOp {
    /// Attribute key for the the start function symbol.
    pub const ATTR_KEY_START_FUNC_SYM: &str = "module.start_func_sym";
    /// Attribute key for the all function symbols in this module (including imports).
    pub const ATTR_KEY_FUNC_SYMBOLS: &str = "module.func_symbols";
    /// Attribute key for the import function types.
    pub const ATTR_KEY_IMPORT_FUNC_TYPES: &str = "module.import_func_types";
    /// Attribute key for the import function modules.
    pub const ATTR_KEY_IMPORT_FUNC_MODULES: &str = "module.import_func_modules";

    /// Create a new [ModuleOp].
    /// The underlying [Operation] is not linked to a [BasicBlock](crate::basic_block::BasicBlock).
    /// The returned module has a single [crate::region::Region] with a single (BasicBlock)[crate::basic_block::BasicBlock].
    pub fn new(
        ctx: &mut Context,
        start_func_name: Option<FuncSym>,
        all_func_syms: Vec<FuncSym>,
        functions: Vec<FuncOp>,
        import_func_types: Vec<Ptr<TypeObj>>,
        import_func_modules: Vec<String>,
    ) -> ModuleOp {
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 1);
        {
            let opref = &mut *op.deref_mut(ctx);
            if let Some(start_func_sym) = start_func_name {
                opref.attributes.insert(
                    Self::ATTR_KEY_START_FUNC_SYM,
                    StringAttr::create(start_func_sym.into()),
                );
            }
            opref.attributes.insert(
                Self::ATTR_KEY_FUNC_SYMBOLS,
                VecAttr::create(
                    all_func_syms
                        .into_iter()
                        .map(|func_sym| StringAttr::create(func_sym.into()))
                        .collect(),
                ),
            );
            opref.attributes.insert(
                Self::ATTR_KEY_IMPORT_FUNC_TYPES,
                VecAttr::create(
                    import_func_types
                        .into_iter()
                        .map(TypeAttr::create)
                        .collect(),
                ),
            );
            opref.attributes.insert(
                Self::ATTR_KEY_IMPORT_FUNC_MODULES,
                VecAttr::create(
                    import_func_modules
                        .into_iter()
                        .map(StringAttr::create)
                        .collect(),
                ),
            );
        }

        let opop = ModuleOp { op };

        // Create an empty block.
        let region = opop.get_region(ctx);
        let block = BasicBlock::new(ctx, None, vec![]);
        block.insert_at_front(region, ctx);

        for func_op in functions {
            opop.append_function(ctx, func_op);
        }

        opop
    }

    /// Add an [Operation] into this module.
    pub fn append_function(&self, ctx: &mut Context, func_op: FuncOp) -> FuncIndex {
        let func_index = {
            let mut self_op = self.get_operation().deref_mut(ctx);
            #[allow(clippy::expect_used)]
            let func_indices_attr = self_op
                .attributes
                .get_mut(Self::ATTR_KEY_FUNC_SYMBOLS)
                .expect("ModuleOp has no function symbols vector attribute")
                .downcast_mut::<VecAttr>()
                .expect("ModuleOp function symbols vector attribute is not a VecAttr");
            func_indices_attr
                .0
                .push(StringAttr::create(func_op.get_symbol_name(ctx)));
            func_indices_attr.0.len() - 1
        };
        self.append_operation(ctx, func_op.get_operation(), 0);
        func_index.into()
    }

    /// Return the start function symbol name
    pub fn get_start_func_sym(&self, ctx: &Context) -> Option<FuncSym> {
        let self_op = self.get_operation().deref(ctx);
        self_op
            .attributes
            .get(Self::ATTR_KEY_START_FUNC_SYM)
            .map(|s_attr| {
                String::from(
                    s_attr
                        .downcast_ref::<StringAttr>()
                        .expect("ModuleOp start function symbol attribute is not a StringAttr")
                        .clone(),
                ).into()
            })
    }

    fn get_func_syms(&self, ctx: &Context) -> Vec<FuncSym> {
        let self_op = self.get_operation().deref(ctx);
        let v_attr = self_op
            .attributes
            .get(Self::ATTR_KEY_FUNC_SYMBOLS)
            .expect("ModuleOp has no function symbols vector attribute");
        v_attr
            .downcast_ref::<VecAttr>()
            .expect("ModuleOp function symbols vector attribute is not a VecAttr")
            .0
            .iter()
            .map(|attr: &AttrObj| {
                let str: String = attr
                    .downcast_ref::<StringAttr>()
                    .expect("ModuleOp function symbol is not a StringAttr")
                    .clone()
                    .into();
                FuncSym::from(str)
            })
            .collect()
    }

    /// Return the function symbol name for the given function index.
    pub fn get_func_sym(&self, ctx: &Context, func_index: FuncIndex) -> Option<FuncSym> {
        self.get_func_syms(ctx)
            .get(usize::from(func_index))
            .cloned()
    }

    /// Return the function index for the given function symbol name.
    pub fn get_func_index(&self, ctx: &Context, func_sym: FuncSym) -> Option<FuncIndex> {
        self.get_func_syms(ctx)
            .iter()
            .position(|sym| *sym == func_sym)
            .map(Into::into)
    }

    pub fn get_func(&self, ctx: &Context, func_sym: &FuncSym) -> Option<FuncOp> {
        for op in self.get_body(ctx, 0).deref(ctx).iter(ctx) {
            let deref_op = &op.deref(ctx).get_op(ctx);
            let Some(func_op) = deref_op.downcast_ref::<FuncOp>() else {
                continue;
            };
            if func_op.get_symbol_name(ctx) == func_sym.as_ref() {
                return Some(*func_op);
            }
        }
        None
    }
}

impl OneRegionInterface for ModuleOp {}
impl SingleBlockRegionInterface for ModuleOp {}

declare_op!(
    /// Function
    FuncOp,
    "func",
    "wasm"
);

impl FuncOp {
    /// Attribute key for the function type
    pub const ATTR_KEY_FUNC_TYPE: &str = "func.type";
    pub const ATTR_KEY_FUNC_LOCALS: &str = "func.locals";

    /// Create a new [FuncOp].
    /// The underlying [Operation] is not linked to a [BasicBlock](crate::basic_block::BasicBlock).
    /// The returned function has a single region with a passed `entry` block.
    pub fn new_unlinked_with_block(
        ctx: &mut Context,
        name: FuncSym,
        ty: Ptr<TypeObj>,
        entry_block: Ptr<BasicBlock>,
        locals: Vec<Ptr<TypeObj>>,
    ) -> FuncOp {
        let ty_attr = TypeAttr::create(ty);
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 1);
        {
            let opref = &mut *op.deref_mut(ctx);
            opref.attributes.insert(Self::ATTR_KEY_FUNC_TYPE, ty_attr);
            opref.attributes.insert(
                Self::ATTR_KEY_FUNC_LOCALS,
                VecAttr::create(locals.into_iter().map(TypeAttr::create).collect()),
            );
        }
        let opop = FuncOp { op };
        // Create an empty entry block.
        let region = opop.get_region(ctx);
        entry_block.insert_at_front(region, ctx);

        opop.set_symbol_name(ctx, name.as_ref());

        opop
    }

    /// Get the function signature (type).
    pub fn get_type_attr(&self, ctx: &Context) -> Ptr<TypeObj> {
        let opref = self.get_operation().deref(ctx);
        #[allow(clippy::unwrap_used)]
        let ty_attr = opref.attributes.get(Self::ATTR_KEY_FUNC_TYPE).unwrap();
        #[allow(clippy::unwrap_used)]
        attr_cast::<dyn TypedAttrInterface>(&**ty_attr)
            .unwrap()
            .get_type()
    }

    /// Get the function signature (type).
    pub fn get_type(&self, ctx: &Context) -> FunctionType {
        let func_type_obj = self.get_type_attr(ctx).deref(ctx);
        #[allow(clippy::panic)]
        let Some(func_type) = func_type_obj.downcast_ref::<FunctionType>() else {
            panic!("FuncOp type is not a FunctionType");
        };
        func_type.clone()
    }

    /// Get the entry block of this function.
    pub fn get_entry_block(&self, ctx: &Context) -> Ptr<BasicBlock> {
        #[allow(clippy::unwrap_used)]
        self.get_region(ctx).deref(ctx).get_head().unwrap()
    }

    /// Get an iterator over all operations.
    pub fn op_iter<'a>(&self, ctx: &'a Context) -> impl Iterator<Item = Ptr<Operation>> + 'a {
        self.get_region(ctx)
            .deref(ctx)
            .iter(ctx)
            .flat_map(|bb| bb.deref(ctx).iter(ctx))
    }

    /// Get the local variables types
    pub fn get_locals(&self, ctx: &Context) -> Vec<Ptr<TypeObj>> {
        let self_op = self.get_operation().deref(ctx);
        let v_attr = self_op
            .attributes
            .get(Self::ATTR_KEY_FUNC_LOCALS)
            .expect("FuncOp has no locals attribute");
        v_attr
            .downcast_ref::<VecAttr>()
            .expect("FuncOp locals attribute is not a VecAttr")
            .0
            .iter()
            .map(|attr: &AttrObj| {
                attr.downcast_ref::<TypeAttr>()
                    .expect("FuncOp local is not a TypeAttr")
                    .clone()
                    .get_type()
            })
            .collect()
    }
}

impl OneRegionInterface for FuncOp {}
#[cast_to]
impl SymbolOpInterface for FuncOp {}

impl DisplayWithContext for FuncOp {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let region = self.get_region(ctx).with_ctx(ctx).to_string();
        write!(
            f,
            "{} @{}{} {{\n{}}}",
            self.get_opid().with_ctx(ctx),
            self.get_symbol_name(ctx),
            self.get_type_attr(ctx).with_ctx(ctx),
            indent::indent_all_by(2, region),
        )
    }
}

impl Verify for FuncOp {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        let ty = self.get_type_attr(ctx);

        if !(ty.deref(ctx).is::<FunctionType>()) {
            return Err(CompilerError::VerificationError {
                msg: "Unexpected Func type".to_string(),
            });
        }
        let op = &*self.get_operation().deref(ctx);
        if op.get_opid() != Self::get_opid_static() {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect OpId".to_string(),
            });
        }
        if op.get_num_results() != 0 || op.get_num_operands() != 0 {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect number of results or operands".to_string(),
            });
        }
        self.verify_interfaces(ctx)?;
        self.get_entry_block(ctx).verify(ctx)?;
        Ok(())
    }
}

declare_op!(
    /// Push numeric constant on stack.
    ///
    /// Attributes:
    ///
    /// | key | value |
    /// |-----|-------|
    /// |[ATTR_KEY_VALUE](ConstantOp::ATTR_KEY_VALUE) | [IntegerAttr] or [FloatAttr] |
    ///
    ConstantOp,
    "const",
    "wasm"
);

impl ConstantOp {
    /// Attribute key for the constant value.
    pub const ATTR_KEY_VALUE: &str = "const.value";
    /// Get the constant value that this Op defines.
    pub fn get_value(&self, ctx: &Context) -> AttrObj {
        let op = self.get_operation().deref(ctx);
        #[allow(clippy::expect_used)]
        let value = op
            .attributes
            .get(Self::ATTR_KEY_VALUE)
            .expect("no attribute found");
        if value.is::<IntegerAttr>() {
            attribute::clone::<IntegerAttr>(value)
        } else {
            attribute::clone::<FloatAttr>(value)
        }
    }

    /// Create a new [ConstOp]. The underlying [Operation] is not linked to a
    /// [BasicBlock](crate::basic_block::BasicBlock).
    pub fn new_unlinked(ctx: &mut Context, val: AttrObj) -> ConstantOp {
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 0);
        op.deref_mut(ctx)
            .attributes
            .insert(Self::ATTR_KEY_VALUE, val);
        ConstantOp { op }
    }

    /// Create a new i32 [ConstOp]. The underlying [Operation] is not linked to a
    /// [BasicBlock](crate::basic_block::BasicBlock).
    pub fn new_i32_unlinked(ctx: &mut Context, val: i32) -> ConstantOp {
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 0);
        let val_attr = i32_attr(ctx, val);
        op.deref_mut(ctx)
            .attributes
            .insert(Self::ATTR_KEY_VALUE, val_attr);
        ConstantOp { op }
    }
}

impl DisplayWithContext for ConstantOp {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} {}",
            self.get_opid().with_ctx(ctx),
            self.get_value(ctx).with_ctx(ctx)
        )
    }
}

impl Verify for ConstantOp {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        let value = self.get_value(ctx);
        if !(value.is::<IntegerAttr>() || value.is::<FloatAttr>()) {
            return Err(CompilerError::VerificationError {
                msg: "Unexpected constant type".to_string(),
            });
        }
        let op = &*self.get_operation().deref(ctx);
        if op.get_opid() != Self::get_opid_static() {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect OpId".to_string(),
            });
        }
        if op.get_num_results() != 0 || op.get_num_operands() != 0 {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect number of results or operands".to_string(),
            });
        }
        Ok(())
    }
}

// TODO: store expected operand types (poped from stack)?

declare_op!(
    /// Push two top stack items, sums them and push result on stack
    ///
    /// Attributes:
    ///
    /// | key | value |
    /// |-----|-------|
    /// | [ATTR_KEY_OP_TYPE](FuncOp::ATTR_KEY_OP_TYPE) | [TypeAttr](super::attributes::TypeAttr) |
    ///
    AddOp,
    "add",
    "wasm"
);

impl AddOp {
    /// Attribute key
    pub const ATTR_KEY_OP_TYPE: &str = "add.type";
    /// Create a new [AddOp]. The underlying [Operation] is not linked to a
    /// [BasicBlock](crate::basic_block::BasicBlock).
    pub fn new_unlinked(ctx: &mut Context, ty: Ptr<TypeObj>) -> ConstantOp {
        let ty_attr = TypeAttr::create(ty);
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 0);
        op.deref_mut(ctx)
            .attributes
            .insert(Self::ATTR_KEY_OP_TYPE, ty_attr);
        ConstantOp { op }
    }

    /// Get the type of the operands and the result of this operation.
    pub fn get_type(&self, ctx: &Context) -> Ptr<TypeObj> {
        let opref = self.get_operation().deref(ctx);
        #[allow(clippy::expect_used)]
        let ty_attr = opref
            .attributes
            .get(Self::ATTR_KEY_OP_TYPE)
            .expect("no type attribute");
        #[allow(clippy::expect_used)]
        attr_cast::<dyn TypedAttrInterface>(&**ty_attr)
            .expect("invalid type attribute")
            .get_type()
    }
}

impl DisplayWithContext for AddOp {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.get_opid().with_ctx(ctx),)
    }
}

impl Verify for AddOp {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        let op = &*self.get_operation().deref(ctx);
        if op.get_opid() != Self::get_opid_static() {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect OpId".to_string(),
            });
        }
        if op.get_num_results() != 0 || op.get_num_operands() != 0 {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect number of results or operands".to_string(),
            });
        }
        Ok(())
    }
}

declare_op!(
    /// Call a function by it's index in the module
    ///
    /// https://webassembly.github.io/spec/core/syntax/instructions.html#syntax-instr-control
    ///
    CallOp,
    "call",
    "wasm"
);

impl CallOp {
    const ATTR_KEY_FUNC_INDEX: &str = "call.func_index";

    /// Get the function index
    pub fn get_func_index(&self, ctx: &Context) -> FuncIndex {
        let op = self.get_operation().deref(ctx);
        #[allow(clippy::expect_used)]
        let func_index = op
            .attributes
            .get(Self::ATTR_KEY_FUNC_INDEX)
            .expect("no attribute found");
        #[allow(clippy::expect_used)]
        let func_index = apint_to_i32(
            func_index
                .downcast_ref::<IntegerAttr>()
                .expect("ModuleOp function index is not an IntegerAttr")
                .clone()
                .into(),
        ) as u32;
        func_index.into()
    }

    /// Create a new [CallOp]. The underlying [Operation] is not linked to a
    /// [BasicBlock](crate::basic_block::BasicBlock).
    pub fn new_unlinked(ctx: &mut Context, func_index: FuncIndex) -> CallOp {
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 0);
        let func_index_attr = u32_attr(ctx, func_index.into());
        op.deref_mut(ctx)
            .attributes
            .insert(Self::ATTR_KEY_FUNC_INDEX, func_index_attr);
        CallOp { op }
    }
}

impl DisplayWithContext for CallOp {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} {}",
            self.get_opid().with_ctx(ctx),
            self.get_func_index(ctx)
        )
    }
}

impl Verify for CallOp {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        let op = &*self.get_operation().deref(ctx);
        if op.get_opid() != Self::get_opid_static() {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect OpId".to_string(),
            });
        }
        if op.get_num_results() != 0 || op.get_num_operands() != 0 {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect number of results or operands".to_string(),
            });
        }
        Ok(())
    }
}

declare_op!(
    /// Return (branch to the outermost block)
    /// https://webassembly.github.io/spec/core/syntax/instructions.html#syntax-instr-control
    ReturnOp,
    "return",
    "wasm"
);

impl ReturnOp {
    /// Create a new op
    pub fn new_unlinked(ctx: &mut Context) -> ReturnOp {
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 0);
        ReturnOp { op }
    }
}

impl DisplayWithContext for ReturnOp {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.get_opid().with_ctx(ctx),)
    }
}

impl Verify for ReturnOp {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        let op = &*self.get_operation().deref(ctx);
        if op.get_opid() != Self::get_opid_static() {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect OpId".to_string(),
            });
        }
        if op.get_num_results() != 0 || op.get_num_operands() != 0 {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect number of results or operands".to_string(),
            });
        }
        Ok(())
    }
}

declare_op!(
    /// A block operation containing a single region.
    BlockOp,
    "block",
    "wasm"
);

impl BlockOp {
    /// Attribute key for the function type
    pub const ATTR_KEY_BLOCK_TYPE: &str = "block.type";

    /// Create a new [BlockOp].
    pub fn new_unlinked(ctx: &mut Context, ty: Ptr<TypeObj>) -> BlockOp {
        let ty_attr = TypeAttr::create(ty);
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 1);
        {
            let opref = &mut *op.deref_mut(ctx);
            // Set function type attributes.
            opref.attributes.insert(Self::ATTR_KEY_BLOCK_TYPE, ty_attr);
        }
        let opop = BlockOp { op };
        // Create an empty block.
        #[allow(clippy::expect_used)]
        let region = opop.get_region(ctx);
        let body = BasicBlock::new(ctx, Some("entry".to_string()), vec![]);
        body.insert_at_front(region, ctx);

        opop
    }

    /// Get the signature (type).
    pub fn get_type(&self, ctx: &Context) -> Ptr<TypeObj> {
        let opref = self.get_operation().deref(ctx);
        #[allow(clippy::unwrap_used)]
        let ty_attr = opref.attributes.get(Self::ATTR_KEY_BLOCK_TYPE).unwrap();
        #[allow(clippy::unwrap_used)]
        attr_cast::<dyn TypedAttrInterface>(&**ty_attr)
            .unwrap()
            .get_type()
    }

    /// Get the bb of this block.
    pub fn get_block(&self, ctx: &Context) -> Ptr<BasicBlock> {
        #[allow(clippy::unwrap_used)]
        self.get_region(ctx).deref(ctx).get_head().unwrap()
    }

    /// Get an iterator over all operations.
    pub fn op_iter<'a>(&self, ctx: &'a Context) -> impl Iterator<Item = Ptr<Operation>> + 'a {
        self.get_region(ctx)
            .deref(ctx)
            .iter(ctx)
            .flat_map(|bb| bb.deref(ctx).iter(ctx))
    }
}

impl OneRegionInterface for BlockOp {}
impl DisplayWithContext for BlockOp {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let region = self.get_region(ctx).with_ctx(ctx).to_string();
        write!(
            f,
            "{} {} {{\n{}}}",
            self.get_opid().with_ctx(ctx),
            self.get_type(ctx).with_ctx(ctx),
            indent::indent_all_by(2, region),
        )
    }
}

impl Verify for BlockOp {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        let ty = self.get_type(ctx);

        if !(ty.deref(ctx).is::<FunctionType>()) {
            return Err(CompilerError::VerificationError {
                msg: "Unexpected Block type".to_string(),
            });
        }
        let op = &*self.get_operation().deref(ctx);
        if op.get_opid() != Self::get_opid_static() {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect OpId".to_string(),
            });
        }
        if op.get_num_results() != 0 || op.get_num_operands() != 0 {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect number of results or operands".to_string(),
            });
        }
        self.verify_interfaces(ctx)?;
        self.get_block(ctx).verify(ctx)?;
        Ok(())
    }
}

declare_op!(
    /// A loop block operation containing a single region.
    ///
    /// Attributes:
    ///
    /// | key | value |
    /// |-----|-------|
    /// | [ATTR_KEY_BLOCK_TYPE](Self::ATTR_KEY_BLOCK_TYPE) | [TypeAttr](super::attributes::TypeAttr) |
    LoopOp,
    "loop",
    "wasm"
);

impl LoopOp {
    /// Attribute key for the function type
    pub const ATTR_KEY_BLOCK_TYPE: &str = "block.type";

    /// Create a new [LoopOp].
    pub fn new_unlinked(ctx: &mut Context, ty: Ptr<TypeObj>) -> LoopOp {
        let ty_attr = TypeAttr::create(ty);
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 1);
        {
            let opref = &mut *op.deref_mut(ctx);
            // Set function type attributes.
            opref.attributes.insert(Self::ATTR_KEY_BLOCK_TYPE, ty_attr);
        }
        let opop = LoopOp { op };
        // Create an empty block.
        #[allow(clippy::expect_used)]
        let region = opop.get_region(ctx);
        let body = BasicBlock::new(ctx, Some("entry".to_string()), vec![]);
        body.insert_at_front(region, ctx);

        opop
    }

    /// Get the signature (type).
    pub fn get_type(&self, ctx: &Context) -> Ptr<TypeObj> {
        let opref = self.get_operation().deref(ctx);
        #[allow(clippy::unwrap_used)]
        let ty_attr = opref.attributes.get(Self::ATTR_KEY_BLOCK_TYPE).unwrap();
        #[allow(clippy::unwrap_used)]
        attr_cast::<dyn TypedAttrInterface>(&**ty_attr)
            .unwrap()
            .get_type()
    }

    /// Get the bb of this block.
    pub fn get_block(&self, ctx: &Context) -> Ptr<BasicBlock> {
        #[allow(clippy::unwrap_used)]
        self.get_region(ctx).deref(ctx).get_head().unwrap()
    }

    /// Get an iterator over all operations.
    pub fn op_iter<'a>(&self, ctx: &'a Context) -> impl Iterator<Item = Ptr<Operation>> + 'a {
        self.get_region(ctx)
            .deref(ctx)
            .iter(ctx)
            .flat_map(|bb| bb.deref(ctx).iter(ctx))
    }
}

impl OneRegionInterface for LoopOp {}
impl DisplayWithContext for LoopOp {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let region = self.get_region(ctx).with_ctx(ctx).to_string();
        write!(
            f,
            "{} {} {{\n{}}}",
            self.get_opid().with_ctx(ctx),
            self.get_type(ctx).with_ctx(ctx),
            indent::indent_all_by(2, region),
        )
    }
}

impl Verify for LoopOp {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        let ty = self.get_type(ctx);

        if !(ty.deref(ctx).is::<FunctionType>()) {
            return Err(CompilerError::VerificationError {
                msg: "Unexpected Block type".to_string(),
            });
        }
        let op = &*self.get_operation().deref(ctx);
        if op.get_opid() != Self::get_opid_static() {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect OpId".to_string(),
            });
        }
        if op.get_num_results() != 0 || op.get_num_operands() != 0 {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect number of results or operands".to_string(),
            });
        }
        self.verify_interfaces(ctx)?;
        self.get_block(ctx).verify(ctx)?;
        Ok(())
    }
}

declare_op!(
    /// Push local variable with the given index onto the stack.
    ///
    /// Attributes:
    ///
    /// | key | value |
    /// |-----|-------|
    /// |[ATTR_KEY_INDEX](Self::ATTR_KEY_INDEX) | [IntegerAttr] |
    ///
    LocalGetOp,
    "local.get",
    "wasm"
);

impl LocalGetOp {
    /// Attribute key for the index
    pub const ATTR_KEY_INDEX: &str = "local.get.index";

    /// Get the index of the local variable.
    pub fn get_index_as_attr(&self, ctx: &Context) -> AttrObj {
        let op = self.get_operation().deref(ctx);
        #[allow(clippy::expect_used)]
        let value = op
            .attributes
            .get(Self::ATTR_KEY_INDEX)
            .expect("no attribute found");
        attribute::clone::<IntegerAttr>(value)
    }

    /// Create a new [LocalGetOp].
    pub fn new_unlinked(ctx: &mut Context, index: u32) -> LocalGetOp {
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 0);

        let index_attr = u32_attr(ctx, index);
        op.deref_mut(ctx)
            .attributes
            .insert(Self::ATTR_KEY_INDEX, index_attr);
        LocalGetOp { op }
    }

    /// Get the index of the local variable.
    pub fn get_index(&self, ctx: &Context) -> LocalIndex {
        let attr = self.get_index_as_attr(ctx);
        let value_u32 = apint_to_i32(
            attr.downcast_ref::<IntegerAttr>()
                .expect("index is not an IntegerAttr")
                .clone()
                .into(),
        ) as u32;
        value_u32.into()
    }
}

impl DisplayWithContext for LocalGetOp {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} {}",
            self.get_opid().with_ctx(ctx),
            self.get_index(ctx)
        )
    }
}

impl Verify for LocalGetOp {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        let index = self.get_index_as_attr(ctx);
        if let Ok(index_attr) = index.downcast::<IntegerAttr>() {
            #[allow(clippy::unwrap_used)]
            if index_attr.get_type() != u32_type_unwrapped(ctx) {
                return Err(CompilerError::VerificationError {
                    msg: "Expected u32 for index".to_string(),
                });
            }
        } else {
            return Err(CompilerError::VerificationError {
                msg: "Unexpected index type".to_string(),
            });
        };
        let op = &*self.get_operation().deref(ctx);
        if op.get_opid() != Self::get_opid_static() {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect OpId".to_string(),
            });
        }
        if op.get_num_results() != 0 || op.get_num_operands() != 0 {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect number of results or operands".to_string(),
            });
        }
        Ok(())
    }
}

declare_op!(
    /// Pops the stack and save the value into the local variable with the given index
    ///
    /// Attributes:
    ///
    /// | key | value |
    /// |-----|-------|
    /// |[ATTR_KEY_INDEX](Self::ATTR_KEY_INDEX) | [IntegerAttr] |
    ///
    LocalSetOp,
    "local.set",
    "wasm"
);

impl LocalSetOp {
    /// Attribute key for the index
    pub const ATTR_KEY_INDEX: &str = "local.set.index";

    /// Get the index of the local variable.
    pub fn get_index_attr(&self, ctx: &Context) -> AttrObj {
        let op = self.get_operation().deref(ctx);
        #[allow(clippy::expect_used)]
        let value = op
            .attributes
            .get(Self::ATTR_KEY_INDEX)
            .expect("no attribute found");
        attribute::clone::<IntegerAttr>(value)
    }

    /// Get the index of the local variable.
    pub fn get_index(&self, ctx: &Context) -> LocalIndex {
        let attr = self.get_index_attr(ctx);
        let value_u32 = apint_to_i32(
            attr.downcast_ref::<IntegerAttr>()
                .expect("index is not an IntegerAttr")
                .clone()
                .into(),
        ) as u32;
        value_u32.into()
    }

    /// Create a new [LocalSetOp].
    pub fn new_unlinked(ctx: &mut Context, index: u32) -> LocalSetOp {
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 0);

        let index_attr = u32_attr(ctx, index);
        op.deref_mut(ctx)
            .attributes
            .insert(Self::ATTR_KEY_INDEX, index_attr);
        LocalSetOp { op }
    }
}

impl DisplayWithContext for LocalSetOp {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} {}",
            self.get_opid().with_ctx(ctx),
            self.get_index_attr(ctx).with_ctx(ctx)
        )
    }
}

impl Verify for LocalSetOp {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        let index = self.get_index_attr(ctx);
        if let Ok(index_attr) = index.downcast::<IntegerAttr>() {
            #[allow(clippy::unwrap_used)]
            if index_attr.get_type() != u32_type_unwrapped(ctx) {
                return Err(CompilerError::VerificationError {
                    msg: "Expected u32 for index".to_string(),
                });
            }
        } else {
            return Err(CompilerError::VerificationError {
                msg: "Unexpected index type".to_string(),
            });
        };
        let op = &*self.get_operation().deref(ctx);
        if op.get_opid() != Self::get_opid_static() {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect OpId".to_string(),
            });
        }
        if op.get_num_results() != 0 || op.get_num_operands() != 0 {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect number of results or operands".to_string(),
            });
        }
        Ok(())
    }
}

declare_op!(
    /// Saves the value from the stack (without popping it) into the local variable with the given index
    ///
    LocalTeeOp,
    "local.tee",
    "wasm"
);

impl LocalTeeOp {
    /// Attribute key for the index
    pub const ATTR_KEY_INDEX: &str = "local.tee.index";

    /// Get the index of the local variable.
    pub fn get_index(&self, ctx: &Context) -> AttrObj {
        let op = self.get_operation().deref(ctx);
        #[allow(clippy::expect_used)]
        let value = op
            .attributes
            .get(Self::ATTR_KEY_INDEX)
            .expect("no attribute found");
        attribute::clone::<IntegerAttr>(value)
    }

    /// Create a new [LocalTeeOp].
    pub fn new_unlinked(ctx: &mut Context, index: u32) -> LocalTeeOp {
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 0);

        let index_attr = u32_attr(ctx, index);
        op.deref_mut(ctx)
            .attributes
            .insert(Self::ATTR_KEY_INDEX, index_attr);
        LocalTeeOp { op }
    }
}

impl DisplayWithContext for LocalTeeOp {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} {}",
            self.get_opid().with_ctx(ctx),
            self.get_index(ctx).with_ctx(ctx)
        )
    }
}

impl Verify for LocalTeeOp {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        let index = self.get_index(ctx);
        if let Ok(index_attr) = index.downcast::<IntegerAttr>() {
            #[allow(clippy::unwrap_used)]
            if index_attr.get_type() != u32_type_unwrapped(ctx) {
                return Err(CompilerError::VerificationError {
                    msg: "Expected u32 for index".to_string(),
                });
            }
        } else {
            return Err(CompilerError::VerificationError {
                msg: "Unexpected index type".to_string(),
            });
        };
        let op = &*self.get_operation().deref(ctx);
        if op.get_opid() != Self::get_opid_static() {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect OpId".to_string(),
            });
        }
        if op.get_num_results() != 0 || op.get_num_operands() != 0 {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect number of results or operands".to_string(),
            });
        }
        Ok(())
    }
}

declare_op!(
    /// Pops the stack and save the value into the global variable with the given index
    ///
    /// Attributes:
    ///
    /// | key | value |
    /// |-----|-------|
    /// |[ATTR_KEY_INDEX](Self::ATTR_KEY_INDEX) | [IntegerAttr] |
    ///
    GlobalSetOp,
    "global.set",
    "wasm"
);

impl GlobalSetOp {
    /// Attribute key for the index
    pub const ATTR_KEY_INDEX: &str = "global.set.index";

    /// Get the index of the global variable.
    pub fn get_index(&self, ctx: &Context) -> GlobalIndex {
        let op = self.get_operation().deref(ctx);
        #[allow(clippy::expect_used)]
        let value = op
            .attributes
            .get(Self::ATTR_KEY_INDEX)
            .expect("no attribute for index found");
        let value_u32 = apint_to_i32(
            value
                .downcast_ref::<IntegerAttr>()
                .expect("index is not an IntegerAttr")
                .clone()
                .into(),
        ) as u32;
        value_u32.into()
    }

    /// Create a new [GlobalSetOp].
    pub fn new_unlinked(ctx: &mut Context, index: GlobalIndex) -> GlobalSetOp {
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 0);

        let index_attr = u32_attr(ctx, index.into());
        op.deref_mut(ctx)
            .attributes
            .insert(Self::ATTR_KEY_INDEX, index_attr);
        GlobalSetOp { op }
    }
}

impl DisplayWithContext for GlobalSetOp {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} {}",
            self.get_opid().with_ctx(ctx),
            self.get_index(ctx)
        )
    }
}

impl Verify for GlobalSetOp {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        // let index = self.get_index(ctx);
        // if let Ok(index_attr) = index.downcast::<IntegerAttr>() {
        //     #[allow(clippy::unwrap_used)]
        //     if index_attr.get_type() != u32_type_unwrapped(ctx) {
        //         return Err(CompilerError::VerificationError {
        //             msg: "Expected u32 for index".to_string(),
        //         });
        //     }
        // } else {
        //     return Err(CompilerError::VerificationError {
        //         msg: "Unexpected index type".to_string(),
        //     });
        // };
        let op = &*self.get_operation().deref(ctx);
        if op.get_opid() != Self::get_opid_static() {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect OpId".to_string(),
            });
        }
        if op.get_num_results() != 0 || op.get_num_operands() != 0 {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect number of results or operands".to_string(),
            });
        }
        Ok(())
    }
}

declare_op!(
    /// Push global variable with the given index onto the stack.
    ///
    /// Attributes:
    ///
    /// | key | value |
    /// |-----|-------|
    /// |[ATTR_KEY_INDEX](Self::ATTR_KEY_INDEX) | [IntegerAttr] |
    ///
    GlobalGetOp,
    "global.get",
    "wasm"
);

impl GlobalGetOp {
    /// Attribute key for the index
    pub const ATTR_KEY_INDEX: &str = "global.get.index";

    /// Get the index of the global variable.
    pub fn get_index(&self, ctx: &Context) -> GlobalIndex {
        let op = self.get_operation().deref(ctx);
        #[allow(clippy::expect_used)]
        let value = op
            .attributes
            .get(Self::ATTR_KEY_INDEX)
            .expect("no attribute for index found");
        let value_u32 = apint_to_i32(
            value
                .downcast_ref::<IntegerAttr>()
                .expect("index is not an IntegerAttr")
                .clone()
                .into(),
        ) as u32;
        value_u32.into()
    }

    /// Create a new [GlobalGetOp].
    pub fn new_unlinked(ctx: &mut Context, index: u32) -> GlobalGetOp {
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 0);

        let index_attr = u32_attr(ctx, index);
        op.deref_mut(ctx)
            .attributes
            .insert(Self::ATTR_KEY_INDEX, index_attr);
        GlobalGetOp { op }
    }
}

impl DisplayWithContext for GlobalGetOp {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} {}",
            self.get_opid().with_ctx(ctx),
            self.get_index(ctx),
        )
    }
}

impl Verify for GlobalGetOp {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        let op = &*self.get_operation().deref(ctx);
        if op.get_opid() != Self::get_opid_static() {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect OpId".to_string(),
            });
        }
        if op.get_num_results() != 0 || op.get_num_operands() != 0 {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect number of results or operands".to_string(),
            });
        }
        Ok(())
    }
}

/// The type of a [StoreOp] or [LoadOp]
#[derive(Debug, Copy, Clone, PartialEq, Display)]
pub enum MemAccessOpValueType {
    /// i32
    I32,
    /// i64
    I64,
}

declare_op!(
    /// Pops the i32 or i64 value and i32 addresss from stack and save the value at the address.
    ///
    StoreOp,
    "store",
    "wasm"
);

impl StoreOp {
    const ATTR_KEY_VALUE_TYPE: &str = "store.value.type";

    /// Create a new [StoreOp].
    pub fn new_unlinked(ctx: &mut Context, ty: MemAccessOpValueType) -> StoreOp {
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 0);

        let value_type_attr = match ty {
            MemAccessOpValueType::I32 => i32_type(ctx),
            MemAccessOpValueType::I64 => i64_type(ctx),
        };
        op.deref_mut(ctx)
            .attributes
            .insert(Self::ATTR_KEY_VALUE_TYPE, TypeAttr::create(value_type_attr));
        StoreOp { op }
    }

    /// Get the type of the value.
    pub fn get_value_type(&self, ctx: &Context) -> MemAccessOpValueType {
        let op = self.get_operation().deref(ctx);
        let value = op
            .attributes
            .get(Self::ATTR_KEY_VALUE_TYPE)
            .expect("no attribute found");
        let ty = value
            .downcast_ref::<TypeAttr>()
            .expect("Expected TypeAttr")
            .get_type()
            .deref(ctx);
        let int_ty = ty
            .downcast_ref::<IntegerType>()
            .expect("Expected IntegerType");
        assert!(int_ty.get_signedness() == Signedness::Signed);
        match int_ty.get_width() {
            32 => MemAccessOpValueType::I32,
            64 => MemAccessOpValueType::I64,
            _ => panic!("Unexpected bitwidth"),
        }
    }
}

impl DisplayWithContext for StoreOp {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} {}",
            self.get_opid().with_ctx(ctx),
            self.get_value_type(ctx)
        )
    }
}

impl Verify for StoreOp {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        let op = &*self.get_operation().deref(ctx);
        if op.get_opid() != Self::get_opid_static() {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect OpId".to_string(),
            });
        }
        if op.get_num_results() != 0 || op.get_num_operands() != 0 {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect number of results or operands".to_string(),
            });
        }
        Ok(())
    }
}

declare_op!(
    /// push the i32 or i64 value loaded from i32 addresss poped from the stack
    ///
    LoadOp,
    "load",
    "wasm"
);

impl LoadOp {
    const ATTR_KEY_VALUE_TYPE: &str = "store.value.type";

    /// Create a new [LoadOp].
    pub fn new_unlinked(ctx: &mut Context, ty: MemAccessOpValueType) -> LoadOp {
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 0);

        let value_type_attr = match ty {
            MemAccessOpValueType::I32 => i32_type(ctx),
            MemAccessOpValueType::I64 => i64_type(ctx),
        };
        op.deref_mut(ctx)
            .attributes
            .insert(Self::ATTR_KEY_VALUE_TYPE, TypeAttr::create(value_type_attr));
        LoadOp { op }
    }

    /// Get the type of the value.
    pub fn get_value_type(&self, ctx: &Context) -> MemAccessOpValueType {
        let op = self.get_operation().deref(ctx);
        let value = op
            .attributes
            .get(Self::ATTR_KEY_VALUE_TYPE)
            .expect("no attribute found");
        let ty = value
            .downcast_ref::<TypeAttr>()
            .expect("Expected TypeAttr")
            .get_type()
            .deref(ctx);
        let int_ty = ty
            .downcast_ref::<IntegerType>()
            .expect("Expected IntegerType");
        assert!(int_ty.get_signedness() == Signedness::Signed);
        match int_ty.get_width() {
            32 => MemAccessOpValueType::I32,
            64 => MemAccessOpValueType::I64,
            _ => panic!("Unexpected bitwidth"),
        }
    }
}

impl DisplayWithContext for LoadOp {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} {}",
            self.get_opid().with_ctx(ctx),
            self.get_value_type(ctx)
        )
    }
}

impl Verify for LoadOp {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        let op = &*self.get_operation().deref(ctx);
        if op.get_opid() != Self::get_opid_static() {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect OpId".to_string(),
            });
        }
        if op.get_num_results() != 0 || op.get_num_operands() != 0 {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect number of results or operands".to_string(),
            });
        }
        Ok(())
    }
}

declare_op!(
    /// Branch op. Transfer control to the end of outer block relative_depth levels up.
    ///
    BrOp,
    "br",
    "wasm"
);

impl BrOp {
    const ATTR_KEY_RELATIVE_DEPTH: &str = "br.relative_depth";

    /// Get the function index
    pub fn get_relative_depth(&self, ctx: &Context) -> RelativeDepth {
        let op = self.get_operation().deref(ctx);
        #[allow(clippy::expect_used)]
        let attr = op
            .attributes
            .get(Self::ATTR_KEY_RELATIVE_DEPTH)
            .expect("no attribute found");
        #[allow(clippy::expect_used)]
        let attr_val = apint_to_i32(
            attr.downcast_ref::<IntegerAttr>()
                .expect("expected IntegerAttr")
                .clone()
                .into(),
        ) as u32;
        attr_val.into()
    }

    /// Create a new [BrOp]. The underlying [Operation] is not linked to a
    /// [BasicBlock](crate::basic_block::BasicBlock).
    pub fn new_unlinked(ctx: &mut Context, relative_depth: RelativeDepth) -> BrOp {
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 0);
        let attr = u32_attr(ctx, relative_depth.into());
        op.deref_mut(ctx)
            .attributes
            .insert(Self::ATTR_KEY_RELATIVE_DEPTH, attr);
        BrOp { op }
    }
}

impl DisplayWithContext for BrOp {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} {}",
            self.get_opid().with_ctx(ctx),
            self.get_relative_depth(ctx)
        )
    }
}

impl Verify for BrOp {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        let op = &*self.get_operation().deref(ctx);
        if op.get_opid() != Self::get_opid_static() {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect OpId".to_string(),
            });
        }
        if op.get_num_results() != 0 || op.get_num_operands() != 0 {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect number of results or operands".to_string(),
            });
        }
        Ok(())
    }
}

declare_op!(
    /// Conditional branch op.
    /// Pop the value from the stack and if its true - transfers control to the end of outer block relative_depth levels up.
    ///
    BrIfOp,
    "br_if",
    "wasm"
);

impl BrIfOp {
    const ATTR_KEY_RELATIVE_DEPTH: &str = "br_if.relative_depth";

    /// Get the function index
    pub fn get_relative_depth(&self, ctx: &Context) -> RelativeDepth {
        let op = self.get_operation().deref(ctx);
        #[allow(clippy::expect_used)]
        let attr = op
            .attributes
            .get(Self::ATTR_KEY_RELATIVE_DEPTH)
            .expect("no attribute found");
        #[allow(clippy::expect_used)]
        let attr_val = apint_to_i32(
            attr.downcast_ref::<IntegerAttr>()
                .expect("expected IntegerAttr")
                .clone()
                .into(),
        ) as u32;
        attr_val.into()
    }

    /// Create a new [BrIfOp]. The underlying [Operation] is not linked to a
    /// [BasicBlock](crate::basic_block::BasicBlock).
    pub fn new_unlinked(ctx: &mut Context, relative_depth: RelativeDepth) -> BrIfOp {
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 0);
        let attr = u32_attr(ctx, relative_depth.into());
        op.deref_mut(ctx)
            .attributes
            .insert(Self::ATTR_KEY_RELATIVE_DEPTH, attr);
        BrIfOp { op }
    }
}

impl DisplayWithContext for BrIfOp {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} {}",
            self.get_opid().with_ctx(ctx),
            self.get_relative_depth(ctx)
        )
    }
}

impl Verify for BrIfOp {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        let op = &*self.get_operation().deref(ctx);
        if op.get_opid() != Self::get_opid_static() {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect OpId".to_string(),
            });
        }
        if op.get_num_results() != 0 || op.get_num_operands() != 0 {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect number of results or operands".to_string(),
            });
        }
        Ok(())
    }
}

declare_op!(
    /// Pops the i32 value from the stack and if its zero pushes 1 otherwise pushes 0 to the stack.
    ///
    I32EqzOp,
    "i32.eqz",
    "wasm"
);

impl I32EqzOp {
    /// Create a new [I32Eqz]. The underlying [Operation] is not linked to a
    /// [BasicBlock](crate::basic_block::BasicBlock).
    pub fn new_unlinked(ctx: &mut Context) -> ConstantOp {
        let op = Operation::new(ctx, Self::get_opid_static(), vec![], vec![], 0);
        ConstantOp { op }
    }
}

impl DisplayWithContext for I32EqzOp {
    fn fmt(&self, ctx: &Context, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.get_opid().with_ctx(ctx),)
    }
}

impl Verify for I32EqzOp {
    fn verify(&self, ctx: &Context) -> Result<(), CompilerError> {
        let op = &*self.get_operation().deref(ctx);
        if op.get_opid() != Self::get_opid_static() {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect OpId".to_string(),
            });
        }
        if op.get_num_results() != 0 || op.get_num_operands() != 0 {
            return Err(CompilerError::VerificationError {
                msg: "Incorrect number of results or operands".to_string(),
            });
        }
        Ok(())
    }
}

pub(crate) fn register(ctx: &mut Context, dialect: &mut Dialect) {
    ModuleOp::register(ctx, dialect);
    ConstantOp::register(ctx, dialect);
    FuncOp::register(ctx, dialect);
    AddOp::register(ctx, dialect);
    CallOp::register(ctx, dialect);
    ReturnOp::register(ctx, dialect);
    BlockOp::register(ctx, dialect);
    LoopOp::register(ctx, dialect);
    LocalGetOp::register(ctx, dialect);
    LocalSetOp::register(ctx, dialect);
    LocalTeeOp::register(ctx, dialect);
    GlobalSetOp::register(ctx, dialect);
    GlobalGetOp::register(ctx, dialect);
    StoreOp::register(ctx, dialect);
    LoadOp::register(ctx, dialect);
    BrOp::register(ctx, dialect);
    BrIfOp::register(ctx, dialect);
    I32EqzOp::register(ctx, dialect);
}
