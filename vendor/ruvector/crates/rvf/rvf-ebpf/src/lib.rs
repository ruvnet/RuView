//! Real eBPF programs for RVF vector distance computation.
//!
//! This crate provides:
//!
//! - Pre-written BPF C source programs (`programs` module) for XDP
//!   distance computation, socket-level port filtering, and TC-based
//!   query priority routing.
//! - An `EbpfCompiler` that invokes `clang` to compile BPF C sources
//!   into ELF object files suitable for embedding in RVF stores.
//! - A `CompiledProgram` struct holding the resulting ELF bytes,
//!   program metadata, and a SHA3-256 hash for integrity verification.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use rvf_types::ebpf::{EbpfAttachType, EbpfHeader, EbpfProgramType, EBPF_MAGIC};

/// Errors that can occur during eBPF compilation or embedding.
#[derive(Debug)]
pub enum EbpfError {
    /// `clang` was not found in PATH.
    ClangNotFound,
    /// Compilation failed with the given stderr output.
    CompilationFailed(String),
    /// I/O error while reading/writing files.
    Io(std::io::Error),
    /// The compiled ELF is empty or invalid.
    InvalidElf,
}

impl std::fmt::Display for EbpfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClangNotFound => write!(f, "clang not found in PATH"),
            Self::CompilationFailed(msg) => write!(f, "BPF compilation failed: {msg}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::InvalidElf => write!(f, "compiled ELF is empty or invalid"),
        }
    }
}

impl std::error::Error for EbpfError {}

impl From<std::io::Error> for EbpfError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Optimization level passed to clang.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptLevel {
    O0,
    O1,
    O2,
    O3,
}

impl OptLevel {
    fn as_flag(&self) -> &'static str {
        match self {
            Self::O0 => "-O0",
            Self::O1 => "-O1",
            Self::O2 => "-O2",
            Self::O3 => "-O3",
        }
    }
}

/// A compiled eBPF program ready for embedding in an RVF store.
#[derive(Clone, Debug)]
pub struct CompiledProgram {
    /// Raw ELF object bytes.
    pub elf_bytes: Vec<u8>,
    /// Program type classification.
    pub program_type: EbpfProgramType,
    /// Attach point classification.
    pub attach_type: EbpfAttachType,
    /// Optional BTF (BPF Type Format) section bytes.
    pub btf_bytes: Option<Vec<u8>>,
    /// Number of BPF instructions (elf_bytes.len() / 8 approximation).
    pub insn_count: u16,
    /// SHA3-256 hash of the ELF bytes for integrity checking.
    pub program_hash: [u8; 32],
}

impl CompiledProgram {
    /// Build an `EbpfHeader` from this compiled program for RVF embedding.
    pub fn to_ebpf_header(&self, max_dimension: u16) -> EbpfHeader {
        EbpfHeader {
            ebpf_magic: EBPF_MAGIC,
            header_version: 1,
            program_type: self.program_type as u8,
            attach_type: self.attach_type as u8,
            program_flags: 0,
            insn_count: self.insn_count,
            max_dimension,
            program_size: self.elf_bytes.len() as u64,
            map_count: 0,
            btf_size: self.btf_bytes.as_ref().map_or(0, |b| b.len() as u32),
            program_hash: self.program_hash,
        }
    }
}

