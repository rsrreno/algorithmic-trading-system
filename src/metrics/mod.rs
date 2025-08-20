use anyhow::Result;

pub fn init(metrics_address: &str) -> Result<()> {
    tracing::info!("Metrics server initialized on {}", metrics_address);
    Ok(())
}
