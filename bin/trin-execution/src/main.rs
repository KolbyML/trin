use std::sync::Arc;

use alloy_chains::Chain;
use clap::Parser;
use tracing::info;
use trin_execution::{
    cli::{TrinExecutionConfig, TrinExecutionSubCommands, APP_NAME},
    engine::{service::EngineService, thread_manager::ThreadManager, utils::initialize_database},
    rpc::{engine::EngineAuthServer, RpcServer},
    storage::block::BlockStorage,
    subcommands::{
        era2::{export::StateExporter, import::StateImporter},
        init::InitState,
        state_gossip_stats::StateGossipStats,
    },
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
            }
            TrinExecutionSubCommands::ExportState(export_state_config) => {
                let state_exporter = StateExporter::new(export_state_config, &data_dir).await?;
                state_exporter.export()?;
                info!(
                    "Exported state into era2: {} {}",
                    state_exporter.header().number,
                    state_exporter.header().state_root,
                );
            }
            TrinExecutionSubCommands::StateGossipStats => {
                let mut state_gossip_stats = StateGossipStats::new(&data_dir)?;
                state_gossip_stats.run()?;
            }
            TrinExecutionSubCommands::Init => {
                let mut init_state = InitState::new(&data_dir)?;
                init_state.run(
                    trin_execution_config.chain,
                    trin_execution_config.save_blocks,
                )?;
            }
            TrinExecutionSubCommands::Import(_) => {
                // Do nothing
            }
        }
        return Ok(());
    }

    if trin_execution_config.beacon_api_endpoint.is_none()
        && trin_execution_config.chain.chain == Chain::mainnet()
    {
        panic!("Beacon API endpoint is required set it using: --beacon-api-endpoint");
    }

    let (execution_position, evm_db, db) =
        initialize_database(&data_dir, trin_execution_config.clone().into()).await?;
    let mut thread_manager = ThreadManager::new();
    let engine_tx = EngineService::spawn(
        &data_dir,
        execution_position.clone(),
        db.clone(),
        evm_db,
        trin_execution_config.clone(),
        &mut thread_manager,
    )
    .await;

    let block_storage = Arc::new(BlockStorage::new(db.clone()));

    // Start the Engine API server
    let engine_api_rpc_handle = EngineAuthServer::start(
        engine_tx,
        execution_position.clone(),
        &data_dir,
        trin_execution_config.clone(),
    )
    .await?;

    // Start API server
    let api_handle = if trin_execution_config.http {
        Some(
            RpcServer::start(
                execution_position,
                trin_execution_config.clone(),
                block_storage,
            )
            .await?,
        )
    } else {
        None
    };

    let join_handle = tokio::spawn(async move {
        tokio::signal::ctrl_c().await.unwrap();
        info!("Received SIGINT, shutting down");
        let _ = engine_api_rpc_handle.stop();
        if let Some(api_handle) = api_handle {
            let _ = api_handle.stop();
        }

        // Wait for all threads to finish
        thread_manager.shutdown_services().await;
    });

    join_handle.await?;

    info!("Trin Execution shutdown successfully");
    Ok(())
}
