use crate::Writer;
use meron_core::{engine::Engine, protocol::Request};
use serde_json::Value;
use std::sync::Arc;

pub(crate) async fn dispatch(
    engine: &Arc<Engine>,
    req: &Request,
    _out: &Writer,
) -> anyhow::Result<Value> {
    let engine = engine.clone();
    let req = req.clone();
    tokio::task::spawn_blocking(move || {
        meron_core::openpgp::commands::dispatch(&engine, &req.method, &req.params)
    })
    .await?
}
