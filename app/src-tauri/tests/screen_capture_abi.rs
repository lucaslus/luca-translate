//! Reproduce the core-graphics 0.24 permission ABI bug without accessing TCC.
//! Apple's APIs return C `bool`: only AL is significant on x86_64. The upper
//! bits below deliberately differ from zero, as a real callee is allowed to do.
#![cfg(all(target_os = "macos", target_arch = "x86_64"))]

core::arch::global_asm!(
    ".globl _CGPreflightScreenCaptureAccess",
    "_CGPreflightScreenCaptureAccess:",
    "mov eax, 0x123401",
    "ret",
    ".globl _CGRequestScreenCaptureAccess",
    "_CGRequestScreenCaptureAccess:",
    "mov eax, 0x123400",
    "ret",
);

#[test]
fn granted_permission_reads_only_the_c_bool_return_value() {
    assert!(core_graphics::access::ScreenCaptureAccess.preflight());
}

#[test]
fn denied_permission_is_not_inferred_from_upper_register_bits() {
    assert!(!core_graphics::access::ScreenCaptureAccess.request());
}
