use cess_proving_system_api::seal::{
    add_piece, aggregate_seal_commit_proofs, clear_cache, compute_comm_d, fauxrep, fauxrep2,
    generate_piece_commitment, get_seal_inputs, seal_commit_phase1, seal_commit_phase2,
    seal_pre_commit_phase1, seal_pre_commit_phase2, verify_aggregate_seal_commit_proofs,
    verify_seal, write_and_preprocess, SealCommitPhase2Output, SealPreCommitPhase2Output,
};
use cess_proving_system_api::{
    PartitionSnarkProof, PieceInfo, PrivateReplicaInfo, RegisteredPoStProof, RegisteredSealProof,
    SectorId, StorageProofsError, UnpaddedByteIndex, UnpaddedBytesAmount,
};
use ffi_toolkit::{
    c_str_to_pbuf, catch_panic_response, raw_ptr, rust_str_to_c_str, FCPResponseStatus,
};

use blstrs::Scalar as Fr;
use log::{error, info};
use rayon::prelude::*;

use std::mem;
use std::path::PathBuf;
use std::slice::from_raw_parts;

use super::helpers::{
    c_to_rust_partition_proofs, c_to_rust_post_proofs, to_private_replica_info_map,
};
use super::types::*;
use crate::util::api::init_log;

// A byte serialized representation of a vanilla proof.
pub type VanillaProof = Vec<u8>;

