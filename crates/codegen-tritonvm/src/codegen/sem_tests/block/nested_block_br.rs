use expect_test::expect;

use crate::codegen::sem_tests::check_wat;

#[test]
fn test_nested_block_br() {
    let input = vec![];
    let secret_input = vec![];
    let expected_output = vec![3, 8, 7];
    check_wat(
        r#"
(module 
    (type (;0;) (func (result i64)))
    (type (;1;) (func (param i64)))
    (type (;2;) (func))
    (import "env" "c2zk_stdlib_pub_input" (func $c2zk_stdlib_pub_input (;0;) (type 0)))
    (import "env" "c2zk_stdlib_pub_output" (func $c2zk_stdlib_pub_output (;1;) (type 1)))
    (import "env" "c2zk_stdlib_secret_input" (func $c2zk_stdlib_secret_input (;2;) (type 0)))
    (export "main" (func $main))
    (start $main)
    (func $main 
        block 
            i64.const 3
            call $c2zk_stdlib_pub_output
            block 
                i64.const 8
                call $c2zk_stdlib_pub_output
                br 1
                i64.const 11
                call $c2zk_stdlib_pub_output
            end
            i64.const 9
            call $c2zk_stdlib_pub_output
        end
        i64.const 7
        call $c2zk_stdlib_pub_output
        return)
)"#,
        input,
        secret_input,
        expected_output,
        expect![[r#"
            call main
            halt
            globals_set:
            push -4
            mul
            push 00000000002147482623
            add
            swap 1
            write_mem
            pop
            return
            c2zk_stdlib_pub_output:
            push 0
            call globals_get
            push -4
            add
            dup 0
            swap 2
            write_mem
            pop
            push 0
            call globals_set
            push 0
            call globals_get
            read_mem
            swap 1
            pop
            write_io
            push 0
            call globals_get
            push 4
            add
            push 0
            call globals_set
            return
            globals_get:
            push -4
            mul
            push 00000000002147482623
            add
            read_mem
            swap 1
            pop
            return
            main:
            call init_mem_for_locals
            call main_l0_b0
            push 7
            call c2zk_stdlib_pub_output
            return
            return
            init_mem_for_locals:
            push 00000000002147483647
            push 0
            call globals_set
            return
            main_l0_b0:
            push 3
            call c2zk_stdlib_pub_output
            call main_l0_b0_l1_b0
            call next_br_propagation
            skiz
            return
            push 9
            call c2zk_stdlib_pub_output
            return
            main_l0_b0_l1_b0:
            push 8
            call c2zk_stdlib_pub_output
            push 2
            push 1
            call globals_set
            return
            push 11
            call c2zk_stdlib_pub_output
            return
            next_br_propagation:
            push 1
            call globals_get
            dup 0
            push 0
            eq
            skiz
            return
            push -1
            add
            dup 0
            push 1
            call globals_set
            return"#]],
    );
}
