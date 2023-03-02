use std::f32::consts::PI;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::Rng;

mod multifit;

use rusterf::ring_buffer::DyadicRingBuffer;

pub fn dynamic(c: &mut Criterion) {
    c.bench_function("Circle_buffer 2^16", |b| {
        b.iter(|| {
            let mut buff = DyadicRingBuffer::<usize>::new(16).unwrap();
            let mut out = DyadicRingBuffer::<usize>::new(3).unwrap();
            // let mut vecd = VecDeque::with_capacity(65536);
            // let mut outv = VecDeque::with_capacity(8);
            // for _ in 0..8 {
            //     outv.push_back(0);
            // }
            for i in 0..65536 {
                buff.push(i);
                // vecd.push_back(i);
            }
            for _ in 0..10 {
                out.extend(buff.iter());
                // for j in vecd.iter().cloned() {
                //     outv.pop_front();
                //     outv.push_back(j);
                // }
            }
            black_box(out);
            // black_box(outv);
        })
    });
}

pub fn multifit_stability(c: &mut Criterion) {
    c.bench_function("multifit", |b| {
        b.iter(|| {
            let num_pts = 250;
            let mut rng = rand::thread_rng();
            let mut fit =
                multifit::FitSetup::init(5, num_pts, 1000, 1e-8, 1e-8, 1e-8, 100.0).unwrap();
            let mut data = Vec::with_capacity(num_pts as usize);
            let center = [100.0, 0.15, 0.0, 200.0];

            // let mut sum_sqr: f32 = 0.0;
            let mut max_dev: f32 = 0.0;
            let mut n_iter = 0;

            for _ in 0..1 {
                let act = [
                    center[0] * rng.gen_range(0.7..1.3),
                    center[1] * rng.gen_range(0.85..1.15),
                    rng.gen_range(-PI..PI),
                    center[3] + rng.gen_range(-100.0..100.0),
                ];
                data.clear();
                data.extend((0..num_pts).map(|x| multifit::sinusoid(x as f32, act)));

                println!("data: {:?}", data);
                let res = fit.fit(data.as_slice(), [0.0, 0.03, 0.0, 0.0]);
                println!("fit: {:?}", res.params);
                // let res = fit.fit(data.as_slice(), center);
                max_dev = max_dev.max((res.params[2] - act[2]).abs());
                // sum_sqr += (res.params[2] - act[2]).powi(2);
                n_iter += res.n_iterations;
                if max_dev > 1.0e-3 {
                    println!("act {:?}, fit {:?}", act, res.params);
                }
                println!(
                    "dev_phi: {}, dev_net: {}, avg iter: {}",
                    max_dev,
                    (res.params
                        .iter()
                        .zip(&act)
                        .map(|(a, b)| (a - b) * (a - b))
                        .sum::<f32>()
                        / res.params.len() as f32)
                        .sqrt(),
                    n_iter,
                );
            }
        })
    });
}

criterion_group!(benches, multifit_stability);
criterion_main!(benches);

// criterion_group!(benches, dynamic);
// criterion_main!(benches);
