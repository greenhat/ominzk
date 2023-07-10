use ozk_valida_dialect as valida;
use ozk_wasm_dialect as wasm;
use pliron::context::Context;
use pliron::context::Ptr;
use pliron::dialect_conversion::apply_partial_conversion;
use pliron::dialect_conversion::ConversionTarget;
use pliron::dialects::builtin::op_interfaces::SymbolOpInterface;
use pliron::op::Op;
use pliron::operation::Operation;
use pliron::pass::Pass;
use pliron::pattern_match::PatternRewriter;
use pliron::pattern_match::RewritePattern;
use pliron::rewrite::RewritePatternSet;

#[derive(Default)]
pub struct WasmToValidaFuncLoweringPass;

impl Pass for WasmToValidaFuncLoweringPass {
    fn name(&self) -> &str {
        "WasmToValidaFuncLoweringPass"
    }

    fn run_on_operation(&self, ctx: &mut Context, op: Ptr<Operation>) -> Result<(), anyhow::Error> {
        let target = ConversionTarget::default();
        // TODO: set illegal ops
        let mut patterns = RewritePatternSet::default();
        patterns.add(Box::<ReturnOpLowering>::default());
        patterns.add(Box::<FuncOpLowering>::default());
        apply_partial_conversion(ctx, op, target, patterns)?;
        Ok(())
    }
}

#[derive(Default)]
pub struct ReturnOpLowering {}

impl RewritePattern for ReturnOpLowering {
    fn name(&self) -> String {
        "ReturnOpLowering".to_string()
    }

    fn match_op(&self, ctx: &Context, op: Ptr<Operation>) -> Result<bool, anyhow::Error> {
        Ok(op
            .deref(ctx)
            .get_op(ctx)
            .downcast_ref::<wasm::ops::ReturnOp>()
            .is_some())
    }

    #[allow(clippy::unwrap_used)]
    fn rewrite(
        &self,
        ctx: &mut Context,
        op: Ptr<Operation>,
        rewriter: &mut dyn PatternRewriter,
    ) -> Result<(), anyhow::Error> {
        let opop = &op.deref(ctx).get_op(ctx);
        #[allow(clippy::panic)]
        let Some(return_op) = opop.downcast_ref::<wasm::ops::ReturnOp>() else {
            panic!("expected ReturnOp");
        };
        let ret_op = valida::ops::JalvOp::new_return_pseudo_op(ctx);
        rewriter.replace_op_with(ctx, return_op.get_operation(), ret_op.get_operation())?;
        Ok(())
    }
}

#[derive(Default)]
pub struct FuncOpLowering {}

impl RewritePattern for FuncOpLowering {
    fn name(&self) -> String {
        "FuncOpLowering".to_string()
    }

    fn match_op(&self, ctx: &Context, op: Ptr<Operation>) -> Result<bool, anyhow::Error> {
        Ok(op
            .deref(ctx)
            .get_op(ctx)
            .downcast_ref::<wasm::ops::FuncOp>()
            .is_some())
    }

    #[allow(clippy::unwrap_used)]
    fn rewrite(
        &self,
        ctx: &mut Context,
        op: Ptr<Operation>,
        rewriter: &mut dyn PatternRewriter,
    ) -> Result<(), anyhow::Error> {
        let opop = &op.deref(ctx).get_op(ctx);
        #[allow(clippy::panic)]
        let Some(wasm_func_op) = opop.downcast_ref::<wasm::ops::FuncOp>() else {
            panic!("expected FuncOp");
        };
        let func_op = valida::ops::FuncOp::new_unlinked(ctx, wasm_func_op.get_symbol_name(ctx));
        for op in wasm_func_op.op_iter(ctx) {
            op.unlink(ctx);
            op.insert_at_back(func_op.get_entry_block(ctx), ctx);
        }
        rewriter.replace_op_with(ctx, wasm_func_op.get_operation(), func_op.get_operation())?;
        Ok(())
    }
}
