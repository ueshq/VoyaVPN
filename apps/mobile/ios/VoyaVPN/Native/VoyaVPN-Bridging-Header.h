//
//  The C half of the uniffi bindings.
//
//  `Generated/voya_mobile_ffi.swift` opens with
//  `#if canImport(voya_mobile_ffiFFI)`, which is false in this target: the
//  generated module map is named `voya_mobile_ffiFFI.modulemap` rather than
//  `module.modulemap`, so Xcode never turns it into a Clang module. The Swift
//  therefore needs `RustBuffer`, `RustCallStatus` and the `uniffi_*` entry
//  points from the plain header, which is what this brings in.
//
//  Do not also make the module map discoverable. Declaring the same C symbols
//  twice — once as a module, once through here — is a redefinition error.
//

#import "voya_mobile_ffiFFI.h"