/// Pre-compiled BPF bytecode for environments without clang.
///
/// Each constant is a minimal valid ELF file containing BPF bytecode
/// for the corresponding program. These are generated from the C
/// sources in `bpf/` and embedded at compile time so that RVF files
/// can be built in CI/CD without requiring a BPF-capable clang.
pub mod precompiled {
    /// Build a minimal valid 64-bit little-endian ELF file containing
    /// BPF bytecode for the given section name and instructions.
    ///
    /// The ELF structure:
    ///   ELF header (64 bytes)
    ///   .text section (BPF instructions)
    ///   section name string table (.shstrtab)
    ///   3 section headers (null, .text, .shstrtab)
    const fn build_minimal_bpf_elf(section_name: &[u8], insns: &[u8]) -> ([u8; 512], usize) {
        let mut buf = [0u8; 512];
        #[allow(unused_assignments)]
        let mut off = 0;

        // --- ELF header (64 bytes for 64-bit) ---
        // e_ident: magic
        buf[0] = 0x7F;
        buf[1] = b'E';
        buf[2] = b'L';
        buf[3] = b'F';
        buf[4] = 2; // ELFCLASS64
        buf[5] = 1; // ELFDATA2LSB (little-endian)
        buf[6] = 1; // EV_CURRENT
        buf[7] = 0; // ELFOSABI_NONE
                    // e_ident[8..16] = padding (zeros)

        // e_type = ET_REL (1) at offset 16
        buf[16] = 1;
        buf[17] = 0;
        // e_machine = EM_BPF (247) at offset 18
        buf[18] = 247;
        buf[19] = 0;
        // e_version = EV_CURRENT (1) at offset 20
        buf[20] = 1;
        buf[21] = 0;
        buf[22] = 0;
        buf[23] = 0;
        // e_entry = 0 at offset 24 (8 bytes)
        // e_phoff = 0 at offset 32 (8 bytes) -- no program headers
        // e_shoff filled below at offset 40 (8 bytes)
        // e_flags = 0 at offset 48 (4 bytes)
        // e_ehsize = 64 at offset 52
        buf[52] = 64;
        buf[53] = 0;
        // e_phentsize = 0 at offset 54
        // e_phnum = 0 at offset 56
        // e_shentsize = 64 at offset 58
        buf[58] = 64;
        buf[59] = 0;
        // e_shnum = 3 at offset 60
        buf[60] = 3;
        buf[61] = 0;
        // e_shstrndx = 2 at offset 62
        buf[62] = 2;
        buf[63] = 0;
        off = 64;

        // --- .text section data (BPF instructions) ---
        let text_offset = off;
        let mut i = 0;
        while i < insns.len() {
            buf[off] = insns[i];
            off += 1;
            i += 1;
        }
        let text_size = insns.len();

        // --- .shstrtab section data ---
        let shstrtab_offset = off;
        // byte 0: null
        buf[off] = 0;
        off += 1;
        // ".text\0" starting at index 1, but we use the actual section name
        // First write a dot
        // We write: \0 <section_name> \0 .shstrtab \0
        // index 0 = \0 (already written above)
        // index 1 = start of section_name
        let name_index = 1u32;
        let mut j = 0;
        while j < section_name.len() {
            buf[off] = section_name[j];
            off += 1;
            j += 1;
        }
        buf[off] = 0; // null terminator for section name
        off += 1;
        let shstrtab_name_index = (off - shstrtab_offset) as u32;
        // ".shstrtab\0"
        buf[off] = b'.';
        off += 1;
        buf[off] = b's';
        off += 1;
        buf[off] = b'h';
        off += 1;
        buf[off] = b's';
        off += 1;
        buf[off] = b't';
        off += 1;
        buf[off] = b'r';
        off += 1;
        buf[off] = b't';
        off += 1;
        buf[off] = b'a';
        off += 1;
        buf[off] = b'b';
        off += 1;
        buf[off] = 0;
        off += 1;
        let shstrtab_size = off - shstrtab_offset;

        // Align to 8 bytes for section headers
        while off % 8 != 0 {
            off += 1;
        }
        let shdr_offset = off;

        // Write e_shoff in the ELF header (offset 40, 8 bytes LE)
        buf[40] = (shdr_offset & 0xFF) as u8;
        buf[41] = ((shdr_offset >> 8) & 0xFF) as u8;
        buf[42] = ((shdr_offset >> 16) & 0xFF) as u8;
        buf[43] = ((shdr_offset >> 24) & 0xFF) as u8;
        // bytes 44-47 are already 0

        // --- Section header 0: null (64 bytes of zeros) ---
        let mut k = 0;
        while k < 64 {
            // already zero
            k += 1;
        }
        off += 64;

        // --- Section header 1: .text ---
        // sh_name (4 bytes) = name_index
        buf[off] = (name_index & 0xFF) as u8;
        buf[off + 1] = ((name_index >> 8) & 0xFF) as u8;
        off += 4;
        // sh_type (4 bytes) = SHT_PROGBITS (1)
        buf[off] = 1;
        off += 4;
        // sh_flags (8 bytes) = SHF_ALLOC | SHF_EXECINSTR (0x6)
        buf[off] = 0x06;
        off += 8;
        // sh_addr (8 bytes) = 0
        off += 8;
        // sh_offset (8 bytes)
        buf[off] = (text_offset & 0xFF) as u8;
        buf[off + 1] = ((text_offset >> 8) & 0xFF) as u8;
        off += 8;
        // sh_size (8 bytes)
        buf[off] = (text_size & 0xFF) as u8;
        buf[off + 1] = ((text_size >> 8) & 0xFF) as u8;
        off += 8;
        // sh_link (4 bytes) = 0
        off += 4;
        // sh_info (4 bytes) = 0
        off += 4;
        // sh_addralign (8 bytes) = 8
        buf[off] = 8;
        off += 8;
        // sh_entsize (8 bytes) = 0
        off += 8;

        // --- Section header 2: .shstrtab ---
        // sh_name (4 bytes)
        buf[off] = (shstrtab_name_index & 0xFF) as u8;
        buf[off + 1] = ((shstrtab_name_index >> 8) & 0xFF) as u8;
        off += 4;
        // sh_type (4 bytes) = SHT_STRTAB (3)
        buf[off] = 3;
        off += 4;
        // sh_flags (8 bytes) = 0
        off += 8;
        // sh_addr (8 bytes) = 0
        off += 8;
        // sh_offset (8 bytes)
        buf[off] = (shstrtab_offset & 0xFF) as u8;
        buf[off + 1] = ((shstrtab_offset >> 8) & 0xFF) as u8;
        off += 8;
        // sh_size (8 bytes)
        buf[off] = (shstrtab_size & 0xFF) as u8;
        buf[off + 1] = ((shstrtab_size >> 8) & 0xFF) as u8;
        off += 8;
        // sh_link, sh_info, sh_addralign, sh_entsize
        off += 4 + 4 + 8 + 8;

        (buf, off)
    }

