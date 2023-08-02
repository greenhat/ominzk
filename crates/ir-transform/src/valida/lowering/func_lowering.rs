use anyhow::Ok;
use ozk_ozk_dialect as ozk;
use ozk_valida_dialect as valida;
use ozk_wasm_dialect as wasm;
use pliron::context::Context;
use pliron::context::Ptr;
use pliron::dialect_conversion::apply_partial_conversion;
use pliron::dialect_conversion::ConversionTarget;
use pliron::dialects::builtin::op_interfaces::SymbolOpInterface;
use pliron::op::Op;
use pliron::operation::Operation;
use pliron::operation::WalkOrder;
use pliron::operation::WalkResult;
use pliron::pass::Pass;
use pliron::pattern_match::PatternRewriter;
use pliron::pattern_match::RewritePattern;
use pliron::rewrite::RewritePatternSet;
use valida::types::Operands;
use wasm::op_interfaces::TrackedStackDepth;
use wasm::ops::LocalGetOp;
use wasm::ops::LocalSetOp;
use wasm::ops::ReturnOp;

use crate::valida::fp_from_wasm_stack;

#[derive(Default)]
pub struct WasmToValidaFuncLoweringPass;

impl Pass for WasmToValidaFuncLoweringPass {
    fn run_on_operation(&self, ctx: &mut Context, op: Ptr<Operation>) -> Result<(), anyhow::Error> {
        let target = ConversionTarget::default();
        // TODO: set illegal ops
        let mut patterns = RewritePatternSet::default();
        patterns.add(Box::<FuncOpLowering>::default());
        apply_partial_conversion(ctx, op, target, patterns)?;
        Ok(())
    }
}

#[derive(Default)]
pub struct FuncOpLowering {}

impl RewritePattern for FuncOpLowering {
    fn match_and_rewrite(
        &self,
        ctx: &mut Context,
        op: Ptr<Operation>,
        rewriter: &mut dyn PatternRewriter,
    ) -> Result<bool, anyhow::Error> {
        let opop = &op.deref(ctx).get_op(ctx);
        let Some(wasm_func_op) = opop.downcast_ref::<wasm::ops::FuncOp>() else {
            return Ok(false);
        };

        convert_func_arg_and_locals(wasm_func_op, ctx, rewriter)?;
        convert_return_ops(wasm_func_op, ctx, rewriter)?;
        convert_call_ops(wasm_func_op, ctx, rewriter)?;

        let func_op = valida::ops::FuncOp::new_unlinked(ctx, wasm_func_op.get_symbol_name(ctx));
        for op in wasm_func_op.op_iter(ctx) {
            op.unlink(ctx);
            op.insert_at_back(func_op.get_entry_block(ctx), ctx);
        }
        rewriter.replace_op_with(ctx, wasm_func_op.get_operation(), func_op.get_operation())?;
        Ok(true)
    }
}

fn convert_call_ops(
    wasm_func_op: &wasm::ops::FuncOp,
    ctx: &mut Context,
    rewriter: &mut dyn PatternRewriter,
) -> Result<(), anyhow::Error> {
    let mut call_ops = Vec::new();
    wasm_func_op.get_operation().walk_only::<ozk::ops::CallOp>(
        ctx,
        WalkOrder::PostOrder,
        &mut |op| {
            call_ops.push(*op);
            WalkResult::Advance
        },
    );
    for call_op in call_ops {
        let wasm_stack_depth_before_op = call_op.get_stack_depth(ctx);
        let fp_last_stack_height: i32 = fp_from_wasm_stack(wasm_stack_depth_before_op).into();
        // 12 is the stack frame size (return value + return fp + return address)
        // Call convention for wasm:
        // arg1
        // arg2
        // Return value (if no args, otherwise in arg1)
        // Return FP
        // Return address (current FP for callee)
        // Local 1
        // ...
        // Local n
        let fp_for_return_address = fp_last_stack_height - 12;
        let return_fp_value = fp_for_return_address + 4;
        let fp_to_restore_after_call = fp_last_stack_height - 12;
        let imm32_op = valida::ops::Imm32Op::new_unlinked(
            ctx,
            Operands::from_i32(return_fp_value, 0, 0, 0, -fp_to_restore_after_call),
        );
        rewriter.set_insertion_point(call_op.get_operation());
        rewriter.insert_before(ctx, imm32_op.get_operation())?;
        let jalsym_op = valida::ops::JalSymOp::new(
            ctx,
            fp_for_return_address,
            fp_for_return_address,
            call_op.get_func_sym(ctx),
        );
        rewriter.replace_op_with(ctx, call_op.get_operation(), jalsym_op.get_operation())?;
    }
    Ok(())
}

fn convert_return_ops(
    wasm_func_op: &wasm::ops::FuncOp,
    ctx: &mut Context,
    rewriter: &mut dyn PatternRewriter,
) -> Result<(), anyhow::Error> {
    let mut return_ops = Vec::new();
    wasm_func_op
        .get_operation()
        .walk_only::<ReturnOp>(ctx, WalkOrder::PostOrder, &mut |op| {
            return_ops.push(*op);
            WalkResult::Advance
        });
    for return_op in return_ops {
        // TODO: check func signature if there is a return value (after I/O is implemented)
        // if wasm_func_op.get_type_typed(ctx).get_results().len() == 1 {
        let wasm_stack_depth_before_op = return_op.get_stack_depth(ctx);
        let last_stack_value_fp_offset = fp_from_wasm_stack(wasm_stack_depth_before_op);
        // let return_value_fp_offset = 4;
        let func_arg_num: i32 = wasm_func_op.get_type(ctx).get_inputs().len() as i32;
        let return_value_fp_offset = 8 + func_arg_num * 4; // Arg 1 cell, or new cell after
        let sw_op = valida::ops::SwOp::new(
            ctx,
            return_value_fp_offset,
            last_stack_value_fp_offset.into(),
        );
        rewriter.set_insertion_point(return_op.get_operation());
        rewriter.insert_before(ctx, sw_op.get_operation())?;
        // } else {
        //     todo!("wasm.func -> valida: multiple return values are not supported yet");
        // }
        // let c = 12 - (-func_arg_num + wasm_func_op.get_type(ctx).get_results().len() as i32) * 4;
        let ret_op = valida::ops::JalvOp::new_return_pseudo_op(ctx);
        rewriter.replace_op_with(ctx, return_op.get_operation(), ret_op.get_operation())?;
    }
    Ok(())
}

