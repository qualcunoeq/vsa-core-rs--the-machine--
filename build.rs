fn main() {
    // When the "cuda" feature is enabled, compile the NHD kernel from source
    // into a shared library that gets loaded at runtime via libloading.
    //
    // The build script discovers the CUDA toolkit by checking:
    //   1. $CUDA_PATH environment variable
    //   2. ~/odysseus/data/local/lib/python3.12/site-packages/nvidia/cu13 (pip)
    //   3. /usr/local/cuda (standard install)
    #[cfg(feature = "cuda")]
    {
        // Find CUDA toolkit
        let cuda_path = std::env::var("CUDA_PATH")
            .or_else(|_| std::env::var("CUDA_HOME"))
            .unwrap_or_else(|_| {
                // Check common locations
                let pip_path = home_dir().join("odysseus/data/local/lib/python3.12/site-packages/nvidia/cu13");
                if pip_path.join("bin/nvcc").exists() {
                    pip_path.to_string_lossy().to_string()
                } else if let Ok(entries) = std::fs::read_dir("/usr/local/cuda") {
                    // Find the first cuda-* directory
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_dir() && p.join("bin/nvcc").exists() {
                            return p.to_string_lossy().to_string();
                        }
                    }
                    String::new()
                } else {
                    String::new()
                }
            });

        if cuda_path.is_empty() {
            println!("cargo:warning=CUDA feature enabled but CUDA toolkit not found. Run with GPU by:");
            println!("cargo:warning=  export CUDA_PATH=/path/to/cuda");
            println!("cargo:warning=Or install via pip: pip install nvidia-cuda-runtime");
            return;
        }

        let cuda_bin = std::path::Path::new(&cuda_path).join("bin");
        let cuda_include = std::path::Path::new(&cuda_path).join("include");
        let cuda_lib = std::path::Path::new(&cuda_path).join("lib");
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let src_file = manifest_dir.join("src/cuda/nhd_wrapper.cu");
        let out_so = manifest_dir.join("src/cuda/libnhd.so");

        if !src_file.exists() {
            println!("cargo:warning=nhd_wrapper.cu not found at {:?}", src_file);
            return;
        }

        // Use exact .so.13 filename instead of requiring a libcudart.so symlink
        // nvcc supports -l:filename syntax for this
        let cudart_link = cuda_lib.join("libcudart.so.13");
        let cudart_arg = format!("-l:{}", cudart_link.to_string_lossy());

        // Prepend cuda bin to PATH for nvcc
        let path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("{}:{}", cuda_bin.to_string_lossy(), path);

        let status = std::process::Command::new("nvcc")
            .env("PATH", &new_path)
            .env("LD_LIBRARY_PATH", cuda_lib.to_string_lossy().as_ref())
            .arg("-shared")
            .arg("-o")
            .arg(&out_so)
            .arg("-arch=sm_120") // RTX 5070 Ti = Blackwell sm_120
            .arg("-Xcompiler")
            .arg("-fPIC")
            .arg("-I")
            .arg(&cuda_include)
            .arg(&src_file)
            .arg("-L")
            .arg(&cuda_lib)
            .arg("-lcudart")
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("cargo:warning=libnhd.so compiled successfully");
            }
            Ok(s) => {
                println!("cargo:warning=nvcc failed with exit code {:?}", s.code());
                println!("cargo:warning=Try compiling manually:");
                println!("cargo:warning=  export CUDA_PATH={}", cuda_path);
                println!("cargo:warning=  export PATH=$CUDA_PATH/bin:$PATH");
                println!("cargo:warning=  export LD_LIBRARY_PATH=$CUDA_PATH/lib:$LD_LIBRARY_PATH");
                println!("cargo:warning=  ln -sf $CUDA_PATH/lib/libcudart.so.13 $CUDA_PATH/lib/libcudart.so");
                println!("cargo:warning=  nvcc -shared -o src/cuda/libnhd.so -arch=sm_120 \\");
                println!("cargo:warning=      -Xcompiler -fPIC -I $CUDA_PATH/include \\");
                println!("cargo:warning=      src/cuda/nhd_wrapper.cu -L $CUDA_PATH/lib -lcudart");
            }
            Err(e) => {
                println!("cargo:warning=nvcc not found: {}", e);
                println!("cargo:warning=CUDA_PATH={} does not contain bin/nvcc", cuda_path);
            }
        }

        println!("cargo:rerun-if-changed=src/cuda/nhd_wrapper.cu");
        println!("cargo:rerun-if-changed=src/cuda/nhd_kernel.cu");
    }
}

fn home_dir() -> std::path::PathBuf {
    std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/home/shiba"))
}