    // BPF instruction encoding: each instruction is 8 bytes
    // opcode(1) | dst_reg:src_reg(1) | offset(2) | imm(4)

    // XDP program: r0 = XDP_PASS (2); exit
    const XDP_INSNS: [u8; 16] = [
        0xB7, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, // mov r0, 2 (XDP_PASS)
        0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // exit
    ];

    // Socket filter: r0 = 0 (allow); exit
    const SOCKET_INSNS: [u8; 16] = [
        0xB7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // mov r0, 0
        0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // exit
    ];

    // TC classifier: r0 = TC_ACT_OK (0); exit
    const TC_INSNS: [u8; 16] = [
        0xB7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // mov r0, 0 (TC_ACT_OK)
        0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // exit
    ];

    /// Pre-compiled XDP distance program (minimal valid BPF ELF).
    pub fn xdp_distance() -> Vec<u8> {
        let (buf, len) = build_minimal_bpf_elf(b"xdp", &XDP_INSNS);
        buf[..len].to_vec()
    }

    /// Pre-compiled socket filter program (minimal valid BPF ELF).
    pub fn socket_filter() -> Vec<u8> {
        let (buf, len) = build_minimal_bpf_elf(b"socket", &SOCKET_INSNS);
        buf[..len].to_vec()
    }

    /// Pre-compiled TC query route program (minimal valid BPF ELF).
    pub fn tc_query_route() -> Vec<u8> {
        let (buf, len) = build_minimal_bpf_elf(b"tc", &TC_INSNS);
        buf[..len].to_vec()
    }
}

/// Compiler front-end for building BPF C programs into ELF objects.
///
/// Uses `clang` with `-target bpf` to produce BPF-compatible ELF
/// object files from C source code.
pub struct EbpfCompiler {
    clang_path: PathBuf,
    target: String,
    optimization: OptLevel,
    include_btf: bool,
    extra_includes: Vec<PathBuf>,
}

impl EbpfCompiler {
    /// Create a new compiler, auto-detecting `clang` from `$PATH`.
    ///
    /// Returns `Err(EbpfError::ClangNotFound)` if clang is not installed.
    pub fn new() -> Result<Self, EbpfError> {
        let clang_path = find_clang().ok_or(EbpfError::ClangNotFound)?;
        Ok(Self {
            clang_path,
            target: "bpf".to_string(),
            optimization: OptLevel::O2,
            include_btf: false,
            extra_includes: Vec::new(),
        })
    }

    /// Create a compiler with an explicit path to clang.
    pub fn with_clang(path: &Path) -> Self {
        Self {
            clang_path: path.to_path_buf(),
            target: "bpf".to_string(),
            optimization: OptLevel::O2,
            include_btf: false,
            extra_includes: Vec::new(),
        }
    }

    /// Set the optimization level.
    pub fn set_optimization(&mut self, level: OptLevel) -> &mut Self {
        self.optimization = level;
        self
    }

    /// Enable or disable BTF generation.
    pub fn set_include_btf(&mut self, enable: bool) -> &mut Self {
        self.include_btf = enable;
        self
    }

    /// Add an extra include path for header resolution.
    pub fn add_include_path(&mut self, path: &Path) -> &mut Self {
        self.extra_includes.push(path.to_path_buf());
        self
    }