fn convert_func_arg_and_locals(
    wasm_func_op: &wasm::ops::FuncOp,
    ctx: &mut Context,
    rewriter: &mut dyn PatternRewriter,
) -> Result<(), anyhow::Error> {
    let mut local_get_ops = Vec::new();
    wasm_func_op
        .get_operation()
        .walk_only::<LocalGetOp>(ctx, WalkOrder::PostOrder, &mut |op| {
            local_get_ops.push(*op);
            WalkResult::Advance
        });
    let fp_func_first_arg: i32 = 12;
    for local_get_op in local_get_ops {
        let zero_based_index: i32 = u32::from(local_get_op.get_index(ctx)) as i32;
        let wasm_stack_depth_before_op = local_get_op.get_stack_depth(ctx);
        let to_fp: i32 = fp_from_wasm_stack(wasm_stack_depth_before_op.next()).into();
        let from_fp: i32 =
            if zero_based_index < wasm_func_op.get_type(ctx).get_inputs().len() as i32 {
                // this is function paramter
                fp_func_first_arg + zero_based_index * 4
            } else {
                // this is a local variable
                -(zero_based_index + 1) * 4
            };
        let sw_op = valida::ops::SwOp::new(ctx, to_fp, from_fp);
        rewriter.replace_op_with(ctx, local_get_op.get_operation(), sw_op.get_operation())?;
    }

    let mut local_set_ops = Vec::new();
    wasm_func_op
        .get_operation()
        .walk_only::<LocalSetOp>(ctx, WalkOrder::PostOrder, &mut |op| {
            local_set_ops.push(*op);
            WalkResult::Advance
        });
    for local_set_op in local_set_ops {
        let zero_based_index: i32 = u32::from(local_set_op.get_index(ctx)) as i32;
        let wasm_stack_depth_before_op = local_set_op.get_stack_depth(ctx);
        let from_fp: i32 = fp_from_wasm_stack(wasm_stack_depth_before_op).into();
        let to_fp: i32 = -(zero_based_index + 1) * 4;
        let sw_op = valida::ops::SwOp::new(ctx, to_fp, from_fp);
        rewriter.replace_op_with(ctx, local_set_op.get_operation(), sw_op.get_operation())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use expect_test::expect;

    use crate::tests_util::check_wasm_valida_passes;
    use crate::valida::lowering::arith_op_lowering::WasmToValidaArithLoweringPass;
    use crate::wasm::track_stack_depth::WasmTrackStackDepthPass;

    use super::*;

    #[test]
    fn func_op_lowering() {
        check_wasm_valida_passes(
            vec![
                Box::new(WasmTrackStackDepthPass::new_reserve_space_for_locals()),
                Box::<WasmToValidaFuncLoweringPass>::default(),
            ],
            r#"
(module
    (start $main)
    (func $add (param i32 i32) (result i32)
        get_local 0
        get_local 1
        i32.add
        return)
    (func $main
        i32.const 3
        i32.const 4
        call $add
        return)
)
        "#,
            expect![[r#"
                wasm.module {
                  block_2_0():
                    valida.func @add {
                      entry():
                        valida.sw 0 -4(fp) 12(fp) 0 0
                        valida.sw 0 -8(fp) 16(fp) 0 0
                        wasm.add
                        valida.sw 0 16(fp) -4(fp) 0 0
                        valida.jalv -4(fp) 0(fp) 4(fp) 0 0
                    }
                    valida.func @main {
                      entry():
                        wasm.const 0x3: si32
                        wasm.const 0x4: si32
                        wasm.call 0
                        valida.sw 0 8(fp) -8(fp) 0 0
                        valida.jalv -4(fp) 0(fp) 4(fp) 0 0
                    }
                }"#]],
        )
    }

    #[test]
    fn smoke_local_var_access() {
        check_wasm_valida_passes(
            vec![
                Box::new(WasmTrackStackDepthPass::new_reserve_space_for_locals()),
                Box::<WasmToValidaArithLoweringPass>::default(),
                Box::<WasmToValidaFuncLoweringPass>::default(),
            ],
            r#"
(module
    (start $main)
    (func $main
        (local i32)
        i32.const 3
        i32.const 7
        local.set 0
        local.get 0
        return)
)
        "#,
            expect![[r#"
                wasm.module {
                  block_1_0():
                    valida.func @main {
                      entry():
                        valida.imm32 -8(fp) 0 0 0 3
                        valida.imm32 -12(fp) 0 0 0 7
                        valida.sw 0 -4(fp) -12(fp) 0 0
                        valida.sw 0 -12(fp) -4(fp) 0 0
                        valida.sw 0 8(fp) -12(fp) 0 0
                        valida.jalv -4(fp) 0(fp) 4(fp) 0 0
                    }
                }"#]],
        )
    }
}
