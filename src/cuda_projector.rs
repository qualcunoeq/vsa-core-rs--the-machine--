//! CUDA-accelerated parallel centroid projection.
//!
//! Loads libnhd.so at runtime via libloading and calls its FFI functions.
//! libnhd.so is a CUDA shared library compiled from src/cuda/nhd_wrapper.cu.
//!
//! Build instructions for the .so:
//! ```bash
//! export CUDA_PATH="/home/shiba/odysseus/data/local/lib/python3.12/site-packages/nvidia/cu13"
//! export LD_LIBRARY_PATH="$CUDA_PATH/lib:$LD_LIBRARY_PATH"
//! export PATH="$CUDA_PATH/bin:$PATH"
//! ln -sf "$CUDA_PATH/lib/libcudart.so.13" "$CUDA_PATH/lib/libcudart.so"
//! nvcc -shared -o src/cuda/libnhd.so -arch=sm_89 -Xcompiler -fPIC \
//!     -I "$CUDA_PATH/include" src/cuda/nhd_wrapper.cu \
//!     -L "$CUDA_PATH/lib" -lcudart
//! ```
//!
//! At runtime, LD_LIBRARY_PATH must include the cuda lib directory:
//! ```bash
//! export LD_LIBRARY_PATH="/home/shiba/odysseus/data/local/lib/python3.12/site-packages/nvidia/cu13/lib:$LD_LIBRARY_PATH"
//! ```

use crate::Hypervector;
use libloading::{Library, Symbol};
use std::path::Path;

const U64_BLOCKS: usize = super::U64_BLOCKS; // 160

/// CUDA error codes
const CUDA_SUCCESS: std::ffi::c_int = 0;

/// Safe wrapper around the CUDA NHD projector (.so loaded at runtime).
pub struct CudaProjector {
    _lib: Library, // keep the library alive
    d_query: *mut u64,
    d_centroids: *mut u64,
    d_results: *mut f64,
    capacity: usize,
    // Function pointers cached from the library
    nhd_project_fn: unsafe extern "C" fn(
        *const u64,
        *const u64,
        *mut f64,
        std::ffi::c_int,
        std::ffi::c_int,
    ) -> std::ffi::c_int,
    nhd_malloc_fn: unsafe extern "C" fn(*mut *mut u64, usize) -> std::ffi::c_int,
    nhd_free_fn: unsafe extern "C" fn(*mut u64) -> std::ffi::c_int,
    nhd_memcpy_htod_fn: unsafe extern "C" fn(*mut u64, *const u64, usize) -> std::ffi::c_int,
    nhd_memcpy_dtoh_fn: unsafe extern "C" fn(*mut u64, *const u64, usize) -> std::ffi::c_int,
}

impl CudaProjector {
    /// Locate libnhd.so relative to the crate manifest directory.
    fn libnhd_path() -> Result<std::path::PathBuf, String> {
        // Try the canonical location in src/cuda/
        let in_src = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("cuda")
            .join("libnhd.so");
        if in_src.exists() {
            return Ok(in_src);
        }

        // Fall back to target directory (for installed binaries)
        let in_target = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("libnhd.so");
        if in_target.exists() {
            return Ok(in_target);
        }

        Err(format!(
            "libnhd.so not found. Compile it first:\n  \
             cd {} && \\\n  \
             CUDA_PATH=\"/home/shiba/odysseus/data/local/lib/python3.12/site-packages/nvidia/cu13\" \\\n  \
             LD_LIBRARY_PATH=\"$CUDA_PATH/lib\" \\\n  \
             PATH=\"$CUDA_PATH/bin:$PATH\" \\\n  \
             nvcc -shared -o src/cuda/libnhd.so -arch=sm_89 -Xcompiler -fPIC \\\n  \
             \t-I \"$CUDA_PATH/include\" src/cuda/nhd_wrapper.cu \\\n  \
             \t-L \"$CUDA_PATH/lib\" -lcudart",
            env!("CARGO_MANIFEST_DIR"),
        ))
    }

