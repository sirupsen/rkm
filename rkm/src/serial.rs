// Serial (single threaded) implementation details
use ndarray::{Array2, ArrayView2, Axis, Ix};
use rand::distributions::{Distribution, Weighted, WeightedChoice};
use rand::prelude::*;
use rand::Rng;
//use super::{Value, RandomSeed, distance_squared, closest_mean};
use crate::common::*;

/// Find the shortest distance between each data point and any of a set of mean points.
pub fn closest_distance<V: Value>(means: &ArrayView2<V>, data: &ArrayView2<V>) -> Vec<V> {
    data.outer_iter()
        .map(|d| {
            let mut iter = means.outer_iter();
            let mut closest = distance_squared(&d, &iter.next().unwrap());
            for m in iter {
                let distance = distance_squared(&d, &m);
                if distance < closest {
                    closest = distance;
                }
            }
            closest
        })
        .collect()
}

/// This is a mean initialization method based on the [kmeans++](https://en.wikipedia.org/wiki/K-means%2B%2B)
/// initialization algorithm.
pub fn initialize_plusplus<V: Value>(
    data: &ArrayView2<V>,
    k: usize,
    seed: Option<RandomSeed>,
) -> Array2<V> {
    assert!(k > 1);
    assert!(data.dim().0 > 0);
    let mut means = Array2::zeros((k as usize, data.shape()[1]));
    let mut rng = match seed {
        Some(seed) => SmallRng::from_seed(seed),
        None => SmallRng::from_rng(rand::thread_rng()).unwrap(),
    };
    let data_len = data.shape()[0];
    let chosen = rng.gen_range(0, data_len) as isize;
    means
        .slice_mut(s![0..1, ..])
        .assign(&data.slice(s![chosen..(chosen + 1), ..]));
    for i in 1..k as isize {
        // Calculate the distance to the closest mean for each data point
        let distances = closest_distance(&means.slice(s![0..i, ..]).view(), &data.view());
        // Pick a random point weighted by the distance from existing means
        let distance_sum: f64 = distances
            .iter()
            .fold(0.0f64, |sum, d| sum + num::cast::<V, f64>(*d).unwrap());
        let mut weights: Vec<Weighted<usize>> = distances
            .iter()
            .zip(0..data_len)
            .map(|p| Weighted {
                weight: ((num::cast::<V, f64>(*p.0).unwrap() / distance_sum)
                    * ((std::u32::MAX) as f64))
                    .floor() as u32,
                item: p.1,
            })
            .collect();
        let chooser = WeightedChoice::new(&mut weights);
        let chosen = chooser.sample(&mut rng) as isize;
        means
            .slice_mut(s![i..(i + 1), ..])
            .assign(&data.slice(s![chosen..(chosen + 1), ..]));
    }
    means
}

/// Calculate the index of the mean each data point is closest to (euclidean distance).
pub fn calculate_clusters<V: Value>(data: &ArrayView2<V>, means: &ArrayView2<V>) -> Vec<Ix> {
    data.outer_iter()
        .map(|point| closest_mean(&point.view(), means))
        .collect()
}

/// Calculate means based on data points and their cluster assignments.
pub fn calculate_means<V: Value>(
    data: &ArrayView2<V>,
    clusters: &Vec<Ix>,
    old_means: &ArrayView2<V>,
    k: usize,
) -> Array2<V> {
    // TODO: replace old_means parameter with just its dimension, or eliminate it completely; that's all we need
    let (means, _) = clusters.iter().zip(data.outer_iter()).fold(
        (Array2::zeros(old_means.dim()), vec![0; k]),
        |mut cumulative_means, point| {
            {
                let mut mean = cumulative_means.0.index_axis_mut(Axis(0), *point.0);
                let n = V::from(cumulative_means.1[*point.0]).unwrap();
                let step1 = &mean * n;
                let step2 = &step1 + &point.1;
                let step3 = &step2 / (n + V::one());
                mean.assign(&step3);
                // TODO: file a bug about how + and += work with ndarray
            }
            cumulative_means.1[*point.0] += 1;
            cumulative_means
        },
    );
    means
}