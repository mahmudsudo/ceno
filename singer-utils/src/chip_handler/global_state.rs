use crate::{
    chip_handler::{ChipHandler, util::cell_to_mixed},
    structs::RAMType,
};
use ff_ext::ExtensionField;
use simple_frontend::structs::{CellId, CircuitBuilder, MixedCell};

pub struct GlobalStateChip {}

impl GlobalStateChip {
    pub fn state_in<Ext: ExtensionField>(
        chip_handler: &mut ChipHandler<Ext>,
        circuit_builder: &mut CircuitBuilder<Ext>,
        pc: &[CellId],
        stack_ts: &[CellId],
        memory_ts: &[CellId],
        stack_top: CellId,
        clk: CellId,
    ) {
        let key = [
            vec![MixedCell::Constant(Ext::BaseField::from(
                RAMType::GlobalState as u64,
            ))],
            cell_to_mixed(pc),
            cell_to_mixed(stack_ts),
            cell_to_mixed(memory_ts),
            vec![stack_top.into(), clk.into()],
        ]
        .concat();

        chip_handler
            .ram_handler
            .read_oam_mixed(circuit_builder, &[], &key, &[]);
    }

    pub fn state_out<Ext: ExtensionField>(
        chip_handler: &mut ChipHandler<Ext>,
        circuit_builder: &mut CircuitBuilder<Ext>,
        pc: &[CellId],
        stack_ts: &[CellId],
        memory_ts: &[CellId],
        stack_top: MixedCell<Ext>,
        clk: MixedCell<Ext>,
    ) {
        let key = [
            vec![MixedCell::Constant(Ext::BaseField::from(
                RAMType::GlobalState as u64,
            ))],
            cell_to_mixed(pc),
            cell_to_mixed(stack_ts),
            cell_to_mixed(memory_ts),
            vec![stack_top, clk],
        ]
        .concat();

        chip_handler
            .ram_handler
            .write_oam_mixed(circuit_builder, &[], &key, &[]);
    }
}