    /// Initialise GPU memory and warm up the CUDA context.
    ///
    /// `max_centroids` — initial capacity for centroid storage on GPU.
    pub fn new(max_centroids: usize) -> Result<Self, String> {
        let lib_path = Self::libnhd_path()?;

        unsafe {
            // Load the shared library
            let lib = Library::new(&lib_path)
                .map_err(|e| format!("Failed to load libnhd.so from {:?}: {}", lib_path, e))?;

            // Resolve function symbols
            let nhd_project_fn: Symbol<
                unsafe extern "C" fn(
                    *const u64,
                    *const u64,
                    *mut f64,
                    std::ffi::c_int,
                    std::ffi::c_int,
                ) -> std::ffi::c_int,
            > = lib
                .get(b"nhd_project")
                .map_err(|e| format!("Symbol 'nhd_project' not found: {}", e))?;

            let nhd_malloc_fn: Symbol<
                unsafe extern "C" fn(*mut *mut u64, usize) -> std::ffi::c_int,
            > = lib
                .get(b"nhd_malloc")
                .map_err(|e| format!("Symbol 'nhd_malloc' not found: {}", e))?;

            let nhd_free_fn: Symbol<unsafe extern "C" fn(*mut u64) -> std::ffi::c_int> = lib
                .get(b"nhd_free")
                .map_err(|e| format!("Symbol 'nhd_free' not found: {}", e))?;

            let nhd_memcpy_htod_fn: Symbol<
                unsafe extern "C" fn(*mut u64, *const u64, usize) -> std::ffi::c_int,
            > = lib
                .get(b"nhd_memcpy_htod")
                .map_err(|e| format!("Symbol 'nhd_memcpy_htod' not found: {}", e))?;

            let nhd_memcpy_dtoh_fn: Symbol<
                unsafe extern "C" fn(*mut u64, *const u64, usize) -> std::ffi::c_int,
            > = lib
                .get(b"nhd_memcpy_dtoh")
                .map_err(|e| format!("Symbol 'nhd_memcpy_dtoh' not found: {}", e))?;

            // Cache function pointers (the Symbol borrows from lib, which we keep alive)
            let nhd_project_fn = *nhd_project_fn;
            let nhd_malloc_fn = *nhd_malloc_fn;
            let nhd_free_fn = *nhd_free_fn;
            let nhd_memcpy_htod_fn = *nhd_memcpy_htod_fn;
            let nhd_memcpy_dtoh_fn = *nhd_memcpy_dtoh_fn;

            // Allocate GPU memory
            let query_bytes = U64_BLOCKS * 8;
            let centroids_bytes = max_centroids * U64_BLOCKS * 8;
            let results_bytes = max_centroids * 8;

            let mut d_query: *mut u64 = std::ptr::null_mut();
            let mut d_centroids: *mut u64 = std::ptr::null_mut();
            let mut d_results: *mut f64 = std::ptr::null_mut();

            let err = nhd_malloc_fn(&mut d_query, query_bytes);
            if err != CUDA_SUCCESS {
                return Err(format!(
                    "nhd_malloc(query, {}) failed: error {}",
                    query_bytes, err
                ));
            }
            let err = nhd_malloc_fn(&mut d_centroids, centroids_bytes);
            if err != CUDA_SUCCESS {
                nhd_free_fn(d_query);
                return Err(format!(
                    "nhd_malloc(centroids, {}) failed: error {}",
                    centroids_bytes, err
                ));
            }
            let err = nhd_malloc_fn(
                &mut d_results as *mut *mut f64 as *mut *mut u64,
                results_bytes,
            );
            if err != CUDA_SUCCESS {
                nhd_free_fn(d_query);
                nhd_free_fn(d_centroids);
                return Err(format!(
                    "nhd_malloc(results, {}) failed: error {}",
                    results_bytes, err
                ));
            }

            Ok(CudaProjector {
                _lib: lib,
                d_query,
                d_centroids,
                d_results,
                capacity: max_centroids,
                nhd_project_fn,
                nhd_malloc_fn,
                nhd_free_fn,
                nhd_memcpy_htod_fn,
                nhd_memcpy_dtoh_fn,
            })
        }
    }

