use alloy::primitives::Address;
use alloy::providers::RootProvider;
use alloy::pubsub::PubSubFrontend;
use alloy::transports::http::{Client, Http};
use anyhow::Result;
use log::info;
use pool_sync::Chain;
use std::collections::HashSet;
use tokio::sync::broadcast::{Sender, Receiver};
use tokio::sync::broadcast;
use std::sync::Arc;
use crate::gas_manager::GasPriceManager;

use crate::stream::*;
use crate::graph::*;
/*
#[derive(Serialize, Deserialize)]
struct AddressSet(HashSet<Address>);
fn write_addresses_to_file(addresses: &HashSet<Address>, filename: &str) -> std::io::Result<()> {
    let file = File::create(filename)?;
    let writer = BufWriter::new(file);
    let address_set = AddressSet(addresses.clone());
    serde_json::to_writer(writer, &address_set)?;
    Ok(())
}

fn read_addresses_from_file(filename: &str) -> std::io::Result<HashSet<Address>> {
    let file = File::open(filename)?;
    let reader = BufReader::new(file);
    let address_set: AddressSet = serde_json::from_reader(reader)?;
    Ok(address_set.0)
}
*/

pub fn get_top_volume_tokens(chain: Chain) -> Result<HashSet<Address>> {
    todo!()
}

//let top_volume_tokens = read_addresses_from_file("addresses.json")?;
//let top_volume_tokens = filter_top_volume(pools.clone(), 3000, Chain::Ethereum).await;
//let mut top_volume_tokens = HashSet::from_iter(top_volume_tokens.into_iter());
//top_volume_tokens.insert(address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"));

//write_addresses_to_file(&top_volume_tokens, "eth_addresses.json").unwrap();
//
//

pub async fn start_workers(
    http: Arc<RootProvider<Http<Client>>>,
    ws: Arc<RootProvider<PubSubFrontend>>,
) {
    // Stream in new blocks
    info!("Starting block stream...");
    let (block_sender, mut block_receiver) = broadcast::channel(10);
    tokio::spawn(stream_new_blocks(ws.clone(), block_sender));


    // On each new block, parse sync events and update reserves
    info!("Starting sync event stream...");
    let (reserve_update_sender, mut reserve_update_receiver) = broadcast::channel(10);
    tokio::spawn(stream_sync_events(
        http.clone(),
        address_to_pool.clone(),
        block_receiver.resubscribe(),
        reserve_update_sender,
    ));


    // Update the gas on each block
    info!("Starting gas manager...");
    let gas_manager = Arc::new(GasPriceManager::new(http.clone(), 0.1, 100));
    let (gas_sender, mut gas_receiver) = broadcast::channel(10);
    tokio::spawn(async move {
        gas_manager
            .update_gas_price(block_receiver.resubscribe(), gas_sender)
            .await;
    });


    info!("Starting arb simulator...");
    // todo!()


    info!("Starting transaction sender...");
    //let (tx_sender, mut tx_receiver) = broadcast::channel(1000);
    //tokio::spawn(send_transactions(
    //    //signer_provider,
    //    tx_receiver.resubscribe(),
    //));

    // finally.... start the searcher!!!!!
    info!("Starting arbitrage searcher...");

    tokio::spawn(search_paths(
        graph,
        cycles,
        anvil_provider.clone(),
        address_to_pool,
        Arc::new(token_to_edge),
        reserve_update_receiver,
        tx_sender,
    ));
        */
}