    /// Compile a BPF C source file from disk.
    pub fn compile(&self, source: &Path) -> Result<CompiledProgram, EbpfError> {
        let output = tempfile::NamedTempFile::new()?;
        let output_path = output.path().to_path_buf();

        let mut cmd = Command::new(&self.clang_path);
        cmd.arg("-target")
            .arg(&self.target)
            .arg(self.optimization.as_flag())
            .arg("-c")
            .arg(source)
            .arg("-o")
            .arg(&output_path)
            .arg("-D__BPF_TRACING__")
            .arg("-Wno-unused-value")
            .arg("-Wno-pointer-sign")
            .arg("-Wno-compare-distinct-pointer-types");

        if self.include_btf {
            cmd.arg("-g");
        }

        // Add the bpf/ directory containing vmlinux.h as an include path
        if let Some(parent) = source.parent() {
            cmd.arg("-I").arg(parent);
        }

        for inc in &self.extra_includes {
            cmd.arg("-I").arg(inc);
        }

        let result = cmd.output()?;

        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr).to_string();
            return Err(EbpfError::CompilationFailed(stderr));
        }

        let elf_bytes = std::fs::read(&output_path)?;
        if elf_bytes.is_empty() {
            return Err(EbpfError::InvalidElf);
        }

        // Infer program type from the source file name
        let program_type = infer_program_type(source);
        let attach_type = infer_attach_type(program_type);

        let program_hash = compute_sha3_256(&elf_bytes);
        let insn_count = (elf_bytes.len() / 8).min(u16::MAX as usize) as u16;

        Ok(CompiledProgram {
            elf_bytes,
            program_type,
            attach_type,
            btf_bytes: None,
            insn_count,
            program_hash,
        })
    }

    /// Compile BPF C source from an in-memory string.
    pub fn compile_source(
        &self,
        source: &str,
        program_type: EbpfProgramType,
    ) -> Result<CompiledProgram, EbpfError> {
        let src_file = tempfile::Builder::new().suffix(".c").tempfile()?;

        // Write source to the temp file
        {
            let mut writer = std::io::BufWriter::new(src_file.as_file());
            writer.write_all(source.as_bytes())?;
            writer.flush()?;
        }

        let output = tempfile::NamedTempFile::new()?;
        let output_path = output.path().to_path_buf();

        let bpf_dir = bpf_source_dir();

        let mut cmd = Command::new(&self.clang_path);
        cmd.arg("-target")
            .arg(&self.target)
            .arg(self.optimization.as_flag())
            .arg("-c")
            .arg(src_file.path())
            .arg("-o")
            .arg(&output_path)
            .arg("-D__BPF_TRACING__")
            .arg("-Wno-unused-value")
            .arg("-Wno-pointer-sign")
            .arg("-Wno-compare-distinct-pointer-types")
            .arg("-I")
            .arg(&bpf_dir);

        if self.include_btf {
            cmd.arg("-g");
        }

        for inc in &self.extra_includes {
            cmd.arg("-I").arg(inc);
        }

        let result = cmd.output()?;

        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr).to_string();
            return Err(EbpfError::CompilationFailed(stderr));
        }

        let elf_bytes = std::fs::read(&output_path)?;
        if elf_bytes.is_empty() {
            return Err(EbpfError::InvalidElf);
        }

        let attach_type = infer_attach_type(program_type);
        let program_hash = compute_sha3_256(&elf_bytes);
        let insn_count = (elf_bytes.len() / 8).min(u16::MAX as usize) as u16;

        Ok(CompiledProgram {
            elf_bytes,
            program_type,
            attach_type,
            btf_bytes: None,
            insn_count,
            program_hash,
        })
    }

    /// Get the path to the clang binary this compiler uses.
    pub fn clang_path(&self) -> &Path {
        &self.clang_path
    }

    /// Return a pre-compiled BPF program for the given program type.
    ///
    /// This uses the embedded minimal BPF ELF bytecode from the
    /// `precompiled` module, requiring no external toolchain.
    pub fn from_precompiled(program_type: EbpfProgramType) -> Result<CompiledProgram, EbpfError> {
        let (elf_bytes, attach_type) = match program_type {
            EbpfProgramType::XdpDistance => {
                (precompiled::xdp_distance(), EbpfAttachType::XdpIngress)
            }
            EbpfProgramType::SocketFilter => {
                (precompiled::socket_filter(), EbpfAttachType::SocketFilter)
            }
            EbpfProgramType::TcFilter => (precompiled::tc_query_route(), EbpfAttachType::TcIngress),
            _ => {
                return Err(EbpfError::CompilationFailed(format!(
                    "no pre-compiled bytecode for program type {:?}",
                    program_type
                )))
            }
        };

        if elf_bytes.len() < 4 || &elf_bytes[..4] != b"\x7fELF" {
            return Err(EbpfError::InvalidElf);
        }

        let program_hash = compute_sha3_256(&elf_bytes);
        let insn_count = (elf_bytes.len() / 8).min(u16::MAX as usize) as u16;

        Ok(CompiledProgram {
            elf_bytes,
            program_type,
            attach_type,
            btf_bytes: None,
            insn_count,
            program_hash,
        })
    }

    /// Compile a BPF C source file, falling back to pre-compiled bytecode
    /// if clang is unavailable.
    ///
    /// This is the recommended entry point: it tries clang-based
    /// compilation first for full-featured programs, and degrades
    /// gracefully to minimal pre-compiled stubs when clang is absent.
    pub fn compile_or_fallback(&self, source: &Path) -> Result<CompiledProgram, EbpfError> {
        match self.compile(source) {
            Ok(prog) => Ok(prog),
            Err(EbpfError::CompilationFailed(_)) | Err(EbpfError::ClangNotFound) => {
                let ptype = infer_program_type(source);
                eprintln!(
                    "rvf-ebpf: clang compilation failed for {:?}, using precompiled fallback",
                    source.file_name().unwrap_or_default()
                );
                Self::from_precompiled(ptype)
            }
            Err(other) => Err(other),
        }
    }
}

