use crate::events::Event;
use log::info;
use tokio::sync::broadcast::{Receiver, Sender};

pub async fn simulate_path(sim_sender: Sender<Event>, mut opt_receiver: Receiver<Event>) {
    while let Ok(Event::OptimizedPath(opt_path)) = opt_receiver.recv().await {
        info!("Got a optimal path");
    }
}

/*

sol!(
    #[derive(Debug)]
    #[sol(rpc)]
    FlashSwap,
    "src/abi/FlashSwapper.json"
);

pub async fn test_sim(provider: Arc<RootProvider<Http<Client>>>) -> Result<()> {
    // spawn anvil instance
    let fork_block = provider.get_block_number().await.unwrap();
    let url = std::env::var("HTTP")?;
    let weth_addr = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");


    let url = "https://eth.merkle.io";
    let anvil = Anvil::new()
        .fork(url)
        .arg("--steps-tracing")
        .fork_block_number(fork_block)
        .try_spawn()?;
    let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
    let wallet = EthereumWallet::from(signer);
    let anvil_provider = Arc::new(
        ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet)
            .on_http(anvil.endpoint().parse()?),
    );

    let account = anvil.addresses()[0];
    let gweiyser = Gweisyer::new(anvil_provider.clone());


    // deploy the flash_swap contract
    let flash_swap = FlashSwap::deploy(anvil_provider.clone()).await?;
    // create the weth token
    let weth = gweiyser.token(weth_addr).await;

    // send some weth to the anvil account and check balance
    weth.deposit(ONE_ETH).await;
    let balance = weth.balance_of(account).await;
    println!("Balance of account: {:?}", balance);

    // approve contract and get allownace
    weth.approve(flash_swap.address().clone(), ONE_ETH).await;
    let allowance = weth.allowance(account, flash_swap.address().clone()).await;
    println!("Allowance of contract: {:?}", allowance);

    // execute the swap
    let swap_results = flash_swap.flashSwap( U256::from(1e14))
        .from(account)
        .call()
        .await;
    println!("Swap results: {:?}", swap_results);




    Ok(())
}

 */