/// TODO: document
///
#[no_mangle]
#[cfg(not(target_os = "windows"))]
pub unsafe extern "C" fn cess_write_with_alignment(
    registered_proof: cess_RegisteredSealProof,
    src_fd: libc::c_int,
    src_size: u64,
    dst_fd: libc::c_int,
    existing_piece_sizes_ptr: *const u64,
    existing_piece_sizes_len: libc::size_t,
) -> *mut cess_WriteWithAlignmentResponse {
    catch_panic_response(|| {
        init_log();

        info!("write_with_alignment: start");

        let mut response = cess_WriteWithAlignmentResponse::default();

        let slice: &[u64] =
            std::slice::from_raw_parts(existing_piece_sizes_ptr, existing_piece_sizes_len);
        let piece_sizes: Vec<UnpaddedBytesAmount> = slice
            .to_vec()
            .iter()
            .map(|n| UnpaddedBytesAmount(*n))
            .collect();

        let n = UnpaddedBytesAmount(src_size);

        match add_piece(
            registered_proof.into(),
            FileDescriptorRef::new(src_fd),
            FileDescriptorRef::new(dst_fd),
            n,
            &piece_sizes,
        ) {
            Ok((info, written)) => {
                response.comm_p = info.commitment;
                response.left_alignment_unpadded = (written - n).into();
                response.status_code = FCPResponseStatus::FCPNoError;
                response.total_write_unpadded = written.into();
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        }

        info!("write_with_alignment: finish");

        raw_ptr(response)
    })
}

/// TODO: document
///
#[no_mangle]
#[cfg(not(target_os = "windows"))]
pub unsafe extern "C" fn cess_write_without_alignment(
    registered_proof: cess_RegisteredSealProof,
    src_fd: libc::c_int,
    src_size: u64,
    dst_fd: libc::c_int,
) -> *mut cess_WriteWithoutAlignmentResponse {
    catch_panic_response(|| {
        init_log();

        info!("write_without_alignment: start");

        let mut response = cess_WriteWithoutAlignmentResponse::default();

        match write_and_preprocess(
            registered_proof.into(),
            FileDescriptorRef::new(src_fd),
            FileDescriptorRef::new(dst_fd),
            UnpaddedBytesAmount(src_size),
        ) {
            Ok((info, written)) => {
                response.comm_p = info.commitment;
                response.status_code = FCPResponseStatus::FCPNoError;
                response.total_write_unpadded = written.into();
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        }

        info!("write_without_alignment: finish");

        raw_ptr(response)
    })
}

#[no_mangle]
pub unsafe extern "C" fn cess_fauxrep(
    registered_proof: cess_RegisteredSealProof,
    cache_dir_path: *const libc::c_char,
    sealed_sector_path: *const libc::c_char,
) -> *mut cess_FauxRepResponse {
    catch_panic_response(|| {
        init_log();

        info!("fauxrep: start");

        let mut response: cess_FauxRepResponse = Default::default();

        let result = fauxrep(
            registered_proof.into(),
            c_str_to_pbuf(cache_dir_path),
            c_str_to_pbuf(sealed_sector_path),
        );

        match result {
            Ok(output) => {
                response.status_code = FCPResponseStatus::FCPNoError;
                response.commitment = output;
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        }

        info!("fauxrep: finish");

        raw_ptr(response)
    })
}

#[no_mangle]
pub unsafe extern "C" fn cess_fauxrep2(
    registered_proof: cess_RegisteredSealProof,
    cache_dir_path: *const libc::c_char,
    existing_p_aux_path: *const libc::c_char,
) -> *mut cess_FauxRepResponse {
    catch_panic_response(|| {
        init_log();

        info!("fauxrep2: start");

        let mut response: cess_FauxRepResponse = Default::default();

        let result = fauxrep2(
            registered_proof.into(),
            c_str_to_pbuf(cache_dir_path),
            c_str_to_pbuf(existing_p_aux_path),
        );

        match result {
            Ok(output) => {
                response.status_code = FCPResponseStatus::FCPNoError;
                response.commitment = output;
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        }

        info!("fauxrep2: finish");

        raw_ptr(response)
    })
}

/// TODO: document
///
#[no_mangle]
pub unsafe extern "C" fn cess_seal_pre_commit_phase1(
    registered_proof: cess_RegisteredSealProof,
    cache_dir_path: *const libc::c_char,
    staged_sector_path: *const libc::c_char,
    sealed_sector_path: *const libc::c_char,
    sector_id: u64,
    prover_id: cess_32ByteArray,
    ticket: cess_32ByteArray,
    pieces_ptr: *const cess_PublicPieceInfo,
    pieces_len: libc::size_t,
) -> *mut cess_SealPreCommitPhase1Response {
    catch_panic_response(|| {
        init_log();

        info!("seal_pre_commit_phase1: start");

        let slice: &[cess_PublicPieceInfo] = std::slice::from_raw_parts(pieces_ptr, pieces_len);
        let public_pieces: Vec<PieceInfo> =
            slice.to_vec().iter().cloned().map(Into::into).collect();

        let mut response: cess_SealPreCommitPhase1Response = Default::default();

        let result = seal_pre_commit_phase1(
            registered_proof.into(),
            c_str_to_pbuf(cache_dir_path),
            c_str_to_pbuf(staged_sector_path),
            c_str_to_pbuf(sealed_sector_path),
            prover_id.inner,
            SectorId::from(sector_id),
            ticket.inner,
            &public_pieces,
        )
        .and_then(|output| serde_json::to_vec(&output).map_err(Into::into));

        match result {
            Ok(output) => {
                response.status_code = FCPResponseStatus::FCPNoError;
                response.seal_pre_commit_phase1_output_ptr = output.as_ptr();
                response.seal_pre_commit_phase1_output_len = output.len();
                mem::forget(output);
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        }

        info!("seal_pre_commit_phase1: finish");

        raw_ptr(response)
    })
}

/// TODO: document
///
#[no_mangle]
pub unsafe extern "C" fn cess_seal_pre_commit_phase2(
    seal_pre_commit_phase1_output_ptr: *const u8,
    seal_pre_commit_phase1_output_len: libc::size_t,
    cache_dir_path: *const libc::c_char,
    sealed_sector_path: *const libc::c_char,
) -> *mut cess_SealPreCommitPhase2Response {
    catch_panic_response(|| {
        init_log();

        info!("seal_pre_commit_phase2: start");

        let mut response: cess_SealPreCommitPhase2Response = Default::default();

        let phase_1_output = serde_json::from_slice(from_raw_parts(
            seal_pre_commit_phase1_output_ptr,
            seal_pre_commit_phase1_output_len,
        ))
        .map_err(Into::into);

        let result = phase_1_output.and_then(|o| {
            seal_pre_commit_phase2::<PathBuf, PathBuf>(
                o,
                c_str_to_pbuf(cache_dir_path),
                c_str_to_pbuf(sealed_sector_path),
            )
        });

        match result {
            Ok(output) => {
                response.status_code = FCPResponseStatus::FCPNoError;
                response.comm_r = output.comm_r;
                response.comm_d = output.comm_d;
                response.registered_proof = output.registered_proof.into();
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        }

        info!("seal_pre_commit_phase2: finish");

        raw_ptr(response)
    })
}

/// TODO: document
///
#[no_mangle]
pub unsafe extern "C" fn cess_seal_commit_phase1(
    registered_proof: cess_RegisteredSealProof,
    comm_r: cess_32ByteArray,
    comm_d: cess_32ByteArray,
    cache_dir_path: *const libc::c_char,
    replica_path: *const libc::c_char,
    sector_id: u64,
    prover_id: cess_32ByteArray,
    ticket: cess_32ByteArray,
    seed: cess_32ByteArray,
    pieces_ptr: *const cess_PublicPieceInfo,
    pieces_len: libc::size_t,
) -> *mut cess_SealCommitPhase1Response {
    catch_panic_response(|| {
        init_log();

        info!("seal_commit_phase1: start");

        let mut response = cess_SealCommitPhase1Response::default();

        let spcp2o = SealPreCommitPhase2Output {
            registered_proof: registered_proof.into(),
            comm_r: comm_r.inner,
            comm_d: comm_d.inner,
        };

        let slice: &[cess_PublicPieceInfo] = std::slice::from_raw_parts(pieces_ptr, pieces_len);
        let public_pieces: Vec<PieceInfo> =
            slice.to_vec().iter().cloned().map(Into::into).collect();

        let result = seal_commit_phase1(
            c_str_to_pbuf(cache_dir_path),
            c_str_to_pbuf(replica_path),
            prover_id.inner,
            SectorId::from(sector_id),
            ticket.inner,
            seed.inner,
            spcp2o,
            &public_pieces,
        );

        match result.and_then(|output| serde_json::to_vec(&output).map_err(Into::into)) {
            Ok(output) => {
                response.status_code = FCPResponseStatus::FCPNoError;
                response.seal_commit_phase1_output_ptr = output.as_ptr();
                response.seal_commit_phase1_output_len = output.len();
                mem::forget(output);
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        }

        info!("seal_commit_phase1: finish");

        raw_ptr(response)
    })
}

#[no_mangle]
pub unsafe extern "C" fn cess_seal_commit_phase2(
    seal_commit_phase1_output_ptr: *const u8,
    seal_commit_phase1_output_len: libc::size_t,
    sector_id: u64,
    prover_id: cess_32ByteArray,
) -> *mut cess_SealCommitPhase2Response {
    catch_panic_response(|| {
        init_log();

        info!("seal_commit_phase2: start");

        let mut response = cess_SealCommitPhase2Response::default();

        let scp1o = serde_json::from_slice(from_raw_parts(
            seal_commit_phase1_output_ptr,
            seal_commit_phase1_output_len,
        ))
        .map_err(Into::into);

        let result =
            scp1o.and_then(|o| seal_commit_phase2(o, prover_id.inner, SectorId::from(sector_id)));

        match result {
            Ok(output) => {
                response.status_code = FCPResponseStatus::FCPNoError;
                response.proof_ptr = output.proof.as_ptr();
                response.proof_len = output.proof.len();
                mem::forget(output.proof);
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        }

        info!("seal_commit_phase2: finish");

        raw_ptr(response)
    })
}

#[no_mangle]
pub unsafe extern "C" fn cess_aggregate_seal_proofs(
    registered_proof: cess_RegisteredSealProof,
    registered_aggregation: cess_RegisteredAggregationProof,
    comm_rs_ptr: *const cess_32ByteArray,
    comm_rs_len: libc::size_t,
    seeds_ptr: *const cess_32ByteArray,
    seeds_len: libc::size_t,
    seal_commit_responses_ptr: *const cess_SealCommitPhase2Response,
    seal_commit_responses_len: libc::size_t,
) -> *mut cess_AggregateProof {
    catch_panic_response(|| {
        init_log();
        info!("aggregate_seal_proofs: start");

        let responses: &[cess_SealCommitPhase2Response] =
            std::slice::from_raw_parts(seal_commit_responses_ptr, seal_commit_responses_len);
        let outputs: Vec<SealCommitPhase2Output> = responses.iter().map(|x| x.into()).collect();

        let raw_comm_rs: &[cess_32ByteArray] = std::slice::from_raw_parts(comm_rs_ptr, comm_rs_len);
        let comm_rs: Vec<[u8; 32]> = raw_comm_rs.iter().map(|x| x.inner).collect();

        let raw_seeds: &[cess_32ByteArray] = std::slice::from_raw_parts(seeds_ptr, seeds_len);
        let seeds: Vec<[u8; 32]> = raw_seeds.iter().map(|x| x.inner).collect();

        let mut response = cess_AggregateProof::default();

        let result = aggregate_seal_commit_proofs(
            registered_proof.into(),
            registered_aggregation.into(),
            &comm_rs,
            &seeds,
            &outputs,
        );

        match result {
            Ok(output) => {
                response.status_code = FCPResponseStatus::FCPNoError;
                response.proof_ptr = output.as_ptr();
                response.proof_len = output.len();

                mem::forget(output);
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        }

        info!("aggregate_seal_proofs: finish");

        raw_ptr(response)
    })
}

/// Retrieves the seal inputs based on the provided input, used for aggregation verification.
///
#[no_mangle]
pub fn convert_aggregation_inputs(
    registered_proof: cess_RegisteredSealProof,
    prover_id: cess_32ByteArray,
    input: &cess_AggregationInputs,
) -> anyhow::Result<Vec<Vec<Fr>>> {
    get_seal_inputs(
        registered_proof.into(),
        input.comm_r.inner,
        input.comm_d.inner,
        prover_id.inner,
        SectorId::from(input.sector_id),
        input.ticket.inner,
        input.seed.inner,
    )
}

/// Verifies the output of an aggregated seal.
///
#[no_mangle]
pub unsafe extern "C" fn cess_verify_aggregate_seal_proof(
    registered_proof: cess_RegisteredSealProof,
    registered_aggregation: cess_RegisteredAggregationProof,
    prover_id: cess_32ByteArray,
    proof_ptr: *const u8,
    proof_len: libc::size_t,
    commit_inputs_ptr: *mut cess_AggregationInputs,
    commit_inputs_len: libc::size_t,
) -> *mut cess_VerifyAggregateSealProofResponse {
    catch_panic_response(|| {
        init_log();

        info!("verify_aggregate_seal_proof: start");

        let commit_inputs: &[cess_AggregationInputs] =
            std::slice::from_raw_parts(commit_inputs_ptr, commit_inputs_len);

        let inputs: anyhow::Result<Vec<Vec<Fr>>> = commit_inputs
            .par_iter()
            .map(|input| convert_aggregation_inputs(registered_proof, prover_id, input))
            .try_reduce(Vec::new, |mut acc, current| {
                acc.extend(current);
                Ok(acc)
            });

        let mut response = cess_VerifyAggregateSealProofResponse::default();

        match inputs {
            Ok(inputs) => {
                let proof_bytes: Vec<u8> =
                    std::slice::from_raw_parts(proof_ptr, proof_len).to_vec();
                let comm_rs: Vec<[u8; 32]> = commit_inputs
                    .iter()
                    .map(|input| input.comm_r.inner)
                    .collect();
                let seeds: Vec<[u8; 32]> =
                    commit_inputs.iter().map(|input| input.seed.inner).collect();

                let result = verify_aggregate_seal_commit_proofs(
                    registered_proof.into(),
                    registered_aggregation.into(),
                    proof_bytes,
                    &comm_rs,
                    &seeds,
                    inputs,
                );

                match result {
                    Ok(true) => {
                        response.status_code = FCPResponseStatus::FCPNoError;
                        response.is_valid = true;
                    }
                    Ok(false) => {
                        response.status_code = FCPResponseStatus::FCPNoError;
                        response.is_valid = false;
                    }
                    Err(err) => {
                        response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                        response.error_msg = rust_str_to_c_str(format!("{:?}", err));
                        response.is_valid = false;
                    }
                }
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
                response.is_valid = false;
            }
        }

        info!("verify_aggregate_seal_proof: finish");

        raw_ptr(response)
    })
}

/// TODO: document
#[no_mangle]
pub unsafe extern "C" fn cess_unseal_range(
    registered_proof: cess_RegisteredSealProof,
    cache_dir_path: *const libc::c_char,
    sealed_sector_fd_raw: libc::c_int,
    unseal_output_fd_raw: libc::c_int,
    sector_id: u64,
    prover_id: cess_32ByteArray,
    ticket: cess_32ByteArray,
    comm_d: cess_32ByteArray,
    unpadded_byte_index: u64,
    unpadded_bytes_amount: u64,
) -> *mut cess_UnsealRangeResponse {
    catch_panic_response(|| {
        init_log();

        info!("unseal_range: start");

        use filepath::FilePath;
        use std::os::unix::io::{FromRawFd, IntoRawFd};

        let sealed_sector = std::fs::File::from_raw_fd(sealed_sector_fd_raw);
        let mut unseal_output = std::fs::File::from_raw_fd(unseal_output_fd_raw);

        let result = cess_proving_system_api::seal::get_unsealed_range_mapped(
            registered_proof.into(),
            c_str_to_pbuf(cache_dir_path),
            sealed_sector.path().unwrap(),
            &mut unseal_output,
            prover_id.inner,
            SectorId::from(sector_id),
            comm_d.inner,
            ticket.inner,
            UnpaddedByteIndex(unpadded_byte_index),
            UnpaddedBytesAmount(unpadded_bytes_amount),
        );

        // keep all file descriptors alive until unseal_range returns
        let _ = sealed_sector.into_raw_fd();
        let _ = unseal_output.into_raw_fd();

        let mut response = cess_UnsealRangeResponse::default();

        match result {
            Ok(_) => {
                response.status_code = FCPResponseStatus::FCPNoError;
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        };

        info!("unseal_range: finish");

        raw_ptr(response)
    })
}

/// Verifies the output of seal.
///
#[no_mangle]
pub unsafe extern "C" fn cess_verify_seal(
    registered_proof: cess_RegisteredSealProof,
    comm_r: cess_32ByteArray,
    comm_d: cess_32ByteArray,
    prover_id: cess_32ByteArray,
    ticket: cess_32ByteArray,
    seed: cess_32ByteArray,
    sector_id: u64,
    proof_ptr: *const u8,
    proof_len: libc::size_t,
) -> *mut super::types::cess_VerifySealResponse {
    catch_panic_response(|| {
        init_log();

        info!("verify_seal: start");

        let proof_bytes: Vec<u8> = std::slice::from_raw_parts(proof_ptr, proof_len).to_vec();

        let result = verify_seal(
            registered_proof.into(),
            comm_r.inner,
            comm_d.inner,
            prover_id.inner,
            SectorId::from(sector_id),
            ticket.inner,
            seed.inner,
            &proof_bytes,
        );

        let mut response = cess_VerifySealResponse::default();

        match result {
            Ok(true) => {
                response.status_code = FCPResponseStatus::FCPNoError;
                response.is_valid = true;
            }
            Ok(false) => {
                response.status_code = FCPResponseStatus::FCPNoError;
                response.is_valid = false;
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        };

        info!("verify_seal: finish");

        raw_ptr(response)
    })
}

/// TODO: document
///
#[no_mangle]
pub unsafe extern "C" fn cess_generate_winning_post_sector_challenge(
    registered_proof: cess_RegisteredPoStProof,
    randomness: cess_32ByteArray,
    sector_set_len: u64,
    prover_id: cess_32ByteArray,
) -> *mut cess_GenerateWinningPoStSectorChallenge {
    catch_panic_response(|| {
        init_log();

        info!("generate_winning_post_sector_challenge: start");

        let mut response = cess_GenerateWinningPoStSectorChallenge::default();

        let result = cess_proving_system_api::post::generate_winning_post_sector_challenge(
            registered_proof.into(),
            &randomness.inner,
            sector_set_len,
            prover_id.inner,
        );

        match result {
            Ok(output) => {
                let mapped: Vec<u64> = output.into_iter().map(u64::from).collect();

                response.status_code = FCPResponseStatus::FCPNoError;
                response.ids_ptr = mapped.as_ptr();
                response.ids_len = mapped.len();

                mem::forget(mapped);
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        }

        info!("generate_winning_post_sector_challenge: finish");

        raw_ptr(response)
    })
}

/// TODO: document
///
#[no_mangle]
pub unsafe extern "C" fn cess_generate_fallback_sector_challenges(
    registered_proof: cess_RegisteredPoStProof,
    randomness: cess_32ByteArray,
    sector_ids_ptr: *const u64,
    sector_ids_len: libc::size_t,
    prover_id: cess_32ByteArray,
) -> *mut cess_GenerateFallbackSectorChallengesResponse {
    catch_panic_response(|| {
        init_log();

        info!("generate_fallback_sector_challenges: start");

        let slice: &[u64] = std::slice::from_raw_parts(sector_ids_ptr, sector_ids_len);
        let pub_sectors: Vec<SectorId> = slice.to_vec().iter().cloned().map(Into::into).collect();

        let result = cess_proving_system_api::post::generate_fallback_sector_challenges(
            registered_proof.into(),
            &randomness.inner,
            &pub_sectors,
            prover_id.inner,
        );

        let mut response = cess_GenerateFallbackSectorChallengesResponse::default();

        match result {
            Ok(output) => {
                response.status_code = FCPResponseStatus::FCPNoError;

                let sector_ids: Vec<u64> = output
                    .clone()
                    .into_iter()
                    .map(|(id, _challenges)| u64::from(id))
                    .collect();
                let mut challenges_stride = 0;
                let mut challenges_stride_mismatch = false;
                let challenges: Vec<u64> = output
                    .into_iter()
                    .flat_map(|(_id, challenges)| {
                        if challenges_stride == 0 {
                            challenges_stride = challenges.len();
                        }

                        if !challenges_stride_mismatch && challenges_stride != challenges.len() {
                            error!(
                                "All challenge strides must be equal: {} != {}",
                                challenges_stride,
                                challenges.len()
                            );
                            challenges_stride_mismatch = true;
                        }

                        challenges
                    })
                    .collect();

                if challenges_stride_mismatch {
                    response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                    response.error_msg = rust_str_to_c_str("Challenge stride mismatch".to_string());
                } else {
                    response.status_code = FCPResponseStatus::FCPNoError;
                    response.ids_ptr = sector_ids.as_ptr();
                    response.ids_len = sector_ids.len();
                    response.challenges_ptr = challenges.as_ptr();
                    response.challenges_len = challenges.len();
                    response.challenges_stride = challenges_stride;

                    mem::forget(sector_ids);
                    mem::forget(challenges);
                }
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        };

        info!("generate_fallback_sector_challenges: finish");

        raw_ptr(response)
    })
}

/// TODO: document
///
#[no_mangle]
pub unsafe extern "C" fn cess_generate_single_vanilla_proof(
    replica: cess_PrivateReplicaInfo,
    challenges_ptr: *const u64,
    challenges_len: libc::size_t,
) -> *mut cess_GenerateSingleVanillaProofResponse {
    catch_panic_response(|| {
        init_log();

        info!("generate_single_vanilla_proof: start");

        let challenges: Vec<u64> = std::slice::from_raw_parts(challenges_ptr, challenges_len)
            .to_vec()
            .iter()
            .copied()
            .collect();

        let sector_id = SectorId::from(replica.sector_id);
        let cache_dir_path = c_str_to_pbuf(replica.cache_dir_path);
        let replica_path = c_str_to_pbuf(replica.replica_path);

        let replica_v1 = PrivateReplicaInfo::new(
            replica.registered_proof.into(),
            replica.comm_r,
            cache_dir_path,
            replica_path,
        );

        let result = cess_proving_system_api::post::generate_single_vanilla_proof(
            replica.registered_proof.into(),
            sector_id,
            &replica_v1,
            &challenges,
        );

        let mut response = cess_GenerateSingleVanillaProofResponse::default();

        match result {
            Ok(output) => {
                response.status_code = FCPResponseStatus::FCPNoError;
                response.vanilla_proof.proof_len = output.len();
                response.vanilla_proof.proof_ptr = output.as_ptr();

                mem::forget(output);
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        };

        info!("generate_single_vanilla_proof: finish");

        raw_ptr(response)
    })
}

/// TODO: document
///
#[no_mangle]
pub unsafe extern "C" fn cess_generate_winning_post_with_vanilla(
    registered_proof: cess_RegisteredPoStProof,
    randomness: cess_32ByteArray,
    prover_id: cess_32ByteArray,
    vanilla_proofs_ptr: *const cess_VanillaProof,
    vanilla_proofs_len: libc::size_t,
) -> *mut cess_GenerateWinningPoStResponse {
    catch_panic_response(|| {
        init_log();

        info!("generate_winning_post_with_vanilla: start");

        let proofs: &[cess_VanillaProof] =
            std::slice::from_raw_parts(vanilla_proofs_ptr, vanilla_proofs_len);
        let vanilla_proofs: Vec<VanillaProof> = proofs
            .iter()
            .cloned()
            .map(|vanilla_proof| {
                std::slice::from_raw_parts(vanilla_proof.proof_ptr, vanilla_proof.proof_len)
                    .to_vec()
            })
            .collect();

        let result = cess_proving_system_api::post::generate_winning_post_with_vanilla(
            registered_proof.into(),
            &randomness.inner,
            prover_id.inner,
            &vanilla_proofs,
        );

        let mut response = cess_GenerateWinningPoStResponse::default();

        match result {
            Ok(output) => {
                let mapped: Vec<cess_PoStProof> = output
                    .iter()
                    .cloned()
                    .map(|(t, proof)| {
                        let out = cess_PoStProof {
                            registered_proof: (t).into(),
                            proof_len: proof.len(),
                            proof_ptr: proof.as_ptr(),
                        };

                        mem::forget(proof);

                        out
                    })
                    .collect();

                response.status_code = FCPResponseStatus::FCPNoError;
                response.proofs_ptr = mapped.as_ptr();
                response.proofs_len = mapped.len();

                mem::forget(mapped);
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        }

        info!("generate_winning_post_with_vanilla: finish");

        raw_ptr(response)
    })
}

/// TODO: document
///
#[no_mangle]
pub unsafe extern "C" fn cess_generate_winning_post(
    randomness: cess_32ByteArray,
    replicas_ptr: *const cess_PrivateReplicaInfo,
    replicas_len: libc::size_t,
    prover_id: cess_32ByteArray,
) -> *mut cess_GenerateWinningPoStResponse {
    catch_panic_response(|| {
        init_log();

        info!("generate_winning_post: start");

        let mut response = cess_GenerateWinningPoStResponse::default();

        let result = to_private_replica_info_map(replicas_ptr, replicas_len).and_then(|rs| {
            cess_proving_system_api::post::generate_winning_post(
                &randomness.inner,
                &rs,
                prover_id.inner,
            )
        });

        match result {
            Ok(output) => {
                let mapped: Vec<cess_PoStProof> = output
                    .iter()
                    .cloned()
                    .map(|(t, proof)| {
                        let out = cess_PoStProof {
                            registered_proof: (t).into(),
                            proof_len: proof.len(),
                            proof_ptr: proof.as_ptr(),
                        };

                        mem::forget(proof);

                        out
                    })
                    .collect();

                response.status_code = FCPResponseStatus::FCPNoError;
                response.proofs_ptr = mapped.as_ptr();
                response.proofs_len = mapped.len();
                mem::forget(mapped);
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        }

        info!("generate_winning_post: finish");

        raw_ptr(response)
    })
}

/// Verifies that a proof-of-spacetime is valid.
#[no_mangle]
pub unsafe extern "C" fn cess_verify_winning_post(
    randomness: cess_32ByteArray,
    replicas_ptr: *const cess_PublicReplicaInfo,
    replicas_len: libc::size_t,
    proofs_ptr: *const cess_PoStProof,
    proofs_len: libc::size_t,
    prover_id: cess_32ByteArray,
) -> *mut cess_VerifyWinningPoStResponse {
    catch_panic_response(|| {
        init_log();

        info!("verify_winning_post: start");

        let mut response = cess_VerifyWinningPoStResponse::default();

        let convert = super::helpers::to_public_replica_info_map(replicas_ptr, replicas_len);

        let result = convert.and_then(|replicas| {
            let post_proofs = c_to_rust_post_proofs(proofs_ptr, proofs_len)?;
            let proofs: Vec<u8> = post_proofs.iter().flat_map(|pp| pp.clone().proof).collect();

            cess_proving_system_api::post::verify_winning_post(
                &randomness.inner,
                &proofs,
                &replicas,
                prover_id.inner,
            )
        });

        match result {
            Ok(is_valid) => {
                response.status_code = FCPResponseStatus::FCPNoError;
                response.is_valid = is_valid;
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        };

        info!("verify_winning_post: {}", "finish");
        raw_ptr(response)
    })
}

/// TODO: document
///
#[no_mangle]
pub unsafe extern "C" fn cess_generate_window_post_with_vanilla(
    registered_proof: cess_RegisteredPoStProof,
    randomness: cess_32ByteArray,
    prover_id: cess_32ByteArray,
    vanilla_proofs_ptr: *const cess_VanillaProof,
    vanilla_proofs_len: libc::size_t,
) -> *mut cess_GenerateWindowPoStResponse {
    catch_panic_response(|| {
        init_log();

        info!("generate_window_post_with_vanilla: start");

        let vanilla_proofs: Vec<VanillaProof> =
            std::slice::from_raw_parts(vanilla_proofs_ptr, vanilla_proofs_len)
                .iter()
                .cloned()
                .map(|vanilla_proof| {
                    std::slice::from_raw_parts(vanilla_proof.proof_ptr, vanilla_proof.proof_len)
                        .to_vec()
                })
                .collect();

        let result = cess_proving_system_api::post::generate_window_post_with_vanilla(
            registered_proof.into(),
            &randomness.inner,
            prover_id.inner,
            &vanilla_proofs,
        );

        let mut response = cess_GenerateWindowPoStResponse::default();

        match result {
            Ok(output) => {
                let mapped: Vec<cess_PoStProof> = output
                    .iter()
                    .cloned()
                    .map(|(t, proof)| {
                        let out = cess_PoStProof {
                            registered_proof: (t).into(),
                            proof_len: proof.len(),
                            proof_ptr: proof.as_ptr(),
                        };

                        mem::forget(proof);

                        out
                    })
                    .collect();

                response.status_code = FCPResponseStatus::FCPNoError;
                response.proofs_ptr = mapped.as_ptr();
                response.proofs_len = mapped.len();
                mem::forget(mapped);
            }
            Err(err) => {
                // If there were faulty sectors, add them to the response
                if let Some(StorageProofsError::FaultySectors(sectors)) =
                    err.downcast_ref::<StorageProofsError>()
                {
                    let sectors_u64 = sectors
                        .iter()
                        .map(|sector| u64::from(*sector))
                        .collect::<Vec<u64>>();
                    response.faulty_sectors_len = sectors_u64.len();
                    response.faulty_sectors_ptr = sectors_u64.as_ptr();
                    mem::forget(sectors_u64)
                }

                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        }

        info!("generate_window_post_with_vanilla: finish");

        raw_ptr(response)
    })
}

/// TODO: document
///
#[no_mangle]
pub unsafe extern "C" fn cess_generate_window_post(
    randomness: cess_32ByteArray,
    replicas_ptr: *const cess_PrivateReplicaInfo,
    replicas_len: libc::size_t,
    prover_id: cess_32ByteArray,
) -> *mut cess_GenerateWindowPoStResponse {
    catch_panic_response(|| {
        init_log();

        info!("generate_window_post: start");

        let mut response = cess_GenerateWindowPoStResponse::default();

        let result = to_private_replica_info_map(replicas_ptr, replicas_len).and_then(|rs| {
            cess_proving_system_api::post::generate_window_post(
                &randomness.inner,
                &rs,
                prover_id.inner,
            )
        });

        match result {
            Ok(output) => {
                let mapped: Vec<cess_PoStProof> = output
                    .iter()
                    .cloned()
                    .map(|(t, proof)| {
                        let out = cess_PoStProof {
                            registered_proof: (t).into(),
                            proof_len: proof.len(),
                            proof_ptr: proof.as_ptr(),
                        };

                        mem::forget(proof);

                        out
                    })
                    .collect();

                response.status_code = FCPResponseStatus::FCPNoError;
                response.proofs_ptr = mapped.as_ptr();
                response.proofs_len = mapped.len();
                mem::forget(mapped);
            }
            Err(err) => {
                // If there were faulty sectors, add them to the response
                if let Some(StorageProofsError::FaultySectors(sectors)) =
                    err.downcast_ref::<StorageProofsError>()
                {
                    let sectors_u64 = sectors
                        .iter()
                        .map(|sector| u64::from(*sector))
                        .collect::<Vec<u64>>();
                    response.faulty_sectors_len = sectors_u64.len();
                    response.faulty_sectors_ptr = sectors_u64.as_ptr();
                    mem::forget(sectors_u64)
                }

                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        }

        info!("generate_window_post: finish");

        raw_ptr(response)
    })
}

/// Verifies that a proof-of-spacetime is valid.
#[no_mangle]
pub unsafe extern "C" fn cess_verify_window_post(
    randomness: cess_32ByteArray,
    replicas_ptr: *const cess_PublicReplicaInfo,
    replicas_len: libc::size_t,
    proofs_ptr: *const cess_PoStProof,
    proofs_len: libc::size_t,
    prover_id: cess_32ByteArray,
) -> *mut cess_VerifyWindowPoStResponse {
    catch_panic_response(|| {
        init_log();

        info!("verify_window_post: start");

        let mut response = cess_VerifyWindowPoStResponse::default();

        let convert = super::helpers::to_public_replica_info_map(replicas_ptr, replicas_len);

        let result = convert.and_then(|replicas| {
            let post_proofs = c_to_rust_post_proofs(proofs_ptr, proofs_len)?;

            let proofs: Vec<(RegisteredPoStProof, &[u8])> = post_proofs
                .iter()
                .map(|x| (x.registered_proof, x.proof.as_ref()))
                .collect();

            cess_proving_system_api::post::verify_window_post(
                &randomness.inner,
                &proofs,
                &replicas,
                prover_id.inner,
            )
        });

        match result {
            Ok(is_valid) => {
                response.status_code = FCPResponseStatus::FCPNoError;
                response.is_valid = is_valid;
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        };

        info!("verify_window_post: {}", "finish");
        raw_ptr(response)
    })
}

/// TODO: document
///
#[no_mangle]
pub unsafe extern "C" fn cess_merge_window_post_partition_proofs(
    registered_proof: cess_RegisteredPoStProof,
    partition_proofs_ptr: *const cess_PartitionSnarkProof,
    partition_proofs_len: libc::size_t,
) -> *mut cess_MergeWindowPoStPartitionProofsResponse {
    catch_panic_response(|| {
        init_log();

        info!("merge_window_post_partition_proofs: start");

        let mut response = cess_MergeWindowPoStPartitionProofsResponse::default();

        let partition_proofs: Vec<PartitionSnarkProof> =
            match c_to_rust_partition_proofs(partition_proofs_ptr, partition_proofs_len) {
                Ok(partition_proofs) => partition_proofs
                    .iter()
                    .map(|pp| PartitionSnarkProof(pp.proof.clone()))
                    .collect(),
                Err(err) => {
                    response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                    response.error_msg = rust_str_to_c_str(format!("{:?}", err));
                    Vec::new()
                }
            };

        if !partition_proofs.is_empty() {
            let result = cess_proving_system_api::post::merge_window_post_partition_proofs(
                registered_proof.into(),
                partition_proofs,
            );

            match result {
                Ok(output) => {
                    let proof = cess_PoStProof {
                        registered_proof,
                        proof_ptr: output.as_ptr(),
                        proof_len: output.len(),
                    };

                    response.status_code = FCPResponseStatus::FCPNoError;
                    response.proof = proof;

                    mem::forget(output);
                }
                Err(err) => {
                    response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                    response.error_msg = rust_str_to_c_str(format!("{:?}", err));
                }
            }
        }

        info!("merge_window_post_partition_proofs: finish");

        raw_ptr(response)
    })
}

/// TODO: document
///
#[no_mangle]
pub unsafe extern "C" fn cess_get_num_partition_for_fallback_post(
    registered_proof: cess_RegisteredPoStProof,
    num_sectors: libc::size_t,
) -> *mut cess_GetNumPartitionForFallbackPoStResponse {
    catch_panic_response(|| {
        init_log();

        info!("get_num_partition_for_fallback_post: start");
        let result = cess_proving_system_api::post::get_num_partition_for_fallback_post(
            registered_proof.into(),
            num_sectors,
        );

        let mut response = cess_GetNumPartitionForFallbackPoStResponse::default();

        match result {
            Ok(output) => {
                response.status_code = FCPResponseStatus::FCPNoError;
                response.num_partition = output;
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        }

        info!("get_num_partition_for_fallback_post: finish");

        raw_ptr(response)
    })
}

/// TODO: document
///
#[no_mangle]
pub unsafe extern "C" fn cess_generate_single_window_post_with_vanilla(
    registered_proof: cess_RegisteredPoStProof,
    randomness: cess_32ByteArray,
    prover_id: cess_32ByteArray,
    vanilla_proofs_ptr: *const cess_VanillaProof,
    vanilla_proofs_len: libc::size_t,
    partition_index: libc::size_t,
) -> *mut cess_GenerateSingleWindowPoStWithVanillaResponse {
    catch_panic_response(|| {
        init_log();

        info!("generate_single_window_post_with_vanilla: start");

        let vanilla_proofs: Vec<VanillaProof> =
            std::slice::from_raw_parts(vanilla_proofs_ptr, vanilla_proofs_len)
                .iter()
                .cloned()
                .map(|vanilla_proof| {
                    std::slice::from_raw_parts(vanilla_proof.proof_ptr, vanilla_proof.proof_len)
                        .to_vec()
                })
                .collect();

        let result = cess_proving_system_api::post::generate_single_window_post_with_vanilla(
            registered_proof.into(),
            &randomness.inner,
            prover_id.inner,
            &vanilla_proofs,
            partition_index,
        );

        let mut response = cess_GenerateSingleWindowPoStWithVanillaResponse::default();

        match result {
            Ok(output) => {
                let partition_proof = cess_PartitionSnarkProof {
                    registered_proof,
                    proof_ptr: output.0.as_ptr(),
                    proof_len: output.0.len(),
                };
                mem::forget(output);

                response.status_code = FCPResponseStatus::FCPNoError;
                response.partition_proof = partition_proof;
            }
            Err(err) => {
                // If there were faulty sectors, add them to the response
                if let Some(StorageProofsError::FaultySectors(sectors)) =
                    err.downcast_ref::<StorageProofsError>()
                {
                    let sectors_u64 = sectors
                        .iter()
                        .map(|sector| u64::from(*sector))
                        .collect::<Vec<u64>>();
                    response.faulty_sectors_len = sectors_u64.len();
                    response.faulty_sectors_ptr = sectors_u64.as_ptr();
                    mem::forget(sectors_u64)
                }

                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        }

        info!("generate_single_window_post_with_vanilla: finish");

        raw_ptr(response)
    })
}

/// Returns the merkle root for a piece after piece padding and alignment.
/// The caller is responsible for closing the passed in file descriptor.
#[no_mangle]
#[cfg(not(target_os = "windows"))]
pub unsafe extern "C" fn cess_generate_piece_commitment(
    registered_proof: cess_RegisteredSealProof,
    piece_fd_raw: libc::c_int,
    unpadded_piece_size: u64,
) -> *mut cess_GeneratePieceCommitmentResponse {
    catch_panic_response(|| {
        init_log();

        use std::os::unix::io::{FromRawFd, IntoRawFd};

        let mut piece_file = std::fs::File::from_raw_fd(piece_fd_raw);

        let unpadded_piece_size = UnpaddedBytesAmount(unpadded_piece_size);
        let result = generate_piece_commitment(
            registered_proof.into(),
            &mut piece_file,
            unpadded_piece_size,
        );

        // avoid dropping the File which closes it
        let _ = piece_file.into_raw_fd();

        let mut response = cess_GeneratePieceCommitmentResponse::default();

        match result {
            Ok(meta) => {
                response.status_code = FCPResponseStatus::FCPNoError;
                response.comm_p = meta.commitment;
                response.num_bytes_aligned = meta.size.into();
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        }

        raw_ptr(response)
    })
}

/// Returns the merkle root for a sector containing the provided pieces.
#[no_mangle]
pub unsafe extern "C" fn cess_generate_data_commitment(
    registered_proof: cess_RegisteredSealProof,
    pieces_ptr: *const cess_PublicPieceInfo,
    pieces_len: libc::size_t,
) -> *mut cess_GenerateDataCommitmentResponse {
    catch_panic_response(|| {
        init_log();

        info!("generate_data_commitment: start");

        let public_pieces: Vec<PieceInfo> = std::slice::from_raw_parts(pieces_ptr, pieces_len)
            .to_vec()
            .iter()
            .cloned()
            .map(Into::into)
            .collect();

        let result = compute_comm_d(registered_proof.into(), &public_pieces);

        let mut response = cess_GenerateDataCommitmentResponse::default();

        match result {
            Ok(commitment) => {
                response.status_code = FCPResponseStatus::FCPNoError;
                response.comm_d = commitment;
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        }

        info!("generate_data_commitment: finish");

        raw_ptr(response)
    })
}

#[no_mangle]
pub unsafe extern "C" fn cess_clear_cache(
    sector_size: u64,
    cache_dir_path: *const libc::c_char,
) -> *mut cess_ClearCacheResponse {
    catch_panic_response(|| {
        init_log();

        let result = clear_cache(sector_size, &c_str_to_pbuf(cache_dir_path));

        let mut response = cess_ClearCacheResponse::default();

        match result {
            Ok(_) => {
                response.status_code = FCPResponseStatus::FCPNoError;
            }
            Err(err) => {
                response.status_code = FCPResponseStatus::FCPUnclassifiedError;
                response.error_msg = rust_str_to_c_str(format!("{:?}", err));
            }
        };

        raw_ptr(response)
    })
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_write_with_alignment_response(
    ptr: *mut cess_WriteWithAlignmentResponse,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_write_without_alignment_response(
    ptr: *mut cess_WriteWithoutAlignmentResponse,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_fauxrep_response(ptr: *mut cess_FauxRepResponse) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_seal_pre_commit_phase1_response(
    ptr: *mut cess_SealPreCommitPhase1Response,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_seal_pre_commit_phase2_response(
    ptr: *mut cess_SealPreCommitPhase2Response,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_seal_commit_phase1_response(
    ptr: *mut cess_SealCommitPhase1Response,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_seal_commit_phase2_response(
    ptr: *mut cess_SealCommitPhase2Response,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_unseal_range_response(ptr: *mut cess_UnsealRangeResponse) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_generate_piece_commitment_response(
    ptr: *mut cess_GeneratePieceCommitmentResponse,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_generate_data_commitment_response(
    ptr: *mut cess_GenerateDataCommitmentResponse,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_string_response(ptr: *mut cess_StringResponse) {
    let _ = Box::from_raw(ptr);
}

/// Returns the number of user bytes that will fit into a staged sector.
///
#[no_mangle]
pub unsafe extern "C" fn cess_get_max_user_bytes_per_staged_sector(
    registered_proof: cess_RegisteredSealProof,
) -> u64 {
    u64::from(UnpaddedBytesAmount::from(
        RegisteredSealProof::from(registered_proof).sector_size(),
    ))
}

/// Returns the CID of the Groth parameter file for sealing.
///
#[no_mangle]
pub unsafe extern "C" fn cess_get_seal_params_cid(
    registered_proof: cess_RegisteredSealProof,
) -> *mut cess_StringResponse {
    registered_seal_proof_accessor(registered_proof, RegisteredSealProof::params_cid)
}

/// Returns the CID of the verifying key-file for verifying a seal proof.
///
#[no_mangle]
pub unsafe extern "C" fn cess_get_seal_verifying_key_cid(
    registered_proof: cess_RegisteredSealProof,
) -> *mut cess_StringResponse {
    registered_seal_proof_accessor(registered_proof, RegisteredSealProof::verifying_key_cid)
}

/// Returns the path from which the proofs library expects to find the Groth
/// parameter file used when sealing.
///
#[no_mangle]
pub unsafe extern "C" fn cess_get_seal_params_path(
    registered_proof: cess_RegisteredSealProof,
) -> *mut cess_StringResponse {
    registered_seal_proof_accessor(registered_proof, |p| {
        p.cache_params_path()
            .map(|pb| String::from(pb.to_string_lossy()))
    })
}

/// Returns the path from which the proofs library expects to find the verifying
/// key-file used when verifying a seal proof.
///
#[no_mangle]
pub unsafe extern "C" fn cess_get_seal_verifying_key_path(
    registered_proof: cess_RegisteredSealProof,
) -> *mut cess_StringResponse {
    registered_seal_proof_accessor(registered_proof, |p| {
        p.cache_verifying_key_path()
            .map(|pb| String::from(pb.to_string_lossy()))
    })
}

/// Returns the identity of the circuit for the provided seal proof.
///
#[no_mangle]
pub unsafe extern "C" fn cess_get_seal_circuit_identifier(
    registered_proof: cess_RegisteredSealProof,
) -> *mut cess_StringResponse {
    registered_seal_proof_accessor(registered_proof, RegisteredSealProof::circuit_identifier)
}

/// Returns the version of the provided seal proof type.
///
#[no_mangle]
pub unsafe extern "C" fn cess_get_seal_version(
    registered_proof: cess_RegisteredSealProof,
) -> *mut cess_StringResponse {
    registered_seal_proof_accessor(registered_proof, |p| Ok(format!("{:?}", p)))
}

/// Returns the CID of the Groth parameter file for generating a PoSt.
///
#[no_mangle]
pub unsafe extern "C" fn cess_get_post_params_cid(
    registered_proof: cess_RegisteredPoStProof,
) -> *mut cess_StringResponse {
    registered_post_proof_accessor(registered_proof, RegisteredPoStProof::params_cid)
}

/// Returns the CID of the verifying key-file for verifying a PoSt proof.
///
#[no_mangle]
pub unsafe extern "C" fn cess_get_post_verifying_key_cid(
    registered_proof: cess_RegisteredPoStProof,
) -> *mut cess_StringResponse {
    registered_post_proof_accessor(registered_proof, RegisteredPoStProof::verifying_key_cid)
}

/// Returns the path from which the proofs library expects to find the Groth
/// parameter file used when generating a PoSt.
///
#[no_mangle]
pub unsafe extern "C" fn cess_get_post_params_path(
    registered_proof: cess_RegisteredPoStProof,
) -> *mut cess_StringResponse {
    registered_post_proof_accessor(registered_proof, |p| {
        p.cache_params_path()
            .map(|pb| String::from(pb.to_string_lossy()))
    })
}

/// Returns the path from which the proofs library expects to find the verifying
/// key-file used when verifying a PoSt proof.
///
#[no_mangle]
pub unsafe extern "C" fn cess_get_post_verifying_key_path(
    registered_proof: cess_RegisteredPoStProof,
) -> *mut cess_StringResponse {
    registered_post_proof_accessor(registered_proof, |p| {
        p.cache_verifying_key_path()
            .map(|pb| String::from(pb.to_string_lossy()))
    })
}

/// Returns the identity of the circuit for the provided PoSt proof type.
///
#[no_mangle]
pub unsafe extern "C" fn cess_get_post_circuit_identifier(
    registered_proof: cess_RegisteredPoStProof,
) -> *mut cess_StringResponse {
    registered_post_proof_accessor(registered_proof, RegisteredPoStProof::circuit_identifier)
}

/// Returns the version of the provided seal proof.
///
#[no_mangle]
pub unsafe extern "C" fn cess_get_post_version(
    registered_proof: cess_RegisteredPoStProof,
) -> *mut cess_StringResponse {
    registered_post_proof_accessor(registered_proof, |p| Ok(format!("{:?}", p)))
}

unsafe fn registered_seal_proof_accessor(
    registered_proof: cess_RegisteredSealProof,
    op: fn(RegisteredSealProof) -> anyhow::Result<String>,
) -> *mut cess_StringResponse {
    let mut response = cess_StringResponse::default();

    let rsp: RegisteredSealProof = registered_proof.into();

    match op(rsp) {
        Ok(s) => {
            response.status_code = FCPResponseStatus::FCPNoError;
            response.string_val = rust_str_to_c_str(s);
        }
        Err(err) => {
            response.status_code = FCPResponseStatus::FCPUnclassifiedError;
            response.error_msg = rust_str_to_c_str(format!("{:?}", err));
        }
    }

    raw_ptr(response)
}

unsafe fn registered_post_proof_accessor(
    registered_proof: cess_RegisteredPoStProof,
    op: fn(RegisteredPoStProof) -> anyhow::Result<String>,
) -> *mut cess_StringResponse {
    let mut response = cess_StringResponse::default();

    let rsp: RegisteredPoStProof = registered_proof.into();

    match op(rsp) {
        Ok(s) => {
            response.status_code = FCPResponseStatus::FCPNoError;
            response.string_val = rust_str_to_c_str(s);
        }
        Err(err) => {
            response.status_code = FCPResponseStatus::FCPUnclassifiedError;
            response.error_msg = rust_str_to_c_str(format!("{:?}", err));
        }
    }

    raw_ptr(response)
}

/// Deallocates a VerifySealResponse.
///
#[no_mangle]
pub unsafe extern "C" fn cess_destroy_verify_seal_response(ptr: *mut cess_VerifySealResponse) {
    let _ = Box::from_raw(ptr);
}

/// Deallocates a VerifyAggregateSealProofResponse.
///
#[no_mangle]
pub unsafe extern "C" fn cess_destroy_verify_aggregate_seal_response(
    ptr: *mut cess_VerifyAggregateSealProofResponse,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_finalize_ticket_response(
    ptr: *mut cess_FinalizeTicketResponse,
) {
    let _ = Box::from_raw(ptr);
}

/// Deallocates a VerifyPoStResponse.
///
#[no_mangle]
pub unsafe extern "C" fn cess_destroy_verify_winning_post_response(
    ptr: *mut cess_VerifyWinningPoStResponse,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_verify_window_post_response(
    ptr: *mut cess_VerifyWindowPoStResponse,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_generate_fallback_sector_challenges_response(
    ptr: *mut cess_GenerateFallbackSectorChallengesResponse,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_generate_single_vanilla_proof_response(
    ptr: *mut cess_GenerateSingleVanillaProofResponse,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_generate_single_window_post_with_vanilla_response(
    ptr: *mut cess_GenerateSingleWindowPoStWithVanillaResponse,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_get_num_partition_for_fallback_post_response(
    ptr: *mut cess_GetNumPartitionForFallbackPoStResponse,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_merge_window_post_partition_proofs_response(
    ptr: *mut cess_MergeWindowPoStPartitionProofsResponse,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_generate_winning_post_response(
    ptr: *mut cess_GenerateWinningPoStResponse,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_generate_window_post_response(
    ptr: *mut cess_GenerateWindowPoStResponse,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_generate_winning_post_sector_challenge(
    ptr: *mut cess_GenerateWinningPoStSectorChallenge,
) {
    let _ = Box::from_raw(ptr);
}

#[no_mangle]
pub unsafe extern "C" fn cess_destroy_clear_cache_response(ptr: *mut cess_ClearCacheResponse) {
    let _ = Box::from_raw(ptr);
}

/// Deallocates a AggregateProof
///
#[no_mangle]
pub unsafe extern "C" fn cess_destroy_aggregate_proof(ptr: *mut cess_AggregateProof) {
    let _ = Box::from_raw(ptr);
}

#[cfg(test)]
pub mod tests {
    use std::fs::remove_file;
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::os::unix::io::IntoRawFd;

    use anyhow::{ensure, Result};
    use ffi_toolkit::{c_str_to_rust_str, FCPResponseStatus};
    use rand::{thread_rng, Rng};

    use super::*;
    use std::ffi::CStr;

    #[test]
    fn test_write_with_and_without_alignment() -> Result<()> {
        let registered_proof = cess_RegisteredSealProof::StackedDrg2KiBV1;

        // write some bytes to a temp file to be used as the byte source
        let mut rng = thread_rng();
        let buf: Vec<u8> = (0..508).map(|_| rng.gen()).collect();

        // first temp file occupies 4 nodes in a merkle tree built over the
        // destination (after preprocessing)
        let mut src_file_a = tempfile::tempfile()?;
        src_file_a.write_all(&buf[0..127])?;
        src_file_a.seek(SeekFrom::Start(0))?;

        // second occupies 16 nodes
        let mut src_file_b = tempfile::tempfile()?;
        src_file_b.write_all(&buf[0..508])?;
        src_file_b.seek(SeekFrom::Start(0))?;

        // create a temp file to be used as the byte destination
        let dest = tempfile::tempfile()?;

        // transmute temp files to file descriptors
        let src_fd_a = src_file_a.into_raw_fd();
        let src_fd_b = src_file_b.into_raw_fd();
        let dst_fd = dest.into_raw_fd();

        // write the first file
        unsafe {
            let resp = cess_write_without_alignment(registered_proof, src_fd_a, 127, dst_fd);

            if (*resp).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp).error_msg);
                panic!("write_without_alignment failed: {:?}", msg);
            }

            assert_eq!(
                (*resp).total_write_unpadded,
                127,
                "should have added 127 bytes of (unpadded) left alignment"
            );
        }

        // write the second
        unsafe {
            let existing = vec![127u64];

            let resp = cess_write_with_alignment(
                registered_proof,
                src_fd_b,
                508,
                dst_fd,
                existing.as_ptr(),
                existing.len(),
            );

            if (*resp).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp).error_msg);
                panic!("write_with_alignment failed: {:?}", msg);
            }

            assert_eq!(
                (*resp).left_alignment_unpadded,
                381,
                "should have added 381 bytes of (unpadded) left alignment"
            );
        }

        Ok(())
    }

    #[test]
    fn test_proof_types() {
        let seal_types = vec![
            cess_RegisteredSealProof::StackedDrg2KiBV1,
            cess_RegisteredSealProof::StackedDrg8MiBV1,
            cess_RegisteredSealProof::StackedDrg512MiBV1,
            cess_RegisteredSealProof::StackedDrg32GiBV1,
            cess_RegisteredSealProof::StackedDrg64GiBV1,
            cess_RegisteredSealProof::StackedDrg2KiBV1_1,
            cess_RegisteredSealProof::StackedDrg8MiBV1_1,
            cess_RegisteredSealProof::StackedDrg512MiBV1_1,
            cess_RegisteredSealProof::StackedDrg32GiBV1_1,
            cess_RegisteredSealProof::StackedDrg64GiBV1_1,
        ];

        let post_types = vec![
            cess_RegisteredPoStProof::StackedDrgWinning2KiBV1,
            cess_RegisteredPoStProof::StackedDrgWinning8MiBV1,
            cess_RegisteredPoStProof::StackedDrgWinning512MiBV1,
            cess_RegisteredPoStProof::StackedDrgWinning32GiBV1,
            cess_RegisteredPoStProof::StackedDrgWinning64GiBV1,
            cess_RegisteredPoStProof::StackedDrgWindow2KiBV1,
            cess_RegisteredPoStProof::StackedDrgWindow8MiBV1,
            cess_RegisteredPoStProof::StackedDrgWindow512MiBV1,
            cess_RegisteredPoStProof::StackedDrgWindow32GiBV1,
            cess_RegisteredPoStProof::StackedDrgWindow64GiBV1,
        ];

        let num_ops = (seal_types.len() + post_types.len()) * 6;

        let mut pairs: Vec<(&str, *mut cess_StringResponse)> = Vec::with_capacity(num_ops);

        unsafe {
            for st in seal_types {
                pairs.push(("get_seal_params_cid", cess_get_seal_params_cid(st)));
                pairs.push((
                    "get_seal_verify_key_cid",
                    cess_get_seal_verifying_key_cid(st),
                ));
                pairs.push(("get_seal_verify_key_cid", cess_get_seal_params_path(st)));
                pairs.push((
                    "get_seal_verify_key_cid",
                    cess_get_seal_verifying_key_path(st),
                ));
                pairs.push((
                    "get_seal_circuit_identifier",
                    cess_get_seal_circuit_identifier(st),
                ));
                pairs.push(("get_seal_version", cess_get_seal_version(st)));
            }

            for pt in post_types {
                pairs.push(("get_post_params_cid", cess_get_post_params_cid(pt)));
                pairs.push((
                    "get_post_verify_key_cid",
                    cess_get_post_verifying_key_cid(pt),
                ));
                pairs.push(("get_post_params_path", cess_get_post_params_path(pt)));
                pairs.push((
                    "get_post_verifying_key_path",
                    cess_get_post_verifying_key_path(pt),
                ));
                pairs.push((
                    "get_post_circuit_identifier",
                    cess_get_post_circuit_identifier(pt),
                ));
                pairs.push(("get_post_version", cess_get_post_version(pt)));
            }
        }

        for (label, r) in pairs {
            unsafe {
                assert_eq!(
                    (*r).status_code,
                    FCPResponseStatus::FCPNoError,
                    "non-success exit code from {:?}: {:?}",
                    label,
                    c_str_to_rust_str((*r).error_msg)
                );

                let x = CStr::from_ptr((*r).string_val);
                let y = x.to_str().unwrap();

                assert!(!y.is_empty());

                cess_destroy_string_response(r);
            }
        }
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn test_sealing_v1() -> Result<()> {
        test_sealing_inner(cess_RegisteredSealProof::StackedDrg2KiBV1)
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn test_sealing_v1_1() -> Result<()> {
        test_sealing_inner(cess_RegisteredSealProof::StackedDrg2KiBV1_1)
    }

    fn test_sealing_inner(registered_proof_seal: cess_RegisteredSealProof) -> Result<()> {
        let wrap = |x| cess_32ByteArray { inner: x };

        // miscellaneous setup and shared values
        let registered_proof_winning_post = cess_RegisteredPoStProof::StackedDrgWinning2KiBV1;
        let registered_proof_window_post = cess_RegisteredPoStProof::StackedDrgWindow2KiBV1;

        let cache_dir = tempfile::tempdir()?;
        let cache_dir_path = cache_dir.into_path();

        let prover_id = cess_32ByteArray { inner: [1u8; 32] };
        let randomness = cess_32ByteArray { inner: [7u8; 32] };
        let sector_id = 42;
        let sector_id2 = 43;
        let seed = cess_32ByteArray { inner: [5u8; 32] };
        let ticket = cess_32ByteArray { inner: [6u8; 32] };

        // create a byte source (a user's piece)
        let mut rng = thread_rng();
        let buf_a: Vec<u8> = (0..2032).map(|_| rng.gen()).collect();

        let mut piece_file_a = tempfile::tempfile()?;
        piece_file_a.write_all(&buf_a[0..127])?;
        piece_file_a.seek(SeekFrom::Start(0))?;

        let mut piece_file_b = tempfile::tempfile()?;
        piece_file_b.write_all(&buf_a[0..1016])?;
        piece_file_b.seek(SeekFrom::Start(0))?;

        // create the staged sector (the byte destination)
        let (staged_file, staged_path) = tempfile::NamedTempFile::new()?.keep()?;

        // create a temp file to be used as the byte destination
        let (sealed_file, sealed_path) = tempfile::NamedTempFile::new()?.keep()?;

        // last temp file is used to output unsealed bytes
        let (unseal_file, unseal_path) = tempfile::NamedTempFile::new()?.keep()?;

        // transmute temp files to file descriptors
        let piece_file_a_fd = piece_file_a.into_raw_fd();
        let piece_file_b_fd = piece_file_b.into_raw_fd();
        let staged_sector_fd = staged_file.into_raw_fd();

        unsafe {
            let resp_a1 = cess_write_without_alignment(
                registered_proof_seal,
                piece_file_a_fd,
                127,
                staged_sector_fd,
            );

            if (*resp_a1).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_a1).error_msg);
                panic!("write_without_alignment failed: {:?}", msg);
            }

            let existing_piece_sizes = vec![127];

            let resp_a2 = cess_write_with_alignment(
                registered_proof_seal,
                piece_file_b_fd,
                1016,
                staged_sector_fd,
                existing_piece_sizes.as_ptr(),
                existing_piece_sizes.len(),
            );

            if (*resp_a2).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_a2).error_msg);
                panic!("write_with_alignment failed: {:?}", msg);
            }

            let pieces = vec![
                cess_PublicPieceInfo {
                    num_bytes: 127,
                    comm_p: (*resp_a1).comm_p,
                },
                cess_PublicPieceInfo {
                    num_bytes: 1016,
                    comm_p: (*resp_a2).comm_p,
                },
            ];

            let resp_x =
                cess_generate_data_commitment(registered_proof_seal, pieces.as_ptr(), pieces.len());

            if (*resp_x).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_x).error_msg);
                panic!("generate_data_commitment failed: {:?}", msg);
            }

            let cache_dir_path_c_str = rust_str_to_c_str(cache_dir_path.to_str().unwrap());
            let staged_path_c_str = rust_str_to_c_str(staged_path.to_str().unwrap());
            let replica_path_c_str = rust_str_to_c_str(sealed_path.to_str().unwrap());
            let unseal_path_c_str = rust_str_to_c_str(unseal_path.to_str().unwrap());

            let resp_b1 = cess_seal_pre_commit_phase1(
                registered_proof_seal,
                cache_dir_path_c_str,
                staged_path_c_str,
                replica_path_c_str,
                sector_id,
                prover_id,
                ticket,
                pieces.as_ptr(),
                pieces.len(),
            );

            if (*resp_b1).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_b1).error_msg);
                panic!("seal_pre_commit_phase1 failed: {:?}", msg);
            }

            let resp_b2 = cess_seal_pre_commit_phase2(
                (*resp_b1).seal_pre_commit_phase1_output_ptr,
                (*resp_b1).seal_pre_commit_phase1_output_len,
                cache_dir_path_c_str,
                replica_path_c_str,
            );

            if (*resp_b2).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_b2).error_msg);
                panic!("seal_pre_commit_phase2 failed: {:?}", msg);
            }

            let pre_computed_comm_d = &(*resp_x).comm_d;
            let pre_commit_comm_d = &(*resp_b2).comm_d;

            assert_eq!(
                format!("{:x?}", &pre_computed_comm_d),
                format!("{:x?}", &pre_commit_comm_d),
                "pre-computed CommD and pre-commit CommD don't match"
            );

            let resp_c1 = cess_seal_commit_phase1(
                registered_proof_seal,
                wrap((*resp_b2).comm_r),
                wrap((*resp_b2).comm_d),
                cache_dir_path_c_str,
                replica_path_c_str,
                sector_id,
                prover_id,
                ticket,
                seed,
                pieces.as_ptr(),
                pieces.len(),
            );

            if (*resp_c1).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_c1).error_msg);
                panic!("seal_commit_phase1 failed: {:?}", msg);
            }

            let resp_c2 = cess_seal_commit_phase2(
                (*resp_c1).seal_commit_phase1_output_ptr,
                (*resp_c1).seal_commit_phase1_output_len,
                sector_id,
                prover_id,
            );

            if (*resp_c2).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_c2).error_msg);
                panic!("seal_commit_phase2 failed: {:?}", msg);
            }

            let resp_d = cess_verify_seal(
                registered_proof_seal,
                wrap((*resp_b2).comm_r),
                wrap((*resp_b2).comm_d),
                prover_id,
                ticket,
                seed,
                sector_id,
                (*resp_c2).proof_ptr,
                (*resp_c2).proof_len,
            );

            if (*resp_d).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_d).error_msg);
                panic!("seal_commit failed: {:?}", msg);
            }

            assert!((*resp_d).is_valid, "proof was not valid");

            let resp_c22 = cess_seal_commit_phase2(
                (*resp_c1).seal_commit_phase1_output_ptr,
                (*resp_c1).seal_commit_phase1_output_len,
                sector_id,
                prover_id,
            );

            if (*resp_c22).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_c22).error_msg);
                panic!("seal_commit_phase2 failed: {:?}", msg);
            }

            let resp_d2 = cess_verify_seal(
                registered_proof_seal,
                wrap((*resp_b2).comm_r),
                wrap((*resp_b2).comm_d),
                prover_id,
                ticket,
                seed,
                sector_id,
                (*resp_c22).proof_ptr,
                (*resp_c22).proof_len,
            );

            if (*resp_d2).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_d2).error_msg);
                panic!("seal_commit failed: {:?}", msg);
            }

            assert!((*resp_d2).is_valid, "proof was not valid");

            let resp_e = cess_unseal_range(
                registered_proof_seal,
                cache_dir_path_c_str,
                sealed_file.into_raw_fd(),
                unseal_file.into_raw_fd(),
                sector_id,
                prover_id,
                ticket,
                wrap((*resp_b2).comm_d),
                0,
                2032,
            );

            if (*resp_e).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_e).error_msg);
                panic!("unseal failed: {:?}", msg);
            }

            // ensure unsealed bytes match what we had in our piece
            let mut buf_b = Vec::with_capacity(2032);
            let mut f = std::fs::File::open(&unseal_path)?;

            let _ = f.read_to_end(&mut buf_b)?;

            let piece_a_len = (*resp_a1).total_write_unpadded as usize;
            let piece_b_len = (*resp_a2).total_write_unpadded as usize;
            let piece_b_prefix_len = (*resp_a2).left_alignment_unpadded as usize;

            let alignment = vec![0; piece_b_prefix_len];

            let expected = [
                &buf_a[0..piece_a_len],
                &alignment[..],
                &buf_a[0..(piece_b_len - piece_b_prefix_len)],
            ]
            .concat();

            assert_eq!(
                format!("{:x?}", &expected),
                format!("{:x?}", &buf_b),
                "original bytes don't match unsealed bytes"
            );

            // generate a PoSt

            let sectors = vec![sector_id];
            let resp_f = cess_generate_winning_post_sector_challenge(
                registered_proof_winning_post,
                randomness,
                sectors.len() as u64,
                prover_id,
            );

            if (*resp_f).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_f).error_msg);
                panic!("generate_candidates failed: {:?}", msg);
            }

            // exercise the ticket-finalizing code path (but don't do anything
            // with the results
            let result: &[u64] = std::slice::from_raw_parts((*resp_f).ids_ptr, (*resp_f).ids_len);

            if result.is_empty() {
                panic!("generate_candidates produced no results");
            }

            let private_replicas = vec![cess_PrivateReplicaInfo {
                registered_proof: registered_proof_winning_post,
                cache_dir_path: cache_dir_path_c_str,
                comm_r: (*resp_b2).comm_r,
                replica_path: replica_path_c_str,
                sector_id,
            }];

            // winning post

            let resp_h = cess_generate_winning_post(
                randomness,
                private_replicas.as_ptr(),
                private_replicas.len(),
                prover_id,
            );

            if (*resp_h).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_h).error_msg);
                panic!("generate_winning_post failed: {:?}", msg);
            }
            let public_replicas = vec![cess_PublicReplicaInfo {
                registered_proof: registered_proof_winning_post,
                sector_id,
                comm_r: (*resp_b2).comm_r,
            }];

            let resp_i = cess_verify_winning_post(
                randomness,
                public_replicas.as_ptr(),
                public_replicas.len(),
                (*resp_h).proofs_ptr,
                (*resp_h).proofs_len,
                prover_id,
            );

            if (*resp_i).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_i).error_msg);
                panic!("verify_winning_post failed: {:?}", msg);
            }

            if !(*resp_i).is_valid {
                panic!("verify_winning_post rejected the provided proof as invalid");
            }

            //////////////////////////////////////////////
            // Winning PoSt using distributed API
            //
            // NOTE: This performs the winning post all over again, just using
            // a different API.  This is just for testing and would not normally
            // be repeated like this in sequence.
            //
            //////////////////////////////////////////////

            // First generate sector challenges.
            let resp_sc = cess_generate_fallback_sector_challenges(
                registered_proof_winning_post,
                randomness,
                sectors.as_ptr(),
                sectors.len(),
                prover_id,
            );

            if (*resp_sc).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_sc).error_msg);
                panic!("fallback_sector_challenges failed: {:?}", msg);
            }

            let sector_ids: Vec<u64> =
                std::slice::from_raw_parts((*resp_sc).ids_ptr, (*resp_sc).ids_len).to_vec();
            let sector_challenges: Vec<u64> =
                std::slice::from_raw_parts((*resp_sc).challenges_ptr, (*resp_sc).challenges_len)
                    .to_vec();
            let challenges_stride = (*resp_sc).challenges_stride;
            let challenge_iterations = sector_challenges.len() / challenges_stride;
            assert_eq!(
                sector_ids.len(),
                challenge_iterations,
                "Challenge iterations must match the number of sector ids"
            );

            let mut vanilla_proofs: Vec<cess_VanillaProof> = Vec::with_capacity(sector_ids.len());

            // Gather up all vanilla proofs.
            for i in 0..challenge_iterations {
                let sector_id = sector_ids[i];
                let challenges: Vec<_> = sector_challenges
                    [i * challenges_stride..i * challenges_stride + challenges_stride]
                    .to_vec();
                let private_replica = private_replicas
                    .iter()
                    .find(|&replica| replica.sector_id == sector_id)
                    .expect("failed to find private replica info")
                    .clone();

                let resp_vp = cess_generate_single_vanilla_proof(
                    private_replica,
                    challenges.as_ptr(),
                    challenges.len(),
                );

                if (*resp_vp).status_code != FCPResponseStatus::FCPNoError {
                    let msg = c_str_to_rust_str((*resp_vp).error_msg);
                    panic!("generate_single_vanilla_proof failed: {:?}", msg);
                }

                vanilla_proofs.push((*resp_vp).vanilla_proof.clone());
                cess_destroy_generate_single_vanilla_proof_response(resp_vp);
            }

            let resp_wpwv = cess_generate_winning_post_with_vanilla(
                registered_proof_winning_post,
                randomness,
                prover_id,
                vanilla_proofs.as_ptr(),
                vanilla_proofs.len(),
            );

            if (*resp_wpwv).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_wpwv).error_msg);
                panic!("generate_winning_post_with_vanilla failed: {:?}", msg);
            }

            // Verify the second winning post (generated by the
            // distributed post API)
            let resp_di = cess_verify_winning_post(
                randomness,
                public_replicas.as_ptr(),
                public_replicas.len(),
                (*resp_wpwv).proofs_ptr,
                (*resp_wpwv).proofs_len,
                prover_id,
            );

            if (*resp_di).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_di).error_msg);
                panic!("verify_winning_post failed: {:?}", msg);
            }

            if !(*resp_di).is_valid {
                panic!("verify_winning_post rejected the provided proof as invalid");
            }

            // window post

            let private_replicas = vec![cess_PrivateReplicaInfo {
                registered_proof: registered_proof_window_post,
                cache_dir_path: cache_dir_path_c_str,
                comm_r: (*resp_b2).comm_r,
                replica_path: replica_path_c_str,
                sector_id,
            }];

            let resp_j = cess_generate_window_post(
                randomness,
                private_replicas.as_ptr(),
                private_replicas.len(),
                prover_id,
            );

            if (*resp_j).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_j).error_msg);
                panic!("generate_window_post failed: {:?}", msg);
            }

            let public_replicas = vec![cess_PublicReplicaInfo {
                registered_proof: registered_proof_window_post,
                sector_id,
                comm_r: (*resp_b2).comm_r,
            }];

            let resp_k = cess_verify_window_post(
                randomness,
                public_replicas.as_ptr(),
                public_replicas.len(),
                (*resp_j).proofs_ptr,
                (*resp_j).proofs_len,
                prover_id,
            );

            if (*resp_k).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_k).error_msg);
                panic!("verify_window_post failed: {:?}", msg);
            }

            if !(*resp_k).is_valid {
                panic!("verify_window_post rejected the provided proof as invalid");
            }

            //////////////////////////////////////////////
            // Window PoSt using distributed API
            //
            // NOTE: This performs the window post all over again, just using
            // a different API.  This is just for testing and would not normally
            // be repeated like this in sequence.
            //
            //////////////////////////////////////////////

            let sectors = vec![sector_id, sector_id2];
            let private_replicas = vec![
                cess_PrivateReplicaInfo {
                    registered_proof: registered_proof_window_post,
                    cache_dir_path: cache_dir_path_c_str,
                    comm_r: (*resp_b2).comm_r,
                    replica_path: replica_path_c_str,
                    sector_id,
                },
                cess_PrivateReplicaInfo {
                    registered_proof: registered_proof_window_post,
                    cache_dir_path: cache_dir_path_c_str,
                    comm_r: (*resp_b2).comm_r,
                    replica_path: replica_path_c_str,
                    sector_id: sector_id2,
                },
            ];
            let public_replicas = vec![
                cess_PublicReplicaInfo {
                    registered_proof: registered_proof_window_post,
                    sector_id,
                    comm_r: (*resp_b2).comm_r,
                },
                cess_PublicReplicaInfo {
                    registered_proof: registered_proof_window_post,
                    sector_id: sector_id2,
                    comm_r: (*resp_b2).comm_r,
                },
            ];

            // Generate sector challenges.
            let resp_sc2 = cess_generate_fallback_sector_challenges(
                registered_proof_window_post,
                randomness,
                sectors.as_ptr(),
                sectors.len(),
                prover_id,
            );

            if (*resp_sc2).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_sc2).error_msg);
                panic!("fallback_sector_challenges failed: {:?}", msg);
            }

            let sector_ids: Vec<u64> =
                std::slice::from_raw_parts((*resp_sc2).ids_ptr, (*resp_sc2).ids_len).to_vec();
            let sector_challenges: Vec<u64> =
                std::slice::from_raw_parts((*resp_sc2).challenges_ptr, (*resp_sc2).challenges_len)
                    .to_vec();
            let challenges_stride = (*resp_sc2).challenges_stride;
            let challenge_iterations = sector_challenges.len() / challenges_stride;
            assert_eq!(
                sector_ids.len(),
                challenge_iterations,
                "Challenge iterations must match the number of sector ids"
            );

            let mut vanilla_proofs: Vec<cess_VanillaProof> = Vec::with_capacity(sector_ids.len());

            // Gather up all vanilla proofs.
            for i in 0..challenge_iterations {
                let sector_id = sector_ids[i];
                let challenges: Vec<_> = sector_challenges
                    [i * challenges_stride..i * challenges_stride + challenges_stride]
                    .to_vec();

                let private_replica = private_replicas
                    .iter()
                    .find(|&replica| replica.sector_id == sector_id)
                    .expect("failed to find private replica info")
                    .clone();

                let resp_vp = cess_generate_single_vanilla_proof(
                    private_replica,
                    challenges.as_ptr(),
                    challenges.len(),
                );

                if (*resp_vp).status_code != FCPResponseStatus::FCPNoError {
                    let msg = c_str_to_rust_str((*resp_vp).error_msg);
                    panic!("generate_single_vanilla_proof failed: {:?}", msg);
                }

                vanilla_proofs.push((*resp_vp).vanilla_proof.clone());
                cess_destroy_generate_single_vanilla_proof_response(resp_vp);
            }

            let resp_wpwv2 = cess_generate_window_post_with_vanilla(
                registered_proof_window_post,
                randomness,
                prover_id,
                vanilla_proofs.as_ptr(),
                vanilla_proofs.len(),
            );

            if (*resp_wpwv2).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_wpwv2).error_msg);
                panic!("generate_window_post_with_vanilla failed: {:?}", msg);
            }

            let resp_k2 = cess_verify_window_post(
                randomness,
                public_replicas.as_ptr(),
                public_replicas.len(),
                (*resp_wpwv2).proofs_ptr,
                (*resp_wpwv2).proofs_len,
                prover_id,
            );

            if (*resp_k2).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_k2).error_msg);
                panic!("verify_window_post failed: {:?}", msg);
            }

            if !(*resp_k2).is_valid {
                panic!("verify_window_post rejected the provided proof as invalid");
            }

            //////////////////////////////////////////////
            // Window PoSt using single partition API
            //
            // NOTE: This performs the window post all over again, just using
            // a different API.  This is just for testing and would not normally
            // be repeated like this in sequence.
            //
            //////////////////////////////////////////////

            // Note: Re-using all of the sector challenges and types
            // required from above previous distributed PoSt API run.

            let num_partitions_resp = cess_get_num_partition_for_fallback_post(
                registered_proof_window_post,
                sectors.len(),
            );
            if (*num_partitions_resp).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*num_partitions_resp).error_msg);
                panic!("get_num_partition_for_fallback_post failed: {:?}", msg);
            }

            let mut partition_proofs: Vec<cess_PartitionSnarkProof> =
                Vec::with_capacity((*num_partitions_resp).num_partition);
            for partition_index in 0..(*num_partitions_resp).num_partition {
                let mut vanilla_proofs = Vec::with_capacity(challenge_iterations);
                for i in 0..challenge_iterations {
                    let sector_id = sector_ids[i];
                    let challenges: Vec<_> = sector_challenges
                        [i * challenges_stride..i * challenges_stride + challenges_stride]
                        .to_vec();

                    let private_replica = private_replicas
                        .iter()
                        .find(|&replica| replica.sector_id == sector_id)
                        .expect("failed to find private replica info")
                        .clone();

                    let resp_vp = cess_generate_single_vanilla_proof(
                        private_replica,
                        challenges.as_ptr(),
                        challenges.len(),
                    );

                    if (*resp_vp).status_code != FCPResponseStatus::FCPNoError {
                        let msg = c_str_to_rust_str((*resp_vp).error_msg);
                        panic!("generate_single_vanilla_proof failed: {:?}", msg);
                    }

                    vanilla_proofs.push((*resp_vp).vanilla_proof.clone());
                    cess_destroy_generate_single_vanilla_proof_response(resp_vp);
                }

                let single_partition_proof_resp = cess_generate_single_window_post_with_vanilla(
                    registered_proof_window_post,
                    randomness,
                    prover_id,
                    vanilla_proofs.as_ptr(),
                    vanilla_proofs.len(),
                    partition_index,
                );

                if (*single_partition_proof_resp).status_code != FCPResponseStatus::FCPNoError {
                    let msg = c_str_to_rust_str((*single_partition_proof_resp).error_msg);
                    panic!("generate_single_window_post_with_vanilla failed: {:?}", msg);
                }

                partition_proofs.push((*single_partition_proof_resp).partition_proof.clone());
                cess_destroy_generate_single_window_post_with_vanilla_response(
                    single_partition_proof_resp,
                );
            }

            cess_destroy_get_num_partition_for_fallback_post_response(num_partitions_resp);

            let merged_proof_resp = cess_merge_window_post_partition_proofs(
                registered_proof_window_post,
                partition_proofs.as_ptr(),
                partition_proofs.len(),
            );

            if (*merged_proof_resp).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*merged_proof_resp).error_msg);
                panic!("merge_window_post_partition_proofs failed: {:?}", msg);
            }

            let resp_k3 = cess_verify_window_post(
                randomness,
                public_replicas.as_ptr(),
                public_replicas.len(),
                &(*merged_proof_resp).proof,
                1, /* len is 1, as it's a single window post proof once merged */
                prover_id,
            );

            if (*resp_k3).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_k3).error_msg);
                panic!("verify_window_post failed: {:?}", msg);
            }

            if !(*resp_k3).is_valid {
                panic!("verify_window_post rejected the provided proof as invalid");
            }

            cess_destroy_merge_window_post_partition_proofs_response(merged_proof_resp);

            ////////////////////
            // Cleanup responses
            ////////////////////

            cess_destroy_write_without_alignment_response(resp_a1);
            cess_destroy_write_with_alignment_response(resp_a2);
            cess_destroy_generate_data_commitment_response(resp_x);

            cess_destroy_seal_pre_commit_phase1_response(resp_b1);
            cess_destroy_seal_pre_commit_phase2_response(resp_b2);
            cess_destroy_seal_commit_phase1_response(resp_c1);
            cess_destroy_seal_commit_phase2_response(resp_c2);

            cess_destroy_verify_seal_response(resp_d);
            cess_destroy_unseal_range_response(resp_e);

            cess_destroy_generate_winning_post_sector_challenge(resp_f);
            cess_destroy_generate_fallback_sector_challenges_response(resp_sc);
            cess_destroy_generate_winning_post_response(resp_h);
            cess_destroy_generate_winning_post_response(resp_wpwv);
            cess_destroy_verify_winning_post_response(resp_i);
            cess_destroy_verify_winning_post_response(resp_di);

            cess_destroy_generate_fallback_sector_challenges_response(resp_sc2);
            cess_destroy_generate_window_post_response(resp_j);
            cess_destroy_generate_window_post_response(resp_wpwv2);
            cess_destroy_verify_window_post_response(resp_k);
            cess_destroy_verify_window_post_response(resp_k2);
            cess_destroy_verify_window_post_response(resp_k3);

            c_str_to_rust_str(cache_dir_path_c_str);
            c_str_to_rust_str(staged_path_c_str);
            c_str_to_rust_str(replica_path_c_str);
            c_str_to_rust_str(unseal_path_c_str);

            ensure!(
                remove_file(&staged_path).is_ok(),
                "failed to remove staged_path"
            );
            ensure!(
                remove_file(&sealed_path).is_ok(),
                "failed to remove sealed_path"
            );
            ensure!(
                remove_file(&unseal_path).is_ok(),
                "failed to remove unseal_path"
            );
        }

        Ok(())
    }

    #[test]
    fn test_faulty_sectors_v1() -> Result<()> {
        test_faulty_sectors_inner(cess_RegisteredSealProof::StackedDrg2KiBV1)
    }

    #[test]
    fn test_faulty_sectors_v1_1() -> Result<()> {
        test_faulty_sectors_inner(cess_RegisteredSealProof::StackedDrg2KiBV1_1)
    }

    fn test_faulty_sectors_inner(registered_proof_seal: cess_RegisteredSealProof) -> Result<()> {
        // miscellaneous setup and shared values
        let registered_proof_window_post = cess_RegisteredPoStProof::StackedDrgWindow2KiBV1;

        let cache_dir = tempfile::tempdir()?;
        let cache_dir_path = cache_dir.into_path();

        let prover_id = cess_32ByteArray { inner: [1u8; 32] };
        let randomness = cess_32ByteArray { inner: [7u8; 32] };
        let sector_id = 42;
        let ticket = cess_32ByteArray { inner: [6u8; 32] };

        // create a byte source (a user's piece)
        let mut rng = thread_rng();
        let buf_a: Vec<u8> = (0..2032).map(|_| rng.gen()).collect();

        let mut piece_file_a = tempfile::tempfile()?;
        piece_file_a.write_all(&buf_a[0..127])?;
        piece_file_a.seek(SeekFrom::Start(0))?;

        let mut piece_file_b = tempfile::tempfile()?;
        piece_file_b.write_all(&buf_a[0..1016])?;
        piece_file_b.seek(SeekFrom::Start(0))?;

        // create the staged sector (the byte destination)
        let (staged_file, staged_path) = tempfile::NamedTempFile::new()?.keep()?;

        // create a temp file to be used as the byte destination
        let (_sealed_file, sealed_path) = tempfile::NamedTempFile::new()?.keep()?;

        // transmute temp files to file descriptors
        let piece_file_a_fd = piece_file_a.into_raw_fd();
        let piece_file_b_fd = piece_file_b.into_raw_fd();
        let staged_sector_fd = staged_file.into_raw_fd();

        unsafe {
            let resp_a1 = cess_write_without_alignment(
                registered_proof_seal,
                piece_file_a_fd,
                127,
                staged_sector_fd,
            );

            if (*resp_a1).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_a1).error_msg);
                panic!("write_without_alignment failed: {:?}", msg);
            }

            let existing_piece_sizes = vec![127];

            let resp_a2 = cess_write_with_alignment(
                registered_proof_seal,
                piece_file_b_fd,
                1016,
                staged_sector_fd,
                existing_piece_sizes.as_ptr(),
                existing_piece_sizes.len(),
            );

            if (*resp_a2).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_a2).error_msg);
                panic!("write_with_alignment failed: {:?}", msg);
            }

            let pieces = vec![
                cess_PublicPieceInfo {
                    num_bytes: 127,
                    comm_p: (*resp_a1).comm_p,
                },
                cess_PublicPieceInfo {
                    num_bytes: 1016,
                    comm_p: (*resp_a2).comm_p,
                },
            ];

            let resp_x =
                cess_generate_data_commitment(registered_proof_seal, pieces.as_ptr(), pieces.len());

            if (*resp_x).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_x).error_msg);
                panic!("generate_data_commitment failed: {:?}", msg);
            }

            let cache_dir_path_c_str = rust_str_to_c_str(cache_dir_path.to_str().unwrap());
            let staged_path_c_str = rust_str_to_c_str(staged_path.to_str().unwrap());
            let replica_path_c_str = rust_str_to_c_str(sealed_path.to_str().unwrap());

            let resp_b1 = cess_seal_pre_commit_phase1(
                registered_proof_seal,
                cache_dir_path_c_str,
                staged_path_c_str,
                replica_path_c_str,
                sector_id,
                prover_id,
                ticket,
                pieces.as_ptr(),
                pieces.len(),
            );

            if (*resp_b1).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_b1).error_msg);
                panic!("seal_pre_commit_phase1 failed: {:?}", msg);
            }

            let resp_b2 = cess_seal_pre_commit_phase2(
                (*resp_b1).seal_pre_commit_phase1_output_ptr,
                (*resp_b1).seal_pre_commit_phase1_output_len,
                cache_dir_path_c_str,
                replica_path_c_str,
            );

            if (*resp_b2).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_b2).error_msg);
                panic!("seal_pre_commit_phase2 failed: {:?}", msg);
            }

            // window post

            let faulty_sealed_file = tempfile::NamedTempFile::new()?;
            let faulty_replica_path_c_str =
                rust_str_to_c_str(faulty_sealed_file.path().to_str().unwrap());

            let private_replicas = vec![cess_PrivateReplicaInfo {
                registered_proof: registered_proof_window_post,
                cache_dir_path: cache_dir_path_c_str,
                comm_r: (*resp_b2).comm_r,
                replica_path: faulty_replica_path_c_str,
                sector_id,
            }];

            let resp_j = cess_generate_window_post(
                randomness,
                private_replicas.as_ptr(),
                private_replicas.len(),
                prover_id,
            );

            assert_eq!(
                (*resp_j).status_code,
                FCPResponseStatus::FCPUnclassifiedError,
                "generate_window_post should have failed"
            );

            let faulty_sectors: &[u64] = std::slice::from_raw_parts(
                (*resp_j).faulty_sectors_ptr,
                (*resp_j).faulty_sectors_len,
            );
            assert_eq!(faulty_sectors, &[42], "sector 42 should be faulty");

            cess_destroy_write_without_alignment_response(resp_a1);
            cess_destroy_write_with_alignment_response(resp_a2);

            cess_destroy_seal_pre_commit_phase1_response(resp_b1);
            cess_destroy_seal_pre_commit_phase2_response(resp_b2);

            cess_destroy_generate_window_post_response(resp_j);

            c_str_to_rust_str(cache_dir_path_c_str);
            c_str_to_rust_str(staged_path_c_str);
            c_str_to_rust_str(replica_path_c_str);

            ensure!(
                remove_file(&staged_path).is_ok(),
                "failed to remove staged_path"
            );
            ensure!(
                remove_file(&sealed_path).is_ok(),
                "failed to remove sealed_path"
            );
        }

        Ok(())
    }

    #[test]
    #[ignore]
    fn test_sealing_aggregation_v1() -> Result<()> {
        test_sealing_aggregation(
            cess_RegisteredSealProof::StackedDrg2KiBV1,
            cess_RegisteredAggregationProof::SnarkPackV1,
        )
    }

    #[test]
    #[ignore]
    fn test_sealing_aggregation_v1_1() -> Result<()> {
        test_sealing_aggregation(
            cess_RegisteredSealProof::StackedDrg2KiBV1_1,
            cess_RegisteredAggregationProof::SnarkPackV1,
        )
    }

    fn test_sealing_aggregation(
        registered_proof_seal: cess_RegisteredSealProof,
        registered_aggregation: cess_RegisteredAggregationProof,
    ) -> Result<()> {
        let wrap = |x| cess_32ByteArray { inner: x };

        // miscellaneous setup and shared values
        let cache_dir = tempfile::tempdir()?;
        let cache_dir_path = cache_dir.into_path();

        let prover_id = cess_32ByteArray { inner: [1u8; 32] };
        let sector_id = 42;
        let seed = cess_32ByteArray { inner: [5u8; 32] };
        let ticket = cess_32ByteArray { inner: [6u8; 32] };

        // create a byte source (a user's piece)
        let mut rng = thread_rng();
        let buf_a: Vec<u8> = (0..2032).map(|_| rng.gen()).collect();

        let mut piece_file_a = tempfile::tempfile()?;
        piece_file_a.write_all(&buf_a[0..127])?;
        piece_file_a.seek(SeekFrom::Start(0))?;

        let mut piece_file_b = tempfile::tempfile()?;
        piece_file_b.write_all(&buf_a[0..1016])?;
        piece_file_b.seek(SeekFrom::Start(0))?;

        // create the staged sector (the byte destination)
        let (staged_file, staged_path) = tempfile::NamedTempFile::new()?.keep()?;

        // create a temp file to be used as the byte destination
        let (_sealed_file, sealed_path) = tempfile::NamedTempFile::new()?.keep()?;

        // transmute temp files to file descriptors
        let piece_file_a_fd = piece_file_a.into_raw_fd();
        let piece_file_b_fd = piece_file_b.into_raw_fd();
        let staged_sector_fd = staged_file.into_raw_fd();

        unsafe {
            let resp_a1 = cess_write_without_alignment(
                registered_proof_seal,
                piece_file_a_fd,
                127,
                staged_sector_fd,
            );

            if (*resp_a1).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_a1).error_msg);
                panic!("write_without_alignment failed: {:?}", msg);
            }

            let existing_piece_sizes = vec![127];

            let resp_a2 = cess_write_with_alignment(
                registered_proof_seal,
                piece_file_b_fd,
                1016,
                staged_sector_fd,
                existing_piece_sizes.as_ptr(),
                existing_piece_sizes.len(),
            );

            if (*resp_a2).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_a2).error_msg);
                panic!("write_with_alignment failed: {:?}", msg);
            }

            let pieces = vec![
                cess_PublicPieceInfo {
                    num_bytes: 127,
                    comm_p: (*resp_a1).comm_p,
                },
                cess_PublicPieceInfo {
                    num_bytes: 1016,
                    comm_p: (*resp_a2).comm_p,
                },
            ];

            let resp_x =
                cess_generate_data_commitment(registered_proof_seal, pieces.as_ptr(), pieces.len());

            if (*resp_x).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_x).error_msg);
                panic!("generate_data_commitment failed: {:?}", msg);
            }

            let cache_dir_path_c_str = rust_str_to_c_str(cache_dir_path.to_str().unwrap());
            let staged_path_c_str = rust_str_to_c_str(staged_path.to_str().unwrap());
            let replica_path_c_str = rust_str_to_c_str(sealed_path.to_str().unwrap());

            let resp_b1 = cess_seal_pre_commit_phase1(
                registered_proof_seal,
                cache_dir_path_c_str,
                staged_path_c_str,
                replica_path_c_str,
                sector_id,
                prover_id,
                ticket,
                pieces.as_ptr(),
                pieces.len(),
            );

            if (*resp_b1).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_b1).error_msg);
                panic!("seal_pre_commit_phase1 failed: {:?}", msg);
            }

            let resp_b2 = cess_seal_pre_commit_phase2(
                (*resp_b1).seal_pre_commit_phase1_output_ptr,
                (*resp_b1).seal_pre_commit_phase1_output_len,
                cache_dir_path_c_str,
                replica_path_c_str,
            );

            if (*resp_b2).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_b2).error_msg);
                panic!("seal_pre_commit_phase2 failed: {:?}", msg);
            }

            let pre_computed_comm_d = &(*resp_x).comm_d;
            let pre_commit_comm_d = &(*resp_b2).comm_d;

            assert_eq!(
                format!("{:x?}", &pre_computed_comm_d),
                format!("{:x?}", &pre_commit_comm_d),
                "pre-computed CommD and pre-commit CommD don't match"
            );

            let resp_c1 = cess_seal_commit_phase1(
                registered_proof_seal,
                wrap((*resp_b2).comm_r),
                wrap((*resp_b2).comm_d),
                cache_dir_path_c_str,
                replica_path_c_str,
                sector_id,
                prover_id,
                ticket,
                seed,
                pieces.as_ptr(),
                pieces.len(),
            );

            if (*resp_c1).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_c1).error_msg);
                panic!("seal_commit_phase1 failed: {:?}", msg);
            }

            let resp_c2 = cess_seal_commit_phase2(
                (*resp_c1).seal_commit_phase1_output_ptr,
                (*resp_c1).seal_commit_phase1_output_len,
                sector_id,
                prover_id,
            );

            if (*resp_c2).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_c2).error_msg);
                panic!("seal_commit_phase2 failed: {:?}", msg);
            }

            let resp_d = cess_verify_seal(
                registered_proof_seal,
                wrap((*resp_b2).comm_r),
                wrap((*resp_b2).comm_d),
                prover_id,
                ticket,
                seed,
                sector_id,
                (*resp_c2).proof_ptr,
                (*resp_c2).proof_len,
            );

            if (*resp_d).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_d).error_msg);
                panic!("seal_commit failed: {:?}", msg);
            }

            assert!((*resp_d).is_valid, "proof was not valid");

            let resp_c22 = cess_seal_commit_phase2(
                (*resp_c1).seal_commit_phase1_output_ptr,
                (*resp_c1).seal_commit_phase1_output_len,
                sector_id,
                prover_id,
            );

            if (*resp_c22).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_c22).error_msg);
                panic!("seal_commit_phase2 failed: {:?}", msg);
            }

            let resp_d2 = cess_verify_seal(
                registered_proof_seal,
                wrap((*resp_b2).comm_r),
                wrap((*resp_b2).comm_d),
                prover_id,
                ticket,
                seed,
                sector_id,
                (*resp_c22).proof_ptr,
                (*resp_c22).proof_len,
            );

            if (*resp_d2).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_d2).error_msg);
                panic!("seal_commit failed: {:?}", msg);
            }

            assert!((*resp_d2).is_valid, "proof was not valid");

            let seal_commit_responses: Vec<cess_SealCommitPhase2Response> =
                vec![(*resp_c2).clone(), (*resp_c22).clone()];

            let comm_rs = vec![
                cess_32ByteArray {
                    inner: (*resp_b2).comm_r,
                },
                cess_32ByteArray {
                    inner: (*resp_b2).comm_r,
                },
            ];
            let seeds = vec![seed, seed];
            let resp_aggregate_proof = cess_aggregate_seal_proofs(
                registered_proof_seal,
                registered_aggregation,
                comm_rs.as_ptr(),
                comm_rs.len(),
                seeds.as_ptr(),
                seeds.len(),
                seal_commit_responses.as_ptr(),
                seal_commit_responses.len(),
            );

            if (*resp_aggregate_proof).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_aggregate_proof).error_msg);
                panic!("aggregate_seal_proofs failed: {:?}", msg);
            }

            let mut inputs: Vec<cess_AggregationInputs> = vec![
                cess_AggregationInputs {
                    comm_r: wrap((*resp_b2).comm_r),
                    comm_d: wrap((*resp_b2).comm_d),
                    sector_id,
                    ticket,
                    seed,
                },
                cess_AggregationInputs {
                    comm_r: wrap((*resp_b2).comm_r),
                    comm_d: wrap((*resp_b2).comm_d),
                    sector_id,
                    ticket,
                    seed,
                },
            ];

            let resp_ad = cess_verify_aggregate_seal_proof(
                registered_proof_seal,
                registered_aggregation,
                prover_id,
                (*resp_aggregate_proof).proof_ptr,
                (*resp_aggregate_proof).proof_len,
                inputs.as_mut_ptr(),
                inputs.len(),
            );

            if (*resp_ad).status_code != FCPResponseStatus::FCPNoError {
                let msg = c_str_to_rust_str((*resp_ad).error_msg);
                panic!("verify_aggregate_seal_proof failed: {:?}", msg);
            }

            assert!((*resp_ad).is_valid, "aggregated proof was not valid");

            cess_destroy_write_without_alignment_response(resp_a1);
            cess_destroy_write_with_alignment_response(resp_a2);
            cess_destroy_generate_data_commitment_response(resp_x);

            cess_destroy_seal_pre_commit_phase1_response(resp_b1);
            cess_destroy_seal_pre_commit_phase2_response(resp_b2);
            cess_destroy_seal_commit_phase1_response(resp_c1);

            cess_destroy_seal_commit_phase2_response(resp_c2);
            cess_destroy_seal_commit_phase2_response(resp_c22);

            cess_destroy_verify_seal_response(resp_d);
            cess_destroy_verify_seal_response(resp_d2);

            cess_destroy_verify_aggregate_seal_response(resp_ad);

            //cess_destroy_aggregation_inputs_response(resp_c2_inputs);
            //cess_destroy_aggregation_inputs_response(resp_c22_inputs);

            cess_destroy_aggregate_proof(resp_aggregate_proof);

            c_str_to_rust_str(cache_dir_path_c_str);
            c_str_to_rust_str(staged_path_c_str);
            c_str_to_rust_str(replica_path_c_str);

            ensure!(
                remove_file(&staged_path).is_ok(),
                "failed to remove staged_path"
            );
            ensure!(
                remove_file(&sealed_path).is_ok(),
                "failed to remove sealed_path"
            );
        }

        Ok(())
    }
}