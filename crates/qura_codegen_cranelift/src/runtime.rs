use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{
    types, AbiParam, BlockArg, Function, InstBuilder, MemFlags, StackSlotData, StackSlotKind, UserFuncName,
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{DataDescription, FuncId, Linkage, Module as _};
use cranelift_object::ObjectModule;
use crate::codegen::CodegenError;

pub struct Runtime {
    pub print_str_id: FuncId,
    pub print_i64_id: FuncId,
    pub print_u64_id: FuncId,
    pub print_bool_id: FuncId,
}

/// Initializes and emits minimal Windows-compatible native output runtime functions
/// directly into the current Cranelift object module.
pub fn init_runtime(
    cl_module: &mut ObjectModule,
    ctx: &mut cranelift_codegen::Context,
    fn_builder_ctx: &mut FunctionBuilderContext,
) -> Result<Runtime, CodegenError> {
    // 1. Declare and define static runtime constants in the data section
    let mut crlf_desc = DataDescription::new();
    crlf_desc.define(b"\r\n\0".to_vec().into_boxed_slice());
    let crlf_id = cl_module.declare_data("__qura_rt_crlf", Linkage::Local, false, false)?;
    cl_module.define_data(crlf_id, &crlf_desc)?;

    let mut true_desc = DataDescription::new();
    true_desc.define(b"true\0".to_vec().into_boxed_slice());
    let true_id = cl_module.declare_data("__qura_rt_true", Linkage::Local, false, false)?;
    cl_module.define_data(true_id, &true_desc)?;

    let mut false_desc = DataDescription::new();
    false_desc.define(b"false\0".to_vec().into_boxed_slice());
    let false_id = cl_module.declare_data("__qura_rt_false", Linkage::Local, false, false)?;
    cl_module.define_data(false_id, &false_desc)?;

    let mut minus_desc = DataDescription::new();
    minus_desc.define(b"-\0".to_vec().into_boxed_slice());
    let minus_id = cl_module.declare_data("__qura_rt_minus", Linkage::Local, false, false)?;
    cl_module.define_data(minus_id, &minus_desc)?;

    // 2. Declare Windows OS kernel32 imports
    let mut sig_get_std = cl_module.make_signature();
    sig_get_std.params.push(AbiParam::new(types::I32)); // DWORD nStdHandle
    sig_get_std.returns.push(AbiParam::new(types::I64)); // HANDLE
    let get_std_handle_id = cl_module.declare_function("GetStdHandle", Linkage::Import, &sig_get_std)?;

    let mut sig_write = cl_module.make_signature();
    sig_write.params.push(AbiParam::new(types::I64)); // HANDLE hFile
    sig_write.params.push(AbiParam::new(types::I64)); // LPCVOID lpBuffer
    sig_write.params.push(AbiParam::new(types::I32)); // DWORD nNumberOfBytesToWrite
    sig_write.params.push(AbiParam::new(types::I64)); // LPDWORD lpNumberOfBytesWritten
    sig_write.params.push(AbiParam::new(types::I64)); // LPOVERLAPPED lpOverlapped
    sig_write.returns.push(AbiParam::new(types::I32)); // BOOL
    let write_file_id = cl_module.declare_function("WriteFile", Linkage::Import, &sig_write)?;

    // 3. Declare internal runtime functions
    let mut sig_write_raw = cl_module.make_signature();
    sig_write_raw.params.push(AbiParam::new(types::I64)); // buf
    sig_write_raw.params.push(AbiParam::new(types::I32)); // len
    let write_raw_id = cl_module.declare_function("__qura_write_raw", Linkage::Local, &sig_write_raw)?;

    let mut sig_print_str = cl_module.make_signature();
    sig_print_str.params.push(AbiParam::new(types::I64)); // s
    sig_print_str.params.push(AbiParam::new(types::I32)); // newline
    let print_str_id = cl_module.declare_function("__qura_print_str", Linkage::Local, &sig_print_str)?;

    let mut sig_print_u64 = cl_module.make_signature();
    sig_print_u64.params.push(AbiParam::new(types::I64)); // val
    sig_print_u64.params.push(AbiParam::new(types::I32)); // newline
    let print_u64_id = cl_module.declare_function("__qura_print_u64", Linkage::Local, &sig_print_u64)?;

    let mut sig_print_i64 = cl_module.make_signature();
    sig_print_i64.params.push(AbiParam::new(types::I64)); // val
    sig_print_i64.params.push(AbiParam::new(types::I32)); // newline
    let print_i64_id = cl_module.declare_function("__qura_print_i64", Linkage::Local, &sig_print_i64)?;

    let mut sig_print_bool = cl_module.make_signature();
    sig_print_bool.params.push(AbiParam::new(types::I8)); // val
    sig_print_bool.params.push(AbiParam::new(types::I32)); // newline
    let print_bool_id = cl_module.declare_function("__qura_print_bool", Linkage::Local, &sig_print_bool)?;

    // 4. Define __qura_write_raw(buf: i64, len: i32)
    {
        ctx.func = Function::with_name_signature(UserFuncName::user(0, write_raw_id.as_u32()), sig_write_raw);
        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);
        let entry = builder.create_block();
        let write_block = builder.create_block();
        let ret_block = builder.create_block();

        builder.switch_to_block(entry);
        builder.append_block_params_for_function_params(entry);
        let buf = builder.block_params(entry)[0];
        let len = builder.block_params(entry)[1];

        let zero32 = builder.ins().iconst(types::I32, 0);
        let is_pos = builder.ins().icmp(IntCC::SignedGreaterThan, len, zero32);
        builder.ins().brif(is_pos, write_block, &[], ret_block, &[]);

        builder.switch_to_block(write_block);
        let std_handle_arg = builder.ins().iconst(types::I32, -11); // STD_OUTPUT_HANDLE
        let local_get_std = cl_module.declare_func_in_func(get_std_handle_id, &mut builder.func);
        let call_res = builder.ins().call(local_get_std, &[std_handle_arg]);
        let h_stdout = builder.inst_results(call_res)[0];

        let written_slot = builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 0));
        let written_ptr = builder.ins().stack_addr(types::I64, written_slot, 0);
        let null_val = builder.ins().iconst(types::I64, 0);

        let local_write_file = cl_module.declare_func_in_func(write_file_id, &mut builder.func);
        builder.ins().call(local_write_file, &[h_stdout, buf, len, written_ptr, null_val]);
        builder.ins().jump(ret_block, &[]);

        builder.switch_to_block(ret_block);
        builder.ins().return_(&[]);

        builder.seal_all_blocks();
        builder.finalize();
        cl_module.define_function(write_raw_id, ctx)?;
        cl_module.clear_context(ctx);
    }

    // 5. Define __qura_print_str(s: i64, newline: i32)
    {
        ctx.func = Function::with_name_signature(UserFuncName::user(0, print_str_id.as_u32()), sig_print_str);
        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);
        let entry = builder.create_block();
        let loop_block = builder.create_block();
        let next_char_block = builder.create_block();
        let done_len_block = builder.create_block();
        let nl_block = builder.create_block();
        let ret_block = builder.create_block();

        builder.switch_to_block(entry);
        builder.append_block_params_for_function_params(entry);
        let s = builder.block_params(entry)[0];
        let newline = builder.block_params(entry)[1];
        builder.ins().jump(loop_block, &[BlockArg::from(s)]);

        builder.switch_to_block(loop_block);
        let curr_ptr = builder.append_block_param(loop_block, types::I64);
        let byte = builder.ins().load(types::I8, MemFlags::new(), curr_ptr, 0);
        let is_zero = builder.ins().icmp_imm(IntCC::Equal, byte, 0);
        builder.ins().brif(is_zero, done_len_block, &[BlockArg::from(curr_ptr)], next_char_block, &[]);

        builder.switch_to_block(next_char_block);
        let next_ptr = builder.ins().iadd_imm(curr_ptr, 1);
        builder.ins().jump(loop_block, &[BlockArg::from(next_ptr)]);

        builder.switch_to_block(done_len_block);
        let end_ptr = builder.append_block_param(done_len_block, types::I64);
        let len64 = builder.ins().isub(end_ptr, s);
        let len32 = builder.ins().ireduce(types::I32, len64);
        let local_write = cl_module.declare_func_in_func(write_raw_id, &mut builder.func);
        builder.ins().call(local_write, &[s, len32]);
        let is_nl = builder.ins().icmp_imm(IntCC::NotEqual, newline, 0);
        builder.ins().brif(is_nl, nl_block, &[], ret_block, &[]);

        builder.switch_to_block(nl_block);
        let local_crlf = cl_module.declare_data_in_func(crlf_id, &mut builder.func);
        let crlf_ptr = builder.ins().symbol_value(types::I64, local_crlf);
        let two32 = builder.ins().iconst(types::I32, 2);
        builder.ins().call(local_write, &[crlf_ptr, two32]);
        builder.ins().jump(ret_block, &[]);

        builder.switch_to_block(ret_block);
        builder.ins().return_(&[]);

        builder.seal_all_blocks();
        builder.finalize();
        cl_module.define_function(print_str_id, ctx)?;
        cl_module.clear_context(ctx);
    }

    // 6. Define __qura_print_bool(val: i8, newline: i32)
    {
        ctx.func = Function::with_name_signature(UserFuncName::user(0, print_bool_id.as_u32()), sig_print_bool);
        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);
        let entry = builder.create_block();
        let true_block = builder.create_block();
        let false_block = builder.create_block();
        let check_nl_block = builder.create_block();
        let nl_block = builder.create_block();
        let ret_block = builder.create_block();

        builder.switch_to_block(entry);
        builder.append_block_params_for_function_params(entry);
        let val = builder.block_params(entry)[0];
        let newline = builder.block_params(entry)[1];

        let is_true = builder.ins().icmp_imm(IntCC::NotEqual, val, 0);
        builder.ins().brif(is_true, true_block, &[], false_block, &[]);

        builder.switch_to_block(true_block);
        let local_write = cl_module.declare_func_in_func(write_raw_id, &mut builder.func);
        let local_true = cl_module.declare_data_in_func(true_id, &mut builder.func);
        let true_ptr = builder.ins().symbol_value(types::I64, local_true);
        let four = builder.ins().iconst(types::I32, 4);
        builder.ins().call(local_write, &[true_ptr, four]);
        builder.ins().jump(check_nl_block, &[]);

        builder.switch_to_block(false_block);
        let local_false = cl_module.declare_data_in_func(false_id, &mut builder.func);
        let false_ptr = builder.ins().symbol_value(types::I64, local_false);
        let five = builder.ins().iconst(types::I32, 5);
        builder.ins().call(local_write, &[false_ptr, five]);
        builder.ins().jump(check_nl_block, &[]);

        builder.switch_to_block(check_nl_block);
        let is_nl = builder.ins().icmp_imm(IntCC::NotEqual, newline, 0);
        builder.ins().brif(is_nl, nl_block, &[], ret_block, &[]);

        builder.switch_to_block(nl_block);
        let local_crlf = cl_module.declare_data_in_func(crlf_id, &mut builder.func);
        let crlf_ptr = builder.ins().symbol_value(types::I64, local_crlf);
        let two = builder.ins().iconst(types::I32, 2);
        builder.ins().call(local_write, &[crlf_ptr, two]);
        builder.ins().jump(ret_block, &[]);

        builder.switch_to_block(ret_block);
        builder.ins().return_(&[]);

        builder.seal_all_blocks();
        builder.finalize();
        cl_module.define_function(print_bool_id, ctx)?;
        cl_module.clear_context(ctx);
    }

    // 7. Define __qura_print_u64(val: i64, newline: i32)
    {
        ctx.func = Function::with_name_signature(UserFuncName::user(0, print_u64_id.as_u32()), sig_print_u64);
        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);
        let entry = builder.create_block();
        let zero_block = builder.create_block();
        let loop_init_block = builder.create_block();
        let loop_block = builder.create_block();
        let next_digit_block = builder.create_block();
        let done_digits_block = builder.create_block();
        let check_nl_block = builder.create_block();
        let nl_block = builder.create_block();
        let ret_block = builder.create_block();

        builder.switch_to_block(entry);
        builder.append_block_params_for_function_params(entry);
        let val = builder.block_params(entry)[0];
        let newline = builder.block_params(entry)[1];

        let slot = builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 32, 0));
        let slot_addr = builder.ins().stack_addr(types::I64, slot, 0);
        let buf_end = builder.ins().iadd_imm(slot_addr, 31);

        let is_zero = builder.ins().icmp_imm(IntCC::Equal, val, 0);
        builder.ins().brif(is_zero, zero_block, &[], loop_init_block, &[]);

        builder.switch_to_block(zero_block);
        let forty_eight = builder.ins().iconst(types::I8, 48);
        builder.ins().store(MemFlags::new(), forty_eight, buf_end, 0);
        let one = builder.ins().iconst(types::I32, 1);
        let local_write = cl_module.declare_func_in_func(write_raw_id, &mut builder.func);
        builder.ins().call(local_write, &[buf_end, one]);
        builder.ins().jump(check_nl_block, &[]);

        builder.switch_to_block(loop_init_block);
        builder.ins().jump(loop_block, &[BlockArg::from(val), BlockArg::from(buf_end)]);

        builder.switch_to_block(loop_block);
        let curr_val = builder.append_block_param(loop_block, types::I64);
        let curr_ptr = builder.append_block_param(loop_block, types::I64);
        let ten = builder.ins().iconst(types::I64, 10);
        let rem = builder.ins().urem(curr_val, ten);
        let digit64 = builder.ins().iadd_imm(rem, 48);
        let digit8 = builder.ins().ireduce(types::I8, digit64);
        builder.ins().store(MemFlags::new(), digit8, curr_ptr, 0);

        let next_val = builder.ins().udiv(curr_val, ten);
        let is_done = builder.ins().icmp_imm(IntCC::Equal, next_val, 0);
        builder.ins().brif(
            is_done,
            done_digits_block,
            &[BlockArg::from(curr_ptr)],
            next_digit_block,
            &[BlockArg::from(next_val), BlockArg::from(curr_ptr)],
        );

        builder.switch_to_block(next_digit_block);
        let v = builder.append_block_param(next_digit_block, types::I64);
        let p = builder.append_block_param(next_digit_block, types::I64);
        let prev_ptr = builder.ins().iadd_imm(p, -1);
        builder.ins().jump(loop_block, &[BlockArg::from(v), BlockArg::from(prev_ptr)]);

        builder.switch_to_block(done_digits_block);
        let first_ptr = builder.append_block_param(done_digits_block, types::I64);
        let buf_after = builder.ins().iadd_imm(slot_addr, 32);
        let len64 = builder.ins().isub(buf_after, first_ptr);
        let len32 = builder.ins().ireduce(types::I32, len64);
        let local_write = cl_module.declare_func_in_func(write_raw_id, &mut builder.func);
        builder.ins().call(local_write, &[first_ptr, len32]);
        builder.ins().jump(check_nl_block, &[]);

        builder.switch_to_block(check_nl_block);
        let is_nl = builder.ins().icmp_imm(IntCC::NotEqual, newline, 0);
        builder.ins().brif(is_nl, nl_block, &[], ret_block, &[]);

        builder.switch_to_block(nl_block);
        let local_crlf = cl_module.declare_data_in_func(crlf_id, &mut builder.func);
        let crlf_ptr = builder.ins().symbol_value(types::I64, local_crlf);
        let two = builder.ins().iconst(types::I32, 2);
        builder.ins().call(local_write, &[crlf_ptr, two]);
        builder.ins().jump(ret_block, &[]);

        builder.switch_to_block(ret_block);
        builder.ins().return_(&[]);

        builder.seal_all_blocks();
        builder.finalize();
        cl_module.define_function(print_u64_id, ctx)?;
        cl_module.clear_context(ctx);
    }

    // 8. Define __qura_print_i64(val: i64, newline: i32)
    {
        ctx.func = Function::with_name_signature(UserFuncName::user(0, print_i64_id.as_u32()), sig_print_i64);
        let mut builder = FunctionBuilder::new(&mut ctx.func, fn_builder_ctx);
        let entry = builder.create_block();
        let neg_block = builder.create_block();
        let pos_block = builder.create_block();

        builder.switch_to_block(entry);
        builder.append_block_params_for_function_params(entry);
        let val = builder.block_params(entry)[0];
        let newline = builder.block_params(entry)[1];

        let is_neg = builder.ins().icmp_imm(IntCC::SignedLessThan, val, 0);
        builder.ins().brif(is_neg, neg_block, &[], pos_block, &[]);

        builder.switch_to_block(neg_block);
        let local_write = cl_module.declare_func_in_func(write_raw_id, &mut builder.func);
        let local_minus = cl_module.declare_data_in_func(minus_id, &mut builder.func);
        let minus_ptr = builder.ins().symbol_value(types::I64, local_minus);
        let one = builder.ins().iconst(types::I32, 1);
        builder.ins().call(local_write, &[minus_ptr, one]);

        let mag = builder.ins().ineg(val);
        let local_u64 = cl_module.declare_func_in_func(print_u64_id, &mut builder.func);
        builder.ins().call(local_u64, &[mag, newline]);
        builder.ins().return_(&[]);

        builder.switch_to_block(pos_block);
        let local_u64 = cl_module.declare_func_in_func(print_u64_id, &mut builder.func);
        builder.ins().call(local_u64, &[val, newline]);
        builder.ins().return_(&[]);

        builder.seal_all_blocks();
        builder.finalize();
        cl_module.define_function(print_i64_id, ctx)?;
        cl_module.clear_context(ctx);
    }

    Ok(Runtime {
        print_str_id,
        print_i64_id,
        print_u64_id,
        print_bool_id,
    })
}
