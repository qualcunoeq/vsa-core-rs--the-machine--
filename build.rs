fn main() {
    // The CUDA kernel sources are relevant for incremental rebuilds even when
    // the feature is off, so the build graph notices when they change.
    println!("cargo:rerun-if-changed=src/cuda/nhd_wrapper.cu");
    println!("cargo:rerun-if-changed=src/cuda/nhd_kernel.cu");

    #[cfg(feature = "cuda")]
    compile_cuda();
}

/// Compile `src/cuda/nhd_wrapper.cu` into `src/cuda/libnhd.so`, which is
/// loaded at runtime via `libloading`.
///
/// This is opt-in through the `cuda` feature. The CPU build never compiles
/// (or requires) any of this.
///
/// Toolkit discovery order:
///   1. `$CUDA_PATH`
///   2. `$CUDA_HOME`
///   3. `/usr/local/cuda`, `/opt/cuda` (Arch)
///   4. any `/usr/local/cuda-*` directory containing `bin/nvcc`
///   5. pip `nvidia-*` packages under `~/.local/lib/python*/site-packages/nvidia/*`
///
/// Target architecture:
///   * `$CUDA_ARCH` if set (accepts `120`, `12.0`, or `sm_120`), otherwise
///   * `nvidia-smi --query-gpu=compute_cap`, otherwise the build is skipped
///     with an explicit warning.
#[cfg(feature = "cuda")]
fn compile_cuda() {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn toolkit_root(candidate: &Path) -> Option<PathBuf> {
        if candidate.join("bin/nvcc").exists() {
            Some(candidate.to_path_buf())
        } else {
            None
        }
    }

    fn pip_toolkits() -> Vec<PathBuf> {
        let mut kits = Vec::new();
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            return kits;
        };
        let lib = home.join(".local/lib");
        let Ok(python_dirs) = std::fs::read_dir(&lib) else {
            return kits;
        };
        for python_dir in python_dirs.flatten() {
            let nvidia = python_dir.path().join("site-packages/nvidia");
            let Ok(packages) = std::fs::read_dir(&nvidia) else {
                continue;
            };
            for package in packages.flatten() {
                if let Some(kit) = toolkit_root(&package.path()) {
                    kits.push(kit);
                }
            }
        }
        kits
    }

    fn discover_toolkit() -> Option<PathBuf> {
        for var in ["CUDA_PATH", "CUDA_HOME"] {
            if let Some(value) = std::env::var_os(var) {
                if let Some(kit) = toolkit_root(Path::new(&value)) {
                    return Some(kit);
                }
            }
        }
        for fixed in ["/usr/local/cuda", "/opt/cuda"] {
            if let Some(kit) = toolkit_root(Path::new(fixed)) {
                return Some(kit);
            }
        }
        if let Ok(entries) = std::fs::read_dir("/usr/local") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("cuda") {
                    if let Some(kit) = toolkit_root(&entry.path()) {
                        return Some(kit);
                    }
                }
            }
        }
        pip_toolkits().into_iter().next()
    }

    fn detect_arch() -> Option<String> {
        if let Ok(arch) = std::env::var("CUDA_ARCH") {
            let arch = arch.trim();
            if !arch.is_empty() {
                let digits = arch.trim_start_matches("sm_").replace('.', "");
                return Some(format!("sm_{digits}"));
            }
        }
        let output = Command::new("nvidia-smi")
            .args(["--query-gpu=compute_cap", "--format=csv,noheader"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let cap = text.lines().next()?.trim();
        if cap.is_empty() || cap.eq_ignore_ascii_case("n/a") {
            return None;
        }
        Some(format!("sm_{}", cap.replace('.', "")))
    }

    /// Returns the directory to add with `-L` and the exact link argument.
    fn find_cudart(kit: &Path) -> Option<(PathBuf, String)> {
        let dirs = [
            kit.join("lib"),
            kit.join("lib64"),
            kit.join("targets/x86_64-linux/lib"),
        ];
        for dir in &dirs {
            if dir.join("libcudart.so").exists() {
                return Some((dir.clone(), "-lcudart".to_string()));
            }
        }
        for dir in &dirs {
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("libcudart.so") {
                    return Some((dir.clone(), format!("-l:{name}")));
                }
            }
        }
        None
    }

    let Some(cuda_path) = discover_toolkit() else {
        println!("cargo:warning=CUDA feature enabled but no CUDA toolkit found.");
        println!("cargo:warning=Set CUDA_PATH (or CUDA_HOME) to a toolkit root containing bin/nvcc.");
        println!("cargo:warning=Arch Linux: pacman -S cuda (installs to /opt/cuda).");
        return;
    };

    let Some(arch) = detect_arch() else {
        println!(
            "cargo:warning=CUDA toolkit found at {} but the target architecture could not be detected.",
            cuda_path.display()
        );
        println!("cargo:warning=Set CUDA_ARCH (for example CUDA_ARCH=120 or CUDA_ARCH=12.0) and rebuild.");
        return;
    };

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_file = manifest_dir.join("src/cuda/nhd_wrapper.cu");
    let out_so = manifest_dir.join("src/cuda/libnhd.so");
    if !src_file.exists() {
        println!("cargo:warning=nhd_wrapper.cu not found at {}", src_file.display());
        return;
    }

    let Some((cudart_dir, cudart_link)) = find_cudart(&cuda_path) else {
        println!(
            "cargo:warning=libcudart not found under {}; install the CUDA runtime.",
            cuda_path.display()
        );
        return;
    };

    let cuda_bin = cuda_path.join("bin");
    let cuda_include = cuda_path.join("include");
    let new_path = match std::env::var("PATH") {
        Ok(path) => format!("{}:{path}", cuda_bin.display()),
        Err(_) => cuda_bin.display().to_string(),
    };

    let status = Command::new(cuda_bin.join("nvcc"))
        .env("PATH", new_path)
        .env("LD_LIBRARY_PATH", cudart_dir.as_os_str())
        .arg("-shared")
        .arg("-o")
        .arg(&out_so)
        .arg(format!("-arch={arch}"))
        .arg("-Xcompiler")
        .arg("-fPIC")
        .arg("-I")
        .arg(&cuda_include)
        .arg(&src_file)
        .arg("-L")
        .arg(&cudart_dir)
        .arg(&cudart_link)
        .status();

    match status {
        Ok(s) if s.success() => {
            println!(
                "cargo:warning=libnhd.so compiled for {arch} using {}",
                cuda_path.display()
            );
        }
        Ok(s) => {
            println!("cargo:warning=nvcc failed with exit code {:?}", s.code());
            println!("cargo:warning=Manual invocation example:");
            println!("cargo:warning=  {}/bin/nvcc -shared -o {} -arch={arch} \\", cuda_path.display(), out_so.display());
            println!("cargo:warning=      -Xcompiler -fPIC -I {} {} -L {} {}", cuda_include.display(), src_file.display(), cudart_dir.display(), cudart_link);
        }
        Err(e) => {
            println!("cargo:warning=Failed to run nvcc: {e}");
            println!("cargo:warning=CUDA_PATH={} does not contain a usable bin/nvcc.", cuda_path.display());
        }
    }
}
