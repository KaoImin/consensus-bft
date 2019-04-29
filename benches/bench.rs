extern crate consensus_bft as BFT;
#[macro_use]
extern crate criterion;

use blake2b_simd::Params;
use criterion::{Benchmark, Criterion};
use crossbeam_channel::unbounded;
use rand::random;
// use sha3::{Digest, Sha3_256};

#[inline(always)]
fn gen_mb(size: usize) -> Vec<u8> {
    let mb: Vec<u8> = (0..1024 * 1024 * size).map(|_| random::<u8>()).collect();
    mb
}

fn convert_proposal(content: Vec<u8>) {
    let (s, r) = unbounded();
    let mut short = Vec::new();
    if content.len() > 100 {
        let a = (content.len() as f64 / 100 as f64).round() as usize;
        for i in 0..100 {
            short.push(content[a * i]);
        }
    }

    let hash = Params::new()
        .hash_length(32)
        .to_state()
        .update(&short)
        .finalize()
        .as_bytes()
        .to_owned();
    s.send(hash.clone()).unwrap();
    r.recv().unwrap();
    s.send(hash).unwrap();
    r.recv().unwrap();
}

fn no_convert_proposal(content: Vec<u8>) {
    let (s, r) = unbounded();
    s.send(content.clone()).unwrap();
    r.recv().unwrap();
    s.send(content).unwrap();
    r.recv().unwrap();
}

fn benchmark_1(c: &mut Criterion) {
    let msg = gen_mb(2);
    c.bench(
        "consensus",
        Benchmark::new("bench_convert", move |b| {
            b.iter(|| convert_proposal(msg.clone()))
        }),
    );
}

fn benchmark_2(c: &mut Criterion) {
    let msg = gen_mb(2);
    c.bench(
        "consensus",
        Benchmark::new("bench_no_convert", move |b| {
            b.iter(|| no_convert_proposal(msg.clone()))
        }),
    );
}

criterion_group!(benches, benchmark_1, benchmark_2);
criterion_main!(benches);

// fn bench_to_proposal(content: Vec<u8>) {
//     let (s, r) = unbounded();
//     let proof = Proof {
//         block_hash: bench_proof(),
//         height: 0,
//         round: 0,
//         precommit_votes: HashMap::new(),
//     };

//     let proposal = Proposal {
//         height: 0,
//         round: 0,
//         content,
//         proof,
//         lock_round: None,
//         lock_votes: Vec::new(),
//         proposer: vec![3],
//     };
//     let res = proposal.to_bft_proposal(vec![4]);
//     s.send(res).unwrap();
//     r.recv().unwrap();
// }

// fn bench_proposal(content: Vec<u8>) {
//     let (s, r) = unbounded();
//     let res = SignedProposal {
//         signature: vec![1, 1, 1],
//         proposal: Proposal {
//             height: 0,
//             round: 0,
//             content,
//             proof: Proof {
//                 block_hash: bench_proof(),
//                 height: 0,
//                 round: 0,
//                 precommit_votes: HashMap::new(),
//             },
//             lock_round: None,
//             lock_votes: Vec::new(),
//             proposer: vec![3],
//         },
//     };

//     s.send(res).unwrap();
//     r.recv().unwrap();
// }
