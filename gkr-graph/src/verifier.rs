use ff_ext::ExtensionField;
use gkr::structs::PointAndEval;
use itertools::{Itertools, izip};
use std::mem;
use transcript::Transcript;

use crate::{
    error::GKRGraphError,
    structs::{
        CircuitGraph, CircuitGraphAuxInfo, GKRVerifierState, IOPProof, IOPVerifierState,
        NodeOutputType, PredType, TargetEvaluations,
    },
};

impl<E: ExtensionField> IOPVerifierState<E> {
    pub fn verify(
        circuit: &CircuitGraph<E>,
        challenges: &[E],
        target_evals: &TargetEvaluations<E>,
        proof: IOPProof<E>,
        aux_info: &CircuitGraphAuxInfo,
        transcript: &mut Transcript<E>,
    ) -> Result<(), GKRGraphError> {
        assert_eq!(target_evals.0.len(), circuit.targets.len());

        let mut output_evals = vec![vec![]; circuit.nodes.len()];
        let mut wit_out_evals = circuit
            .nodes
            .iter()
            .map(|node| vec![PointAndEval::default(); node.circuit.n_witness_out])
            .collect_vec();
        izip!(&circuit.targets, &target_evals.0).for_each(|(target, eval)| match target {
            NodeOutputType::OutputLayer(id) => output_evals[*id].push(eval.clone()),
            NodeOutputType::WireOut(id, wire_out_id) => {
                wit_out_evals[*id][*wire_out_id as usize] = eval.clone()
            }
        });

        for ((node, instance_num_vars), proof) in izip!(
            izip!(&circuit.nodes, &aux_info.instance_num_vars,).rev(),
            proof.gkr_proofs
        ) {
            let input_claim = GKRVerifierState::verify_parallel(
                &node.circuit,
                challenges,
                mem::take(&mut output_evals[node.id]),
                mem::take(&mut wit_out_evals[node.id]),
                proof,
                *instance_num_vars,
                transcript,
            )?;

            let new_instance_num_vars = aux_info.instance_num_vars[node.id];

            izip!(&node.preds, input_claim.point_and_evals).for_each(
                |(pred_type, point_and_eval)| {
                    match pred_type {
                        PredType::Source => {
                            // TODO: collect `(proof.point.clone(), *eval)` as `TargetEvaluations`
                            // for later PCS open?
                        }
                        PredType::PredWire(pred_out) | PredType::PredWireDup(pred_out) => {
                            let point = match pred_type {
                                PredType::PredWire(_) => point_and_eval.point.clone(),
                                PredType::PredWireDup(out) => {
                                    let node_id = match out {
                                        NodeOutputType::OutputLayer(id) => *id,
                                        NodeOutputType::WireOut(id, _) => *id,
                                    };
                                    // Suppose the new point is
                                    // [single_instance_slice ||
                                    // new_instance_index_slice]. The old point
                                    // is [single_instance_slices ||
                                    // new_instance_index_slices[(new_instance_num_vars
                                    // - old_instance_num_vars)..]]
                                    let old_instance_num_vars = aux_info.instance_num_vars[node_id];
                                    let num_vars =
                                        point_and_eval.point.len() - new_instance_num_vars;
                                    [
                                        point_and_eval.point[..num_vars].to_vec(),
                                        point_and_eval.point[num_vars
                                            + (new_instance_num_vars - old_instance_num_vars)..]
                                            .to_vec(),
                                    ]
                                    .concat()
                                }
                                _ => unreachable!(),
                            };
                            match pred_out {
                                NodeOutputType::OutputLayer(id) => output_evals[*id]
                                    .push(PointAndEval::new_from_ref(&point, &point_and_eval.eval)),
                                NodeOutputType::WireOut(id, wire_id) => {
                                    let evals = &mut wit_out_evals[*id][*wire_id as usize];
                                    assert!(
                                        evals.point.is_empty() && evals.eval.is_zero_vartime(),
                                        "unimplemented",
                                    );
                                    *evals = PointAndEval::new(point, point_and_eval.eval);
                                }
                            }
                        }
                    }
                },
            );
        }

        Ok(())
    }
}
