use alloy::network::Network;
use alloy::primitives::Address;
use alloy::providers::ext::DebugApi;
use alloy::providers::Provider;
use alloy::rpc::types::trace::common::TraceResult;
use alloy::rpc::types::trace::geth::{
    GethDebugBuiltInTracerType::PreStateTracer,
    GethDebugTracerType::BuiltInTracer
};
use alloy::rpc::types::trace::geth::*;
use alloy::rpc::types::BlockNumberOrTag;
use alloy::transports::Transport;
use log::warn;
use std::collections::BTreeMap;
use std::sync::Arc;

// Trace the block to get all addresses with storage changes
pub async fn debug_trace_block<T: Transport + Clone, N: Network, P: Provider<T, N>>(
    client: Arc<P>,
    block_tag: BlockNumberOrTag,
    diff_mode: bool,
) -> Vec<BTreeMap<Address, AccountState>> {
    let tracer_opts = GethDebugTracingOptions {
        config: GethDefaultTracingOptions::default(),
        ..GethDebugTracingOptions::default()
    }
    .with_tracer(BuiltInTracer(PreStateTracer))
    .with_prestate_config(PreStateConfig {
        diff_mode: Some(diff_mode),
        disable_code: Some(false),
        disable_storage: Some(false),
    });
    let results = client
        .debug_trace_block_by_number(block_tag, tracer_opts)
        .await
        .unwrap();

    let mut post: Vec<BTreeMap<Address, AccountState>> = Vec::new();

    for trace_result in results.into_iter() {
        if let TraceResult::Success { result, .. } = trace_result {
            match result {
                GethTrace::PreStateTracer(PreStateFrame::Diff(diff_frame)) => {
                    post.push(diff_frame.post)
                }
                _ => warn!("Invalid trace"),
            }
        }
    }
    post
}