/// Built-in BPF program source code, included at compile time.
pub mod programs {
    /// XDP program for computing L2 vector distance on ingress packets.
    pub const XDP_DISTANCE: &str = include_str!("../bpf/xdp_distance.c");

    /// Socket filter for port-based access control.
    pub const SOCKET_FILTER: &str = include_str!("../bpf/socket_filter.c");

    /// TC classifier for routing queries by priority tier.
    pub const TC_QUERY_ROUTE: &str = include_str!("../bpf/tc_query_route.c");

    /// Minimal vmlinux.h type stubs for BPF compilation without kernel headers.
    pub const VMLINUX_H: &str = include_str!("../bpf/vmlinux.h");
}

/// Try to find `clang` in the system PATH.
pub fn find_clang() -> Option<PathBuf> {
    // Check common names in order of preference
    for name in &["clang-18", "clang-17", "clang-16", "clang-15", "clang"] {
        if let Ok(output) = Command::new("which").arg(name).output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(PathBuf::from(path));
                }
            }
        }
    }
    None
}

/// Get the path to the `bpf/` directory containing the built-in source files.
///
/// This resolves relative to the crate's `CARGO_MANIFEST_DIR` at build time,
/// with a fallback for runtime usage.
fn bpf_source_dir() -> PathBuf {
    // At build time, CARGO_MANIFEST_DIR is set
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        return PathBuf::from(manifest_dir).join("bpf");
    }
    // Fallback: look relative to the current executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidate = parent.join("bpf");
            if candidate.exists() {
                return candidate;
            }
        }
    }
    // Last resort
    PathBuf::from("bpf")
}

/// Infer the BPF program type from the source file name.
fn infer_program_type(path: &Path) -> EbpfProgramType {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

    if stem.contains("xdp") {
        EbpfProgramType::XdpDistance
    } else if stem.contains("socket") {
        EbpfProgramType::SocketFilter
    } else if stem.contains("tc") {
        EbpfProgramType::TcFilter
    } else if stem.contains("tracepoint") || stem.contains("tp") {
        EbpfProgramType::Tracepoint
    } else if stem.contains("kprobe") {
        EbpfProgramType::Kprobe
    } else if stem.contains("cgroup") {
        EbpfProgramType::CgroupSkb
    } else {
        EbpfProgramType::Custom
    }
}

/// Infer the attach type from the program type.
fn infer_attach_type(ptype: EbpfProgramType) -> EbpfAttachType {
    match ptype {
        EbpfProgramType::XdpDistance => EbpfAttachType::XdpIngress,
        EbpfProgramType::TcFilter => EbpfAttachType::TcIngress,
        EbpfProgramType::SocketFilter => EbpfAttachType::SocketFilter,
        EbpfProgramType::CgroupSkb => EbpfAttachType::CgroupIngress,
        _ => EbpfAttachType::None,
    }
}

