use expect_test::expect;

use crate::codegen::sem_tests::check_wasm;

#[test]
fn test_fib() {
    let input = vec![25];
    let secret_input = vec![];
    let expected_output = vec![75025];
    let native_output = c2zk_rust_wasm_tests_helper::wrap_main_with_io(
        &c2zk_rust_wasm_tests_bundle1::fib::fib_seq,
    )(input.clone(), secret_input.clone());
    assert_eq!(native_output, expected_output);
    let wasm_bytes = c2zk_rust_wasm_tests_helper::compile_rust_wasm_tests_bundle1_bin("fib");
    check_wasm(
        &wasm_bytes,
        input,
        secret_input,
        expected_output,
        expect![[r#"
            (module
              (type (;0;) (func (result i64)))
              (type (;1;) (func (param i64)))
              (type (;2;) (func))
              (import "env" "c2zk_stdlib_pub_input" (func $c2zk_stdlib_pub_input (;0;) (type 0)))
              (import "env" "c2zk_stdlib_pub_output" (func $c2zk_stdlib_pub_output (;1;) (type 1)))
              (func $__main (;2;) (type 2)
                call $_ZN28c2zk_rust_wasm_tests_bundle13fib7fib_seq17h65c3299d95c55f5bE
              )
              (func $_ZN28c2zk_rust_wasm_tests_bundle13fib7fib_seq17h65c3299d95c55f5bE (;3;) (type 2)
                (local i64 i64 i64 i64)
                block ;; label = @1
                  call $_ZN11c2zk_stdlib9pub_input17h060bd075f37c6b24E
                  local.tee 0
                  i64.eqz
                  i32.eqz
                  br_if 0 (;@1;)
                  i64.const 0
                  call $_ZN11c2zk_stdlib10pub_output17hc744a302b8a83f64E
                  return
                end
                local.get 0
                i64.const 7
                i64.and
                local.set 1
                block ;; label = @1
                  block ;; label = @2
                    local.get 0
                    i64.const -1
                    i64.add
                    i64.const 7
                    i64.ge_u
                    br_if 0 (;@2;)
                    i64.const 1
                    local.set 0
                    i64.const 0
                    local.set 2
                    br 1 (;@1;)
                  end
                  local.get 0
                  i64.const -8
                  i64.and
                  local.set 3
                  i64.const 1
                  local.set 0
                  i64.const 0
                  local.set 2
                  loop ;; label = @2
                    local.get 0
                    local.get 2
                    i64.add
                    local.tee 2
                    local.get 0
                    i64.add
                    local.tee 0
                    local.get 2
                    i64.add
                    local.tee 2
                    local.get 0
                    i64.add
                    local.tee 0
                    local.get 2
                    i64.add
                    local.tee 2
                    local.get 0
                    i64.add
                    local.tee 0
                    local.get 2
                    i64.add
                    local.tee 2
                    local.get 0
                    i64.add
                    local.set 0
                    local.get 3
                    i64.const -8
                    i64.add
                    local.tee 3
                    i64.eqz
                    i32.eqz
                    br_if 0 (;@2;)
                  end
                end
                block ;; label = @1
                  local.get 1
                  i64.eqz
                  br_if 0 (;@1;)
                  local.get 2
                  local.set 3
                  loop ;; label = @2
                    local.get 0
                    local.tee 2
                    local.get 3
                    i64.add
                    local.set 0
                    local.get 2
                    local.set 3
                    local.get 1
                    i64.const -1
                    i64.add
                    local.tee 1
                    i64.const 0
                    i64.ne
                    br_if 0 (;@2;)
                  end
                end
                local.get 2
                call $_ZN11c2zk_stdlib10pub_output17hc744a302b8a83f64E
              )
              (func $_ZN11c2zk_stdlib9pub_input17h060bd075f37c6b24E (;4;) (type 0) (result i64)
                call $c2zk_stdlib_pub_input
              )
              (func $_ZN11c2zk_stdlib10pub_output17hc744a302b8a83f64E (;5;) (type 1) (param i64)
                local.get 0
                call $c2zk_stdlib_pub_output
              )
              (memory (;0;) 16)
              (global $__stack_pointer (;0;) (mut i32) i32.const 1048576)
              (global (;1;) i32 i32.const 1048576)
              (global (;2;) i32 i32.const 1048576)
              (export "memory" (memory 0))
              (export "__main" (func $__main))
              (export "__data_end" (global 1))
              (export "__heap_base" (global 2))
            )"#]],
        expect![[r#"
            call f
            halt
            c2zk_stdlib_pub_input:
            read_io
            return
            c2zk_stdlib_pub_output:
            write_io
            return
            f:
            call f
            return
            f:
            call f_l0_b0
            push -1 // Begin: propagate Br* in block (0)
            add
            skiz
            return // End: propagate Br* in block
            push 7
            nop
            nop
            call f_l0_b1
            push -1 // Begin: propagate Br* in block (0)
            add
            skiz
            return // End: propagate Br* in block
            call f_l0_b2
            push -1 // Begin: propagate Br* in block (0)
            add
            skiz
            return // End: propagate Br* in block
            call f
            return
            f:
            call c2zk_stdlib_pub_input
            return
            f:
            call c2zk_stdlib_pub_output
            return
            f_l0_b0:
            call f
            nop
            nop
            nop
            push 0
            call f
            return
            push 1 // Begin: extracted func prologue (0)
            return // End: extracted func prologue
            f_l0_b1:
            call f_l0_b1_l1_b0
            push -1 // Begin: propagate Br* in block (1)
            add
            skiz
            return // End: propagate Br* in block
            push -8
            nop
            nop
            push 1
            nop
            push 0
            nop
            call f_l0_b1_l1_b1
            push -1
            add // Begin: propagate Br* in loop (1)
            skiz
            return
            recurse // End: propagate Br* in loop
            push 1 // Begin: extracted func prologue (0)
            return // End: extracted func prologue
            f_l0_b1_l1_b0:
            push -1
            add
            push 7
            nop
            push 1
            nop
            push 0
            nop
            push 1 // Begin: extracted func prologue (1)
            return // End: extracted func prologue
            f_l0_b1_l1_b1:
            add
            nop
            add
            nop
            add
            nop
            add
            nop
            add
            nop
            add
            nop
            add
            nop
            add
            nop
            push -8
            add
            nop
            nop
            nop
            push 1 // Begin: extracted func prologue (1)
            return // End: extracted func prologue
            f_l0_b2:
            nop
            nop
            call f_l0_b2_l1_b0
            push -1
            add // Begin: propagate Br* in loop (1)
            skiz
            return
            recurse // End: propagate Br* in loop
            push 1 // Begin: extracted func prologue (0)
            return // End: extracted func prologue
            f_l0_b2_l1_b0:
            push 1
            swap1
            skiz
            return
            pop
            nop
            add
            nop
            nop
            push -1
            add
            nop
            push 0
            nop
            push 1 // Begin: extracted func prologue (1)
            return // End: extracted func prologue"#]],
    )
}
