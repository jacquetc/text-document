
# CI Steps
# taken from Clap https://github.com/clap-rs/clap/blob/master/Makefile
#
# Considerations
# - Easy to debug: show the command being run
# - Leverage CI features: Only run individual steps so we can use features like reporting elapsed time per step

ARGS?=--workspace
TOOLCHAIN_TARGET ?=
ifneq (${TOOLCHAIN_TARGET},)
  ARGS+=--target ${TOOLCHAIN_TARGET}
endif

MSRV?=1.61

_FEATURES =
_FEATURES_minimal = --no-default-features
_FEATURES_default =
_FEATURES_wasm = 
_FEATURES_full =
_FEATURES_next = ${_FEATURES_full}
_FEATURES_debug = ${_FEATURES_full}
_FEATURES_release = ${_FEATURES_full} --release

check-%:
	cargo check ${_FEATURES_${@:check-%=%}} --all-targets ${ARGS}

build-%:
	cargo test ${_FEATURES_${@:build-%=%}} --all-targets --no-run ${ARGS}

test-%:
	cargo test ${_FEATURES_${@:test-%=%}} ${ARGS}

clippy-%:
	cargo clippy ${_FEATURES_${@:clippy-%=%}} ${ARGS} --all-targets -- -D warnings -A deprecated

doc:
	cargo doc --workspace --all-features --no-deps --document-private-items