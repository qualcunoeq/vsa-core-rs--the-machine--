/**
 * Parallel NHD (Normalized Hamming Distance) kernel.
 *
 * Computes popcount(query XOR centroid) / 10240.0 for each centroid
 * in a single kernel launch, using 256 threads per block.
 *
 * Compile to PTX:
 *   nvcc -ptx -arch=sm_50 -o nhd_kernel.ptx nhd_kernel.cu
 */

#define U64_BLOCKS 160   // matches HD_DIMENSION (10240) / 64

extern "C" __global__ void nhd_project(
    const unsigned long long* query,      // [U64_BLOCKS]
    const unsigned long long* centroids,  // [num_centroids * U64_BLOCKS]
    double* results,                      // [num_centroids]
    int num_centroids,
    int total_u64                         // U64_BLOCKS per centroid
) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx >= num_centroids) return;

    unsigned long long pop = 0;
    int base = idx * total_u64;
    #pragma unroll 4
    for (int i = 0; i < total_u64; i++) {
        pop += __popcll(query[i] ^ centroids[base + i]);
    }
    results[idx] = (double)pop / 10240.0;
}
