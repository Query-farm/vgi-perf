//! Deterministic synthetic `perf.data` generation, for golden tests and the
//! SQL end-to-end suite.
//!
//! None of the upstream / real captures we test against record call chains
//! (`perf record` without `-g`), and the marquee feature of this worker is the
//! `callchain LIST(UBIGINT)` column plus the `ip ↔ mmap` range join. So this
//! module emits a tiny, hand-built but **spec-valid** file-mode `perf.data`
//! (magic `PERFILE2`, 104-byte header, one VER0 `cycles` hardware attribute) that
//! carries:
//!
//! - one `COMM` record (`loop`, pid 1234),
//! - two `MMAP2` records with **inline build-ids** (`/usr/bin/loop`,
//!   `/usr/lib/libc.so.6`),
//! - three `SAMPLE` records whose `ip`s fall inside those mappings and that carry
//!   known **call chains**.
//!
//! It deliberately writes an **empty feature bitmap**, so every `HEADER_*`
//! feature section is absent — exercising the worker's graceful degradation
//! (`meta` columns → `NULL`). The bytes are small enough to also embed inline as
//! a SQL `BLOB` literal, exercising the `bytes` source overload.

/// PERF_RECORD_* type numbers (kernel ABI).
const PERF_RECORD_COMM: u32 = 3;
const PERF_RECORD_SAMPLE: u32 = 9;
const PERF_RECORD_MMAP2: u32 = 10;

/// `PERF_RECORD_MISC_USER` (cpu mode) and `PERF_RECORD_MISC_MMAP_BUILD_ID`.
const PERF_RECORD_MISC_USER: u16 = 2;
const PERF_RECORD_MISC_MMAP_BUILD_ID: u16 = 0x4000;

/// `sample_type` = IP | TID | TIME | CALLCHAIN | CPU | PERIOD.
const SAMPLE_TYPE: u64 = 0x1 | 0x2 | 0x4 | 0x20 | 0x80 | 0x100;
/// `PERF_ATTR_SIZE_VER0` — the smallest valid `perf_event_attr`.
const ATTR_SIZE: u64 = 64;
/// `PERFILE2` file-mode header size.
const HEADER_SIZE: u64 = 104;

/// A sample's known shape in the generated file: `(ip, cpu, time, callchain)`.
/// Exposed so tests can assert against the exact values that were written.
pub struct SyntheticSample {
    pub ip: u64,
    pub cpu: u32,
    pub time: u64,
    pub period: u64,
    pub callchain: Vec<u64>,
}

/// A mapping's known shape: `(pid, addr, len, path, build_id)`.
pub struct SyntheticMmap {
    pub pid: i32,
    pub addr: u64,
    pub len: u64,
    pub path: &'static str,
    pub build_id: [u8; 20],
}

/// The mappings written into [`synthetic_callchain`], in declaration order.
pub fn synthetic_mmaps() -> Vec<SyntheticMmap> {
    vec![
        SyntheticMmap {
            pid: 1234,
            addr: 0x40_0000,
            len: 0x1000,
            path: "/usr/bin/loop",
            build_id: [0xaa; 20],
        },
        SyntheticMmap {
            pid: 1234,
            addr: 0x7f00_0000_0000,
            len: 0x20_0000,
            path: "/usr/lib/libc.so.6",
            build_id: [0xbb; 20],
        },
    ]
}

/// The samples written into [`synthetic_callchain`], in declaration order. Two
/// fall in `/usr/bin/loop` (`0x400100`), one in `libc` (`0x7f0000000100`).
pub fn synthetic_samples() -> Vec<SyntheticSample> {
    vec![
        SyntheticSample {
            ip: 0x40_0100,
            cpu: 2,
            time: 1000,
            period: 4000,
            callchain: vec![0x40_0100, 0x40_0050],
        },
        SyntheticSample {
            ip: 0x7f00_0000_0100,
            cpu: 2,
            time: 2000,
            period: 4000,
            callchain: vec![0x7f00_0000_0100, 0x40_0100],
        },
        SyntheticSample {
            ip: 0x40_0100,
            cpu: 3,
            time: 3000,
            period: 4000,
            callchain: vec![0x40_0100],
        },
    ]
}

/// The pid/tid/comm the single `COMM` record carries.
pub const SYNTHETIC_COMM: (i32, i32, &str) = (1234, 1234, "loop");

struct Buf(Vec<u8>);

impl Buf {
    fn new() -> Self {
        Buf(Vec::new())
    }
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn bytes(&mut self, v: &[u8]) {
        self.0.extend_from_slice(v);
    }
    fn len(&self) -> usize {
        self.0.len()
    }
}

