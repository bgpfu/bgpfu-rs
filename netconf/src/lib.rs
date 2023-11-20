//! Netconf client library
#![doc(html_root_url = "https://docs.rs/bgpfu-netconf/0.1.0-alpha.2")]
// clippy lints
#![warn(clippy::pedantic)]
#![warn(clippy::cargo)]
#![warn(clippy::nursery)]
#![allow(clippy::redundant_pub_crate)]
#![allow(clippy::multiple_crate_versions)]
// rustc lints
#![allow(box_pointers)]
#![warn(absolute_paths_not_starting_with_crate)]
#![warn(deprecated_in_future)]
#![warn(elided_lifetimes_in_paths)]
#![warn(explicit_outlives_requirements)]
#![warn(keyword_idents)]
#![warn(macro_use_extern_crate)]
#![warn(meta_variable_misuse)]
#![warn(missing_abi)]
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(non_ascii_idents)]
#![warn(noop_method_call)]
#![warn(pointer_structural_match)]
#![warn(rust_2021_incompatible_closure_captures)]
#![warn(rust_2021_incompatible_or_patterns)]
#![warn(rust_2021_prefixes_incompatible_syntax)]
#![warn(rust_2021_prelude_collisions)]
#![warn(single_use_lifetimes)]
#![warn(trivial_casts)]
#![warn(trivial_numeric_casts)]
#![warn(unreachable_pub)]
#![warn(unsafe_code)]
#![warn(unsafe_op_in_unsafe_fn)]
#![warn(unstable_features)]
#![warn(unused_crate_dependencies)]
#![warn(unused_extern_crates)]
#![warn(unused_import_braces)]
#![warn(unused_lifetimes)]
#![warn(unused_qualifications)]
#![warn(unused_results)]
#![warn(variant_size_differences)]
// docs.rs build config
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[allow(missing_docs, clippy::missing_errors_doc)]
mod error;
pub use self::error::Error;

#[allow(missing_docs, clippy::missing_errors_doc)]
pub mod message;

#[allow(missing_docs, clippy::missing_errors_doc)]
pub mod transport;

#[allow(missing_docs, clippy::missing_errors_doc)]
mod session;
pub use self::session::Session;

// silence unused dev-dependency warnings
#[cfg(test)]
mod deps {
    use anyhow as _;
    use clap as _;
    use clap_verbosity_flag as _;
    use simplelog as _;
    use version_sync as _;
}
