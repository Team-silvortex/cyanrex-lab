use cyanrex_engine::services::runner_agent_client::{run_runner_agent, RunnerAgentClientConfig};

#[tokio::main]
async fn main() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    match RunnerAgentClientConfig::from_env() {
        Ok(config) => {
            if let Err(error) = run_runner_agent(config).await {
                tracing::error!(%error, "Runner Agent stopped");
                std::process::exit(1);
            }
        }
        Err(error) => {
            tracing::error!(%error, "Runner Agent configuration failed");
            std::process::exit(2);
        }
    }
}