/// Build one record: an 8-byte `perf_event_header` (type/misc/size) followed by
/// `body`, padded with NUL bytes to an 8-byte boundary. Returns the framed bytes.
fn record(record_type: u32, misc: u16, body: &[u8]) -> Vec<u8> {
    let mut total = 8 + body.len();
    let pad = (8 - (total % 8)) % 8;
    total += pad;
    let mut r = Buf::new();
    r.u32(record_type);
    r.u16(misc);
    r.u16(total as u16);
    r.bytes(body);
    r.bytes(&vec![0u8; pad]);
    r.0
}

fn comm_record() -> Vec<u8> {
    let (pid, tid, comm) = SYNTHETIC_COMM;
    let mut b = Buf::new();
    b.i32(pid);
    b.i32(tid);
    b.bytes(comm.as_bytes());
    b.u8(0); // NUL terminator
    record(PERF_RECORD_COMM, PERF_RECORD_MISC_USER, &b.0)
}

fn mmap2_record(m: &SyntheticMmap) -> Vec<u8> {
    let mut b = Buf::new();
    b.i32(m.pid);
    b.i32(m.pid); // tid == pid
    b.u64(m.addr);
    b.u64(m.len);
    b.u64(0); // pgoff
              // BuildId variant: len(1) + 2 align bytes + 20 build-id bytes.
    b.u8(20);
    b.u8(0);
    b.u16(0);
    b.bytes(&m.build_id);
    b.u32(0); // protection
    b.u32(0); // flags
    b.bytes(m.path.as_bytes());
    b.u8(0); // NUL terminator
    record(
        PERF_RECORD_MMAP2,
        PERF_RECORD_MISC_USER | PERF_RECORD_MISC_MMAP_BUILD_ID,
        &b.0,
    )
}

fn sample_record(s: &SyntheticSample) -> Vec<u8> {
    // Field order is the kernel ABI canonical order for SAMPLE_TYPE:
    // ip, (pid,tid), time, (cpu,res), period, callchain(nr + ips).
    let mut b = Buf::new();
    b.u64(s.ip);
    b.i32(1234); // pid
    b.i32(1234); // tid
    b.u64(s.time);
    b.u32(s.cpu);
    b.u32(0); // reserved
    b.u64(s.period);
    b.u64(s.callchain.len() as u64);
    for ip in &s.callchain {
        b.u64(*ip);
    }
    record(PERF_RECORD_SAMPLE, PERF_RECORD_MISC_USER, &b.0)
}

/// One `cycles` hardware `perf_event_attr` (VER0, 64 bytes), period-sampled at
/// 4000, with `sample_type = SAMPLE_TYPE`.
fn attr() -> Vec<u8> {
    let mut a = Buf::new();
    a.u32(0); // type_ = PERF_TYPE_HARDWARE
    a.u32(ATTR_SIZE as u32); // size = 64
    a.u64(0); // config = PERF_COUNT_HW_CPU_CYCLES
    a.u64(4000); // sample_period (no FREQ flag → period sampling)
    a.u64(SAMPLE_TYPE); // sample_type
    a.u64(0); // read_format
    a.u64(0); // flags
    a.u32(0); // wakeup_events
    a.u32(0); // bp_type
    a.u64(0); // config1
    debug_assert_eq!(a.len() as u64, ATTR_SIZE);
    a.0
}

/// Generate the complete synthetic `perf.data` byte image described in the module
/// docs. Deterministic: the same bytes every call.
pub fn synthetic_callchain() -> Vec<u8> {
    // Data section: COMM, the two MMAP2s, then the three SAMPLEs.
    let mut data = Buf::new();
    data.bytes(&comm_record());
    for m in synthetic_mmaps() {
        data.bytes(&mmap2_record(&m));
    }
    for s in synthetic_samples() {
        data.bytes(&sample_record(&s));
    }
    let data_bytes = data.0;

    let attr_offset = HEADER_SIZE;
    let data_offset = attr_offset + ATTR_SIZE;

    let mut out = Buf::new();
    out.bytes(b"PERFILE2");
    out.u64(HEADER_SIZE);
    out.u64(ATTR_SIZE);
    // attr_section { offset, size }
    out.u64(attr_offset);
    out.u64(ATTR_SIZE);
    // data_section { offset, size }
    out.u64(data_offset);
    out.u64(data_bytes.len() as u64);
    // event_types_section { offset, size } — none.
    out.u64(0);
    out.u64(0);
    // feature bitmap [u64; 4] — empty: no HEADER_* sections (graceful NULLs).
    out.u64(0);
    out.u64(0);
    out.u64(0);
    out.u64(0);
    debug_assert_eq!(out.len() as u64, HEADER_SIZE);

    out.bytes(&attr());
    out.bytes(&data_bytes);
    out.0
}