    /// Compute NHD between `query` and each centroid in `centroids`.
    ///
    /// Returns `Vec<f64>` where `result[i] = NHD(query, centroids[i])`.
    pub fn project(
        &self,
        query: &Hypervector,
        centroids: &[Hypervector],
    ) -> Result<Vec<f64>, String> {
        let n = centroids.len();
        if n == 0 {
            return Ok(Vec::new());
        }
        if n > self.capacity {
            return Err(format!(
                "centroid count {} exceeds capacity {}. Call resize({}) first.",
                n, self.capacity, n
            ));
        }

        unsafe {
            // 1. Upload query
            let err = (self.nhd_memcpy_htod_fn)(self.d_query, query.bits.as_ptr(), U64_BLOCKS * 8);
            if err != CUDA_SUCCESS {
                return Err(format!("nhd_memcpy_htod(query) failed: error {}", err));
            }

            // 2. Flatten and upload centroids
            let mut flat = Vec::with_capacity(n * U64_BLOCKS);
            for c in centroids {
                flat.extend_from_slice(&c.bits);
            }
            let err =
                (self.nhd_memcpy_htod_fn)(self.d_centroids, flat.as_ptr(), n * U64_BLOCKS * 8);
            if err != CUDA_SUCCESS {
                return Err(format!("nhd_memcpy_htod(centroids) failed: error {}", err));
            }

            // 3. Launch kernel
            let err = (self.nhd_project_fn)(
                self.d_query,
                self.d_centroids,
                self.d_results,
                n as std::ffi::c_int,
                U64_BLOCKS as std::ffi::c_int,
            );
            if err != CUDA_SUCCESS {
                return Err(format!("nhd_project kernel failed: error {}", err));
            }

            // 4. Download results
            let mut results = vec![0.0f64; n];
            let err = (self.nhd_memcpy_dtoh_fn)(
                results.as_mut_ptr() as *mut u64,
                self.d_results as *const u64,
                n * 8,
            );
            if err != CUDA_SUCCESS {
                return Err(format!("nhd_memcpy_dtoh(results) failed: error {}", err));
            }

            Ok(results)
        }
    }

    /// Reallocate GPU memory for a larger/smaller centroid set.
    pub fn resize(&mut self, max_centroids: usize) -> Result<(), String> {
        if max_centroids == self.capacity {
            return Ok(());
        }

        let centroids_bytes = max_centroids * U64_BLOCKS * 8;
        let results_bytes = max_centroids * 8;

        unsafe {
            // Free old
            (self.nhd_free_fn)(self.d_centroids);
            (self.nhd_free_fn)(self.d_results as *mut u64);

            // Allocate new
            let mut d_centroids: *mut u64 = std::ptr::null_mut();
            let mut d_results: *mut f64 = std::ptr::null_mut();

            let err = (self.nhd_malloc_fn)(&mut d_centroids, centroids_bytes);
            if err != CUDA_SUCCESS {
                return Err(format!(
                    "nhd_malloc(centroids) resize failed: error {}",
                    err
                ));
            }
            let err = (self.nhd_malloc_fn)(
                &mut d_results as *mut *mut f64 as *mut *mut u64,
                results_bytes,
            );
            if err != CUDA_SUCCESS {
                (self.nhd_free_fn)(d_centroids);
                return Err(format!("nhd_malloc(results) resize failed: error {}", err));
            }

            self.d_centroids = d_centroids;
            self.d_results = d_results;
            self.capacity = max_centroids;

            Ok(())
        }
    }
}

impl Drop for CudaProjector {
    fn drop(&mut self) {
        unsafe {
            (self.nhd_free_fn)(self.d_query);
            (self.nhd_free_fn)(self.d_centroids);
            (self.nhd_free_fn)(self.d_results as *mut u64);
        }
    }
}

// Safety: CudaProjector owns GPU memory and a library handle.
// All GPU operations are synchronization-free once they return.
unsafe impl Send for CudaProjector {}
unsafe impl Sync for CudaProjector {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_libnhd_path_resolves() {
        let path = CudaProjector::libnhd_path();
        assert!(path.is_ok(), "libnhd_path should resolve: {:?}", path);
        let path_str = path.unwrap();
        assert!(
            path_str.to_string_lossy().ends_with("libnhd.so"),
            "path should end with libnhd.so"
        );
    }

    #[test]
    fn test_new_fails_without_cuda_library() {
        match CudaProjector::new(16) {
            Ok(_) => eprintln!("  CUDA library found — CudaProjector constructed"),
            Err(e) => eprintln!("  Expected: CudaProjector::new failed: {}", e),
        }
        // Either result is acceptable — the test just verifies no panic
    }
}
