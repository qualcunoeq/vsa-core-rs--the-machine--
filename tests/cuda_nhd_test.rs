//! Integration test for CUDA-accelerated NHD projection.
//!
//! These tests are only compiled when the `cuda` feature is enabled:
//!   cargo test --features cuda --test cuda_nhd_test
//!
//! They require an NVIDIA GPU with CUDA driver installed (libcuda.so).

#![cfg(feature = "cuda")]

use the_machine::cuda_projector::CudaProjector;
use the_machine::Hypervector;

/// Verify that GPU-computed NHD matches the CPU reference for small sets.
#[test]
fn test_cuda_nhd_matches_cpu() {
    let query = Hypervector::new_random();
    let centroids: Vec<Hypervector> = (0..32).map(|_| Hypervector::new_random()).collect();

    let projector = CudaProjector::new(64).expect("CUDA initialisation failed");

    let gpu_results = projector.project(&query, &centroids)
        .expect("GPU projection failed");

    assert_eq!(gpu_results.len(), 32);

    // Compare with CPU reference
    for (i, (centroid, gpu_dist)) in centroids.iter().zip(gpu_results.iter()).enumerate() {
        let cpu_dist = query.normalized_hamming_distance(centroid);
        let diff = (cpu_dist - gpu_dist).abs();
        assert!(
            diff < 1e-12,
            "Mismatch at centroid {}: CPU={:.10} GPU={:.10} diff={:.2e}",
            i, cpu_dist, gpu_dist, diff
        );
        assert!(
            *gpu_dist >= 0.0 && *gpu_dist <= 1.0,
            "NHD out of range at centroid {}: {}",
            i, gpu_dist
        );
    }
}

/// Verify that identical hypervectors yield NHD ≈ 0 on GPU.
#[test]
fn test_cuda_nhd_identity() {
    let hv = Hypervector::new_random();
    let centroids = vec![hv; 8]; // 8 copies of the same vector

    let projector = CudaProjector::new(16).expect("CUDA init failed");

    let results = projector.project(&hv, &centroids).expect("GPU projection failed");
    for (i, &d) in results.iter().enumerate() {
        assert!(
            d < 1e-12,
            "Identity NHD should be ~0 at centroid {}, got {}",
            i, d
        );
    }
}

/// Verify the GPU can handle the full pipeline centroid set (21 centroids).
#[test]
fn test_cuda_nhd_21_centroids() {
    let query = Hypervector::new_random();
    let centroids: Vec<Hypervector> = (0..21).map(|_| Hypervector::new_random()).collect();

    let projector = CudaProjector::new(64).expect("CUDA init failed");
    let results = projector.project(&query, &centroids).expect("GPU projection failed");

    assert_eq!(results.len(), 21);
    for (i, &d) in results.iter().enumerate() {
        assert!(
            d >= 0.0 && d <= 1.0,
            "NHD out of range at centroid {}: {}", i, d
        );
    }
}

/// Verify all 315 reference centroids project correctly.
#[test]
fn test_cuda_nhd_315_centroids() {
    let query = Hypervector::new_random();
    let centroids: Vec<Hypervector> = (0..315).map(|_| Hypervector::new_random()).collect();

    let projector = CudaProjector::new(315).expect("CUDA init failed");
    let results = projector.project(&query, &centroids).expect("GPU projection failed");

    assert_eq!(results.len(), 315);

    // Spot-check a few against CPU
    for i in [0, 100, 200, 314] {
        let cpu_dist = query.normalized_hamming_distance(&centroids[i]);
        let diff = (cpu_dist - results[i]).abs();
        assert!(
            diff < 1e-12,
            "Mismatch at centroid {}: CPU={:.10} GPU={:.10} diff={:.2e}",
            i, cpu_dist, results[i], diff
        );
    }
}

/// Verify resizing works correctly.
#[test]
fn test_cuda_nhd_resize() {
    let query = Hypervector::new_random();
    let small: Vec<Hypervector> = (0..10).map(|_| Hypervector::new_random()).collect();
    let large: Vec<Hypervector> = (0..200).map(|_| Hypervector::new_random()).collect();

    let mut projector = CudaProjector::new(64).expect("CUDA init failed");

    // Project small set
    let small_results = projector.project(&query, &small).expect("Small projection failed");
    assert_eq!(small_results.len(), 10);

    // Resize for larger set
    projector.resize(256).expect("Resize failed");

    // Project large set
    let large_results = projector.project(&query, &large).expect("Large projection failed");
    assert_eq!(large_results.len(), 200);

    for (i, (centroid, gpu_dist)) in large.iter().zip(large_results.iter()).enumerate() {
        let cpu_dist = query.normalized_hamming_distance(centroid);
        let diff = (cpu_dist - gpu_dist).abs();
        assert!(
            diff < 1e-12,
            "Mismatch after resize at centroid {}: CPU={:.10} GPU={:.10} diff={:.2e}",
            i, cpu_dist, gpu_dist, diff
        );
    }
}
