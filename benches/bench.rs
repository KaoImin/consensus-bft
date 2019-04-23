extern crate consensus_bft as BFT;
#[macro_use]
extern crate criterion;

use crate::types::*;
use bincode::serialize;
use criterion::{Benchmark, Criterion};
use crossbeam::crossbeam_channel::unbounded;
use rand::random;
use rlp::Encodable;
use std::collections::HashMap;
use BFT::*;

#[inline(always)]
fn bench_proof() -> Vec<u8> {
    let mut hash = Vec::new();
    for _i in 0..256 {
        hash.push(1);
    }
    hash
}

#[inline(always)]
fn gen_mb() -> Vec<u8> {
    let mb: Vec<u8> = (0..1024 * 1024 * 3).map(|_| random::<u8>()).collect();
    mb
}

fn bench_bincode(content: Vec<u8>) {
    let proof = Proof {
        block_hash: bench_proof(),
        height: 0,
        round: 0,
        precommit_votes: HashMap::new(),
    };

    let proposal = Proposal {
        height: 0,
        round: 0,
        content,
        proof,
        lock_round: None,
        lock_votes: Vec::new(),
        proposer: vec![3],
    };
    serialize(&proposal).unwrap();
}

fn bench_rlp(content: Vec<u8>) {
    let proof = Proof {
        block_hash: bench_proof(),
        height: 0,
        round: 0,
        precommit_votes: HashMap::new(),
    };

    let proposal = Proposal {
        height: 0,
        round: 0,
        content,
        proof,
        lock_round: None,
        lock_votes: Vec::new(),
        proposer: vec![3],
    };

    proposal.rlp_bytes();
}

fn bench_to_proposal(content: Vec<u8>) {
    let (s, r) = unbounded();
    let proof = Proof {
        block_hash: bench_proof(),
        height: 0,
        round: 0,
        precommit_votes: HashMap::new(),
    };

    let proposal = Proposal {
        height: 0,
        round: 0,
        content,
        proof,
        lock_round: None,
        lock_votes: Vec::new(),
        proposer: vec![3],
    };
    let res = proposal.to_bft_proposal(vec![4]);
    s.send(res).unwrap();
    r.recv().unwrap();
}

fn bench_proposal(content: Vec<u8>) {
    let (s, r) = unbounded();
    let res = SignedProposal {
        signature: vec![1, 1, 1],
        proposal: Proposal {
            height: 0,
            round: 0,
            content,
            proof: Proof {
                block_hash: bench_proof(),
                height: 0,
                round: 0,
                precommit_votes: HashMap::new(),
            },
            lock_round: None,
            lock_votes: Vec::new(),
            proposer: vec![3],
        },
    };

    s.send(res).unwrap();
    r.recv().unwrap();
}

fn bench_to_status() {
    let (s, r) = unbounded();
    let node = Node {
        address: vec![1],
        proposal_weight: 1,
        vote_weight: 1,
    };
    let status = Status {
        height: 0,
        pre_hash: vec![123],
        interval: None,
        authority_list: vec![node],
    };
    let res = status.to_bft_status();
    s.send(res).unwrap();
    r.recv().unwrap();
}

fn criterion_benchmark(c: &mut Criterion) {
    let mb = gen_mb();
    c.bench(
        "consensus",
        Benchmark::new("bench", move |b| b.iter(|| bench_bincode(mb.clone()))),
    );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
