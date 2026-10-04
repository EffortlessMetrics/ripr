//! Clean pip/uv wheel consumer journey for issue #4626.
//!
//! Discriminating oracle tests always run. The live Unix journey installs a
//! local wheel of the candidate `ripr` binary through pip and uv, analyzes the
//! Python no-config fixture, follows the product-emitted `ripr explain`
//! continuation, and exercises explicit-config / PATH / lifecycle controls.
//!
//! This is not the Maturin adapter (#4490), admitted payload qualification
//! (#4489), or CI orchestration (#4493).

#![cfg(feature = "lang-python")]

mod identity;
mod isolation;
#[cfg(unix)]
mod live;
mod oracles;
#[cfg(unix)]
mod support;

#[cfg(unix)]
#[test]
fn pip_and_uv_install_a_local_wheel_and_analyze_python_without_rust_or_ambient_ripr()
-> Result<(), String> {
    live::run_live_pip_and_uv_journey()
}
