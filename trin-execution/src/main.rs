use clap::Parser;
use tracing::info;
use trin_execution::{
    cli::{TrinExecutionConfig, TrinExecutionSubCommands, APP_NAME},
    engine::{service::EngineService, thread_manager::ThreadManager, utils::initialize_database},
    rpc::engine::EngineAuthServer,
    subcommands::era2::{export::StateExporter, import::StateImporter},
};
use trin_utils::{dir::setup_data_dir, log::init_tracing_logger};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing_logger();

    let trin_execution_config = TrinExecutionConfig::parse();

    // Initialize prometheus metrics
    if let Some(addr) = trin_execution_config.enable_metrics_with_url {
        prometheus_exporter::start(addr)?;
    }

    let data_dir = setup_data_dir(
        APP_NAME,
        trin_execution_config.data_dir.clone(),
        trin_execution_config.ephemeral,
    )?;

    if let Some(command) = trin_execution_config.command {
        match command {
            TrinExecutionSubCommands::ImportState(import_state_config) => {
                let state_importer = StateImporter::new(import_state_config, &data_dir).await?;
                let header = state_importer.import().await?;
                info!(
                    "Imported state from era2: {} {}",
                    header.number, header.state_root,
                );
                return Ok(());
            }
            TrinExecutionSubCommands::ExportState(export_state_config) => {
                let state_exporter = StateExporter::new(export_state_config, &data_dir).await?;
                state_exporter.export()?;
                info!(
                    "Exported state into era2: {} {}",
                    state_exporter.header().number,
                    state_exporter.header().state_root,
                );
                return Ok(());
            }
        }
    }

    let (execution_position, evm_db) =
        initialize_database(&data_dir, trin_execution_config.clone().into()).await?;
    let mut thread_manager = ThreadManager::new();
    let engine_tx = EngineService::spawn(
        &data_dir,
        execution_position.clone(),
        evm_db,
        &mut thread_manager,
    )
    .await;

    let engine_api_rpc_handle = EngineAuthServer::start(
        engine_tx,
        execution_position,
        &data_dir,
        trin_execution_config.clone(),
    )
    .await?;

    let join_handle = tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        info!("Received SIGINT, shutting down");
        let _ = engine_api_rpc_handle.stop();

        // Wait for all threads to finish
        thread_manager.shutdown_services().await;
    });

    join_handle.await?;

    Ok(())
}
