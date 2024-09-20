use ceno_emul::{InsnKind, StepRecord};
use ff_ext::ExtensionField;

use super::{
    config::ExprLtConfig,
    constants::{UInt, PC_STEP_SIZE},
};
use crate::{
    chip_handler::{GlobalStateRegisterMachineChipOperations, RegisterChipOperations},
    circuit_builder::CircuitBuilder,
    error::ZKVMError,
    expression::{Expression, ToExpr, WitIn},
    instructions::riscv::config::ExprLtInput,
    set_val,
    tables::InsnRecord,
    uint::Value,
    witness::LkMultiplicity,
};
use core::mem::MaybeUninit;

/// This config handles the common part of R-type instructions:
/// - PC, cycle, fetch.
/// - Registers read and write.
///
/// It does not witness of the register values, nor the actual function (e.g. add, sub, etc).
#[derive(Debug)]
pub struct RInstructionConfig<E: ExtensionField> {
    pc: WitIn,
    ts: WitIn,
    rs1_id: WitIn,
    rs2_id: WitIn,
    rd_id: WitIn,
    prev_rd_value: UInt<E>,
    prev_rs1_ts: WitIn,
    prev_rs2_ts: WitIn,
    prev_rd_ts: WitIn,
    lt_rs1_cfg: ExprLtConfig,
    lt_rs2_cfg: ExprLtConfig,
    lt_prev_ts_cfg: ExprLtConfig,
}

impl<E: ExtensionField> RInstructionConfig<E> {
    pub fn construct_circuit(
        circuit_builder: &mut CircuitBuilder<E>,
        insn_kind: InsnKind,
        rs1_read: &impl ToExpr<E, Output = Vec<Expression<E>>>,
        rs2_read: &impl ToExpr<E, Output = Vec<Expression<E>>>,
        rd_written: &impl ToExpr<E, Output = Vec<Expression<E>>>,
    ) -> Result<Self, ZKVMError> {
        // State in.
        let pc = circuit_builder.create_witin(|| "pc")?;
        let cur_ts = circuit_builder.create_witin(|| "cur_ts")?;
        circuit_builder.state_in(pc.expr(), cur_ts.expr())?;

        // Register indexes.
        let rs1_id = circuit_builder.create_witin(|| "rs1_id")?;
        let rs2_id = circuit_builder.create_witin(|| "rs2_id")?;
        let rd_id = circuit_builder.create_witin(|| "rd_id")?;

        // Fetch the instruction.
        circuit_builder.lk_fetch(&InsnRecord::new(
            pc.expr(),
            (insn_kind.codes().opcode as usize).into(),
            rd_id.expr(),
            (insn_kind.codes().func3 as usize).into(),
            rs1_id.expr(),
            rs2_id.expr(),
            (insn_kind.codes().func7 as usize).into(),
        ))?;

        // Register state.
        let prev_rs1_ts = circuit_builder.create_witin(|| "prev_rs1_ts")?;
        let prev_rs2_ts = circuit_builder.create_witin(|| "prev_rs2_ts")?;
        let prev_rd_ts = circuit_builder.create_witin(|| "prev_rd_ts")?;
        let prev_rd_value = UInt::new_unchecked(|| "prev_rd_value", circuit_builder)?;

        // Register read and write.
        let (ts, lt_rs1_cfg) = circuit_builder.register_read(
            || "read_rs1",
            &rs1_id,
            prev_rs1_ts.expr(),
            cur_ts.expr(),
            rs1_read,
        )?;
        let (ts, lt_rs2_cfg) = circuit_builder.register_read(
            || "read_rs2",
            &rs2_id,
            prev_rs2_ts.expr(),
            ts,
            rs2_read,
        )?;
        let (ts, lt_prev_ts_cfg) = circuit_builder.register_write(
            || "write_rd",
            &rd_id,
            prev_rd_ts.expr(),
            ts,
            &prev_rd_value,
            rd_written,
        )?;

        // State out.
        let next_pc = pc.expr() + PC_STEP_SIZE.into();
        let next_ts = ts + 1.into();
        circuit_builder.state_out(next_pc, next_ts)?;

        Ok(RInstructionConfig {
            pc,
            ts: cur_ts,
            rs1_id,
            rs2_id,
            rd_id,
            prev_rd_value,
            prev_rs1_ts,
            prev_rs2_ts,
            prev_rd_ts,
            lt_rs1_cfg,
            lt_rs2_cfg,
            lt_prev_ts_cfg,
        })
    }

    pub fn assign_instance(
        &self,
        instance: &mut [MaybeUninit<<E as ExtensionField>::BaseField>],
        lk_multiplicity: &mut LkMultiplicity,
        step: &StepRecord,
    ) -> Result<(), ZKVMError> {
        // State in.
        set_val!(instance, self.pc, step.pc().before.0 as u64);
        set_val!(instance, self.ts, step.cycle());

        // Register indexes.
        set_val!(instance, self.rs1_id, step.insn().rs1() as u64);
        set_val!(instance, self.rs2_id, step.insn().rs2() as u64);
        set_val!(instance, self.rd_id, step.insn().rd() as u64);

        // Fetch the instruction.
        lk_multiplicity.fetch(step.pc().before.0);

        // Register state.
        set_val!(
            instance,
            self.prev_rs1_ts,
            step.rs1().unwrap().previous_cycle
        );
        set_val!(
            instance,
            self.prev_rs2_ts,
            step.rs2().unwrap().previous_cycle
        );
        set_val!(instance, self.prev_rd_ts, step.rd().unwrap().previous_cycle);
        self.prev_rd_value.assign_limbs(
            instance,
            Value::new_unchecked(step.rd().unwrap().value.before).u16_fields(),
        );

        // Register read and write.
        ExprLtInput {
            lhs: step.rs1().unwrap().previous_cycle,
            rhs: step.cycle(),
        }
        .assign(instance, &self.lt_rs1_cfg, lk_multiplicity);
        ExprLtInput {
            lhs: step.rs2().unwrap().previous_cycle,
            rhs: step.cycle() + 1,
        }
        .assign(instance, &self.lt_rs2_cfg, lk_multiplicity);
        ExprLtInput {
            lhs: step.rd().unwrap().previous_cycle,
            rhs: step.cycle() + 2,
        }
        .assign(instance, &self.lt_prev_ts_cfg, lk_multiplicity);

        Ok(())
    }
}