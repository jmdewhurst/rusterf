use rand::Rng;

use super::*;

#[test]
fn basic() {
    let num_points = 1000;
    let mut rng = rand::thread_rng();
    let mut setup = FitSetup::init(1, num_points, 32, 1.0e-8, 1.0e-8, 1.0e-8, 1.5).unwrap();
    let mut data = Vec::new();
    let center = [1000.0, 0.02, 0.0, 2000.0];

    for _ in 0..100 {
        let actual = [
            center[0] * rng.gen_range(0.8..1.2),
            center[1] * rng.gen_range(0.9..1.1),
            rng.gen_range(-PI..PI),
            center[3] + rng.gen_range(-100.0..100.0),
        ];
        data.clear();
        data.extend((0..num_points).map(|x| sinusoid(x as f32, actual)));

        let res = setup.fit(data.as_slice(), center);
        assert!((res.params[0] - actual[0]).abs() < 1.0);
        assert!((res.params[1] - actual[1]).abs() / actual[1] < 0.001);
        assert!((res.params[2] - actual[2]).abs() < 0.001);
        assert!((res.params[3] - actual[3]).abs() < 1.0);
    }
}

#[test]
fn skip_rate() {
    let num_points = 16384;
    let center = [1000.0, 0.0012, 0.0, 2000.0];
    let guess = [1001.0, 0.0011, 0.2, 1900.0];
    let base_data: Vec<f32> = (0..num_points)
        .map(|x| sinusoid(x as f32, center))
        .collect();
    for skip_rate in [1u32, 2, 4, 8, 10, 40, 100, 1000] {
        let num_points_reduced = (num_points + skip_rate - 1) / skip_rate;
        let data_reduced: Vec<f32> = base_data
            .iter()
            .copied()
            .step_by(skip_rate as usize)
            .collect();
        let mut setup = FitSetup::init(
            skip_rate,
            num_points_reduced,
            32,
            1.0e-8,
            1.0e-8,
            1.0e-8,
            1.5,
        )
        .unwrap();
        let res = setup.fit(data_reduced.as_slice(), guess);
        assert!((res.params[0] - center[0]).abs() < 1.0);
        assert!((res.params[1] - center[1]).abs() / center[1] < 0.001);
        assert!((res.params[2] - center[2]).abs() < 0.001);
        assert!((res.params[3] - center[3]).abs() < 1.0);
    }
}

#[test]
fn iterations() {
    let num_points = 1000;
    let mut rng = rand::thread_rng();
    let mut setup = FitSetup::init(1, num_points, 32, 1.0e-8, 1.0e-8, 1.0e-8, 1.5).unwrap();
    let mut data = Vec::new();
    let center = [1000.0, 0.02, 0.0, 2000.0];

    for _ in 0..100 {
        let actual = [
            center[0] * rng.gen_range(0.8..1.2),
            center[1] * rng.gen_range(0.9..1.1),
            rng.gen_range(-PI..PI),
            center[3] + rng.gen_range(-100.0..100.0),
        ];
        data.clear();
        data.extend((0..num_points).map(|x| sinusoid(x as f32, actual)));

        let res = setup.fit(data.as_slice(), center);
        assert!((res.params[0] - actual[0]).abs() < 1.0);
        assert!((res.params[1] - actual[1]).abs() / actual[1] < 0.001);
        assert!((res.params[2] - actual[2]).abs() < 0.001);
        assert!((res.params[3] - actual[3]).abs() < 1.0);
        assert!(res.n_iterations < 16);
    }
}

#[test]
fn stability() {
    let num_points = 100;
    let num_trials = 10_000;
    let mut rng = rand::thread_rng();
    let mut setup = FitSetup::init(1, num_points, 32, 1.0e-8, 1.0e-8, 1.0e-8, 1.5).unwrap();
    let mut data = Vec::new();
    let center = [1000.0, 0.2, 0.0, 2000.0];

    let mut num_failures = 0;
    for i in 0..num_trials {
        let actual = [
            center[0] * rng.gen_range(0.2..1.5),
            center[1] * rng.gen_range(0.8..1.25),
            rng.gen_range(-PI..PI),
            center[3] + rng.gen_range(-1000.0..1000.0),
        ];
        data.clear();
        data.extend((0..num_points).map(|x| sinusoid(x as f32, actual)));

        let res = setup.fit(data.as_slice(), [0.0, center[1], 0.0, 0.0]);
        if ((res.params[0] - actual[0]).abs() > 1.0)
            || ((res.params[1] - actual[1]).abs() / actual[1] > 0.001)
            || ((res.params[2] - actual[2]).abs() > 0.001)
            || ((res.params[3] - actual[3]).abs() > 1.0)
        {
            println!("failure at iteration {}:", i);
            println!("fitting {:?}\nresults {:?}", actual, res.params);
            println!("guess {:?}", center);
            num_failures += 1;
        }

        if num_failures > 5 {
            println!(
                "failed {} times in {} attempts, exceeding threshold of {}",
                num_failures, num_trials, 5
            );
            panic!();
        }
        // assert!((res.params[0] - actual[0]).abs() < 1.0);
        // assert!((res.params[1] - actual[1]).abs() / actual[1] < 0.001);
        // assert!((res.params[2] - actual[2]).abs() < 0.001);
        // assert!((res.params[3] - actual[3]).abs() < 1.0);
    }
}
