/**
 * C wrapper around the CUDA NHD kernel.
 * Compiles to a shared library that Rust can FFI into.
 *
 * Compile:
 *   nvcc -shared -o src/cuda/libnhd.so \
 *         -arch=sm_89 \
 *         -Xcompiler -fPIC \
 *         src/cuda/nhd_wrapper.cu
 */

#include <cuda_runtime.h>
#include <stdint.h>

#define U64_BLOCKS 160

/**
 * Kernel: compute NHD between query and N centroids in parallel.
 * Each thread handles one centroid.
 */
__global__ void nhd_project_kernel(
    const uint64_t* __restrict__ query,
    const uint64_t* __restrict__ centroids,
    double* __restrict__ results,
    int num_centroids,
    int total_u64
) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx >= num_centroids) return;

    uint64_t pop = 0;
    int base = idx * total_u64;
    #pragma unroll 4
    for (int i = 0; i < total_u64; i++) {
        pop += __popcll(query[i] ^ centroids[base + i]);
    }
    results[idx] = (double)pop / 10240.0;
}

/**
 * Host-callable wrapper: project query against N centroids.
 * All pointers must be device pointers.
 *
 * Returns 0 on success, non-zero CUDA error code on failure.
 */
extern "C" int nhd_project(
    const uint64_t* d_query,
    const uint64_t* d_centroids,
    double* d_results,
    int num_centroids,
    int total_u64
) {
    if (num_centroids <= 0 || total_u64 <= 0) return cudaErrorInvalidValue;

    dim3 block(256, 1, 1);
    dim3 grid((num_centroids + 255) / 256, 1, 1);

    nhd_project_kernel<<<grid, block>>>(d_query, d_centroids, d_results, num_centroids, total_u64);

    cudaError_t err = cudaGetLastError();
    if (err != cudaSuccess) return (int)err;

    err = cudaDeviceSynchronize();
    return (int)err;
}

/**
 * Allocate device memory. Returns pointer via *dptr, or 0 on failure.
 * Returns 0 on success.
 */
extern "C" int nhd_malloc(uint64_t** dptr, size_t bytes) {
    if (bytes == 0) return cudaErrorInvalidValue;
    cudaError_t err = cudaMalloc(dptr, bytes);
    return (int)err;
}

/**
 * Free device memory. Returns 0 on success.
 */
extern "C" int nhd_free(uint64_t* dptr) {
    cudaError_t err = cudaFree(dptr);
    return (int)err;
}

/**
 * Copy host → device. Returns 0 on success.
 */
extern "C" int nhd_memcpy_htod(uint64_t* dst, const uint64_t* src, size_t bytes) {
    cudaError_t err = cudaMemcpy(dst, src, bytes, cudaMemcpyHostToDevice);
    return (int)err;
}

/**
 * Copy device → host. Returns 0 on success.
 */
extern "C" int nhd_memcpy_dtoh(uint64_t* dst, const uint64_t* src, size_t bytes) {
    cudaError_t err = cudaMemcpy(dst, src, bytes, cudaMemcpyDeviceToHost);
    return (int)err;
}