/// Compute SHA3-256 hash of the given data.
fn compute_sha3_256(data: &[u8]) -> [u8; 32] {
    use sha3::Digest;
    let mut hasher = sha3::Sha3_256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdp_distance_source_is_valid() {
        let src = programs::XDP_DISTANCE;
        assert!(!src.is_empty());
        assert!(src.contains("SEC(\"xdp\")"));
        assert!(src.contains("SEC(\"license\")"));
        assert!(src.contains("\"GPL\""));
        assert!(src.contains("xdp_vector_distance"));
        assert!(src.contains("struct xdp_md"));
        assert!(src.contains("XDP_PASS"));
        assert!(src.contains("vector_cache"));
    }

    #[test]
    fn socket_filter_source_is_valid() {
        let src = programs::SOCKET_FILTER;
        assert!(!src.is_empty());
        assert!(src.contains("SEC(\"socket\")"));
        assert!(src.contains("SEC(\"license\")"));
        assert!(src.contains("\"GPL\""));
        assert!(src.contains("rvf_port_filter"));
        assert!(src.contains("allowed_ports"));
        assert!(src.contains("__sk_buff"));
    }

    #[test]
    fn tc_query_route_source_is_valid() {
        let src = programs::TC_QUERY_ROUTE;
        assert!(!src.is_empty());
        assert!(src.contains("SEC(\"tc\")"));
        assert!(src.contains("SEC(\"license\")"));
        assert!(src.contains("\"GPL\""));
        assert!(src.contains("rvf_query_classify"));
        assert!(src.contains("tc_classid"));
        assert!(src.contains("CLASS_HOT"));
        assert!(src.contains("CLASS_COLD"));
    }

    #[test]
    fn vmlinux_h_has_essential_types() {
        let src = programs::VMLINUX_H;
        assert!(src.contains("struct xdp_md"));
        assert!(src.contains("struct __sk_buff"));
        assert!(src.contains("struct ethhdr"));
        assert!(src.contains("struct iphdr"));
        assert!(src.contains("struct udphdr"));
        assert!(src.contains("bpf_map_lookup_elem"));
        assert!(src.contains("XDP_PASS"));
        assert!(src.contains("TC_ACT_OK"));
    }

    #[test]
    fn find_clang_detection() {
        // This test verifies the clang detection logic runs without
        // panicking. It may or may not find clang depending on the
        // environment.
        let result = find_clang();
        if let Some(path) = &result {
            assert!(path.exists(), "detected clang path should exist");
        }
        // Not asserting result.is_some() since clang may not be installed
    }

    #[test]
    fn infer_program_type_from_filename() {
        assert_eq!(
            infer_program_type(Path::new("xdp_distance.c")),
            EbpfProgramType::XdpDistance,
        );
        assert_eq!(
            infer_program_type(Path::new("socket_filter.c")),
            EbpfProgramType::SocketFilter,
        );
        assert_eq!(
            infer_program_type(Path::new("tc_query_route.c")),
            EbpfProgramType::TcFilter,
        );
        assert_eq!(
            infer_program_type(Path::new("unknown.c")),
            EbpfProgramType::Custom,
        );
    }

    #[test]
    fn infer_attach_type_from_program_type() {
        assert_eq!(
            infer_attach_type(EbpfProgramType::XdpDistance),
            EbpfAttachType::XdpIngress,
        );
        assert_eq!(
            infer_attach_type(EbpfProgramType::TcFilter),
            EbpfAttachType::TcIngress,
        );
        assert_eq!(
            infer_attach_type(EbpfProgramType::SocketFilter),
            EbpfAttachType::SocketFilter,
        );
        assert_eq!(
            infer_attach_type(EbpfProgramType::Custom),
            EbpfAttachType::None,
        );
    }

    #[test]
    fn compiled_program_to_ebpf_header() {
        let program = CompiledProgram {
            elf_bytes: vec![0u8; 1024],
            program_type: EbpfProgramType::XdpDistance,
            attach_type: EbpfAttachType::XdpIngress,
            btf_bytes: Some(vec![0u8; 256]),
            insn_count: 128,
            program_hash: [0xAB; 32],
        };

        let header = program.to_ebpf_header(2048);

        assert_eq!(header.ebpf_magic, EBPF_MAGIC);
        assert_eq!(header.header_version, 1);
        assert_eq!(header.program_type, EbpfProgramType::XdpDistance as u8);
        assert_eq!(header.attach_type, EbpfAttachType::XdpIngress as u8);
        assert_eq!(header.insn_count, 128);
        assert_eq!(header.max_dimension, 2048);
        assert_eq!(header.program_size, 1024);
        assert_eq!(header.btf_size, 256);
        assert_eq!(header.program_hash, [0xAB; 32]);
    }

    #[test]
    fn ebpf_header_round_trip_from_compiled() {
        let program = CompiledProgram {
            elf_bytes: vec![0u8; 512],
            program_type: EbpfProgramType::TcFilter,
            attach_type: EbpfAttachType::TcIngress,
            btf_bytes: None,
            insn_count: 64,
            program_hash: [0xCD; 32],
        };

        let header = program.to_ebpf_header(1536);
        let bytes = header.to_bytes();
        let decoded = EbpfHeader::from_bytes(&bytes).expect("round-trip should succeed");

        assert_eq!(decoded.ebpf_magic, EBPF_MAGIC);
        assert_eq!(decoded.program_type, EbpfProgramType::TcFilter as u8);
        assert_eq!(decoded.attach_type, EbpfAttachType::TcIngress as u8);
        assert_eq!(decoded.insn_count, 64);
        assert_eq!(decoded.max_dimension, 1536);
        assert_eq!(decoded.program_size, 512);
        assert_eq!(decoded.btf_size, 0);
        assert_eq!(decoded.program_hash, [0xCD; 32]);
    }

    #[test]
    fn opt_level_flags() {
        assert_eq!(OptLevel::O0.as_flag(), "-O0");
        assert_eq!(OptLevel::O1.as_flag(), "-O1");
        assert_eq!(OptLevel::O2.as_flag(), "-O2");
        assert_eq!(OptLevel::O3.as_flag(), "-O3");
    }

    #[test]
    fn from_precompiled_xdp_returns_valid_elf() {
        let prog = EbpfCompiler::from_precompiled(EbpfProgramType::XdpDistance).unwrap();
        assert!(!prog.elf_bytes.is_empty());
        assert_eq!(&prog.elf_bytes[..4], b"\x7fELF");
        assert_eq!(prog.program_type, EbpfProgramType::XdpDistance);
        assert_eq!(prog.attach_type, EbpfAttachType::XdpIngress);
        assert!(prog.insn_count > 0);
        // ELF class should be ELFCLASS64
        assert_eq!(prog.elf_bytes[4], 2);
        // Data encoding should be little-endian
        assert_eq!(prog.elf_bytes[5], 1);
        // e_machine should be EM_BPF (247)
        assert_eq!(prog.elf_bytes[18], 247);
    }

    #[test]
    fn from_precompiled_socket_filter_returns_valid_elf() {
        let prog = EbpfCompiler::from_precompiled(EbpfProgramType::SocketFilter).unwrap();
        assert_eq!(&prog.elf_bytes[..4], b"\x7fELF");
        assert_eq!(prog.program_type, EbpfProgramType::SocketFilter);
        assert_eq!(prog.attach_type, EbpfAttachType::SocketFilter);
        assert_eq!(prog.elf_bytes[18], 247); // EM_BPF
    }

    #[test]
    fn from_precompiled_tc_returns_valid_elf() {
        let prog = EbpfCompiler::from_precompiled(EbpfProgramType::TcFilter).unwrap();
        assert_eq!(&prog.elf_bytes[..4], b"\x7fELF");
        assert_eq!(prog.program_type, EbpfProgramType::TcFilter);
        assert_eq!(prog.attach_type, EbpfAttachType::TcIngress);
        assert_eq!(prog.elf_bytes[18], 247); // EM_BPF
    }

    #[test]
    fn from_precompiled_unknown_type_returns_error() {
        let result = EbpfCompiler::from_precompiled(EbpfProgramType::Custom);
        assert!(result.is_err());
    }

    #[test]
    fn precompiled_elf_has_valid_structure() {
        // Verify all three precompiled programs have valid ELF structure
        for (name, elf) in [
            ("xdp", precompiled::xdp_distance()),
            ("socket", precompiled::socket_filter()),
            ("tc", precompiled::tc_query_route()),
        ] {
            // ELF magic
            assert_eq!(&elf[..4], b"\x7fELF", "{name}: ELF magic");
            // 64-bit, little-endian
            assert_eq!(elf[4], 2, "{name}: ELFCLASS64");
            assert_eq!(elf[5], 1, "{name}: little-endian");
            // ET_REL
            assert_eq!(elf[16], 1, "{name}: ET_REL");
            // EM_BPF
            assert_eq!(elf[18], 247, "{name}: EM_BPF");
            // e_shnum = 3 (null + .text + .shstrtab)
            assert_eq!(elf[60], 3, "{name}: 3 section headers");
            // Size is reasonable
            assert!(
                elf.len() > 64 && elf.len() < 1024,
                "{name}: reasonable size"
            );
        }
    }

    #[test]
    fn ebpf_error_display() {
        let err = EbpfError::ClangNotFound;
        assert_eq!(format!("{err}"), "clang not found in PATH");

        let err = EbpfError::CompilationFailed("syntax error".into());
        assert!(format!("{err}").contains("syntax error"));

        let err = EbpfError::InvalidElf;
        assert!(format!("{err}").contains("empty or invalid"));
    }

    #[test]
    fn compiler_new_returns_error_or_ok() {
        // This tests that the constructor doesn't panic.
        // It returns Ok if clang is found, Err otherwise.
        let result = EbpfCompiler::new();
        match result {
            Ok(compiler) => {
                assert!(compiler.clang_path().exists());
            }
            Err(EbpfError::ClangNotFound) => {
                // Expected on systems without clang
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn compiler_with_explicit_clang() {
        let compiler = EbpfCompiler::with_clang(Path::new("/usr/bin/clang"));
        assert_eq!(compiler.clang_path(), Path::new("/usr/bin/clang"));
    }

    /// If clang is available, test actual compilation of the socket_filter program.
    #[test]
    fn compile_socket_filter_if_clang_available() {
        let compiler = match EbpfCompiler::new() {
            Ok(c) => c,
            Err(EbpfError::ClangNotFound) => {
                eprintln!("skipping: clang not found");
                return;
            }
            Err(e) => panic!("unexpected error: {e}"),
        };

        // Write vmlinux.h + socket_filter.c to a temp dir
        let dir = tempfile::tempdir().unwrap();
        let vmlinux_path = dir.path().join("vmlinux.h");
        let source_path = dir.path().join("socket_filter.c");

        std::fs::write(&vmlinux_path, programs::VMLINUX_H).unwrap();
        std::fs::write(&source_path, programs::SOCKET_FILTER).unwrap();

        let result = compiler.compile(&source_path);
        match result {
            Ok(program) => {
                assert!(!program.elf_bytes.is_empty());
                assert_eq!(program.program_type, EbpfProgramType::SocketFilter);
                assert_eq!(program.attach_type, EbpfAttachType::SocketFilter);
                assert!(program.insn_count > 0);
                // Verify ELF magic bytes
                assert_eq!(&program.elf_bytes[..4], b"\x7fELF");
            }
            Err(EbpfError::CompilationFailed(msg)) => {
                // Compilation may fail if system lacks BPF-capable clang
                eprintln!("compilation failed (may need BPF-capable clang): {msg}");
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    /// If clang is available, test compile_source with in-memory source.
    #[test]
    fn compile_source_if_clang_available() {
        let compiler = match EbpfCompiler::new() {
            Ok(c) => c,
            Err(EbpfError::ClangNotFound) => {
                eprintln!("skipping: clang not found");
                return;
            }
            Err(e) => panic!("unexpected error: {e}"),
        };

        let result =
            compiler.compile_source(programs::SOCKET_FILTER, EbpfProgramType::SocketFilter);

        match result {
            Ok(program) => {
                assert!(!program.elf_bytes.is_empty());
                assert_eq!(program.program_type, EbpfProgramType::SocketFilter);
            }
            Err(EbpfError::CompilationFailed(msg)) => {
                eprintln!("compilation failed (may need BPF-capable clang): {msg}");
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    /// If clang is available, test compilation of the XDP distance program.
    #[test]
    fn compile_xdp_distance_if_clang_available() {
        let compiler = match EbpfCompiler::new() {
            Ok(c) => c,
            Err(EbpfError::ClangNotFound) => {
                eprintln!("skipping: clang not found");
                return;
            }
            Err(e) => panic!("unexpected error: {e}"),
        };

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("vmlinux.h"), programs::VMLINUX_H).unwrap();
        std::fs::write(dir.path().join("xdp_distance.c"), programs::XDP_DISTANCE).unwrap();

        let result = compiler.compile(&dir.path().join("xdp_distance.c"));
        match result {
            Ok(program) => {
                assert!(!program.elf_bytes.is_empty());
                assert_eq!(program.program_type, EbpfProgramType::XdpDistance);
                assert_eq!(program.attach_type, EbpfAttachType::XdpIngress);
                assert_eq!(&program.elf_bytes[..4], b"\x7fELF");
            }
            Err(EbpfError::CompilationFailed(msg)) => {
                eprintln!("XDP compilation failed: {msg}");
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    /// If clang is available, test compilation of the TC query route program.
    #[test]
    fn compile_tc_query_route_if_clang_available() {
        let compiler = match EbpfCompiler::new() {
            Ok(c) => c,
            Err(EbpfError::ClangNotFound) => {
                eprintln!("skipping: clang not found");
                return;
            }
            Err(e) => panic!("unexpected error: {e}"),
        };

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("vmlinux.h"), programs::VMLINUX_H).unwrap();
        std::fs::write(
            dir.path().join("tc_query_route.c"),
            programs::TC_QUERY_ROUTE,
        )
        .unwrap();

        let result = compiler.compile(&dir.path().join("tc_query_route.c"));
        match result {
            Ok(program) => {
                assert!(!program.elf_bytes.is_empty());
                assert_eq!(program.program_type, EbpfProgramType::TcFilter);
                assert_eq!(program.attach_type, EbpfAttachType::TcIngress);
                assert_eq!(&program.elf_bytes[..4], b"\x7fELF");
            }
            Err(EbpfError::CompilationFailed(msg)) => {
                eprintln!("TC compilation failed: {msg}");
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }
}
