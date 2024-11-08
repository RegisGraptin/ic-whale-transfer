use std::{cell::RefCell, time::Duration};

use crate::{create_icp_signer, get_rpc_service_base, get_rpc_service_sepolia};

use alloy::{
    network::EthereumWallet,
    eips::BlockNumberOrTag,
    primitives::{address, Address, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::{Filter, Log},
    signers::Signer,
    sol,
    sol_types::SolEvent,
    transports::icp::IcpConfig,
};

use ic_cdk_timers::TimerId;

const POLL_LIMIT: usize = 3;

thread_local! {
    static NONCE: RefCell<Option<u64>> = const { RefCell::new(None) };
}

struct State {
    timer_id: Option<TimerId>,
    logs: Vec<String>,
    poll_count: usize,
}

impl State {
    fn default() -> State {
        State {
            // Store the id of the IC_CDK timer used for polling the EVM RPC periodically.
            // This id can be used to cancel the timer before the configured `POLL_LIMIT`
            // has been reached.
            timer_id: None,
            // The logs returned by the EVM are stored here for display in the frontend.
            logs: Vec::new(),
            // The number of polls made. Polls finish automatically, once the `POLL_LIMIT`
            // has been reached. This count is used to create a good interactive UI experience.
            poll_count: 0,
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}


// Codegen from ABI file to interact with the contract.
sol!(
    #[allow(missing_docs, clippy::too_many_arguments)]
    #[sol(rpc)]
    WhaleNFT,
    "src/abi/WhaleNFT.json"
);

sol!(
    #[allow(missing_docs, clippy::too_many_arguments)]
    #[sol(rpc)]
    USDC,
    "src/abi/USDC.json"
);

async fn mint_new_whale_nft(target_address: Address) -> Result<String, String> {

    // Setup signer
    let signer = create_icp_signer().await;
    let address = signer.address();

    // Setup provider
    let wallet = EthereumWallet::from(signer);
    let rpc_service = get_rpc_service_sepolia();
    let config = IcpConfig::new(rpc_service);
    let provider = ProviderBuilder::new()
        .with_gas_estimation()
        .wallet(wallet)
        .on_icp(config);

    // Attempt to get nonce from thread-local storage
    let maybe_nonce = NONCE.with_borrow(|maybe_nonce| {
        // If a nonce exists, the next nonce to use is latest nonce + 1
        maybe_nonce.map(|nonce| nonce + 1)
    });

    // If no nonce exists, get it from the provider
    let nonce = if let Some(nonce) = maybe_nonce {
        nonce
    } else {
        provider.get_transaction_count(address).await.unwrap_or(0)
    };

    // Mint a new NFT
    let contract = WhaleNFT::new(
        address!("63A0bfd6a5cdCF446ae12135E2CD86b908659568"),
        provider.clone(),
    );

    match contract
        .newWhale(target_address)
        .nonce(nonce)
        .chain_id(11155111)
        .from(address)
        .send()
        .await
    {
        Ok(builder) => {
            let node_hash = *builder.tx_hash();
            let tx_response = provider.get_transaction_by_hash(node_hash).await.unwrap();

            match tx_response {
                Some(tx) => {
                    // The transaction has been mined and included in a block, the nonce
                    // has been consumed. Save it to thread-local storage. Next transaction
                    // for this address will use a nonce that is = this nonce + 1
                    NONCE.with_borrow_mut(|nonce| {
                        *nonce = Some(tx.nonce);
                    });
                    Ok(format!("{:?}", tx))
                }
                None => Err("Could not get transaction.".to_string()),
            }
        }
        Err(e) => Err(format!("{:?}", e)),
    }

}

/// Using the ICP poller for Alloy allows smart contract canisters
/// to watch EVM blockchain changes easily. In this example, the canister
/// watches for USDC transfer logs.
#[ic_cdk::update]
async fn watch_usdc_transfer_start() -> Result<String, String> {
    // Don't start a timer if one is already running
    STATE.with_borrow(|state| {
        if state.timer_id.is_some() {
            return Err("Already watching for logs.".to_string());
        }
        Ok(())
    })?;

    let rpc_service = get_rpc_service_base();
    let config = IcpConfig::new(rpc_service).set_max_response_size(100_000);
    let provider = ProviderBuilder::new().on_icp(config);

    // This callback will be called every time new logs are received
    let callback = |incoming_logs: Vec<Log>| {
        STATE.with_borrow_mut(|state| async {
            for log in incoming_logs.iter() {
                let transfer: Log<USDC::Transfer> = log.log_decode().unwrap();
                let USDC::Transfer { from, to, value } = transfer.data();
                
                if value > &U256::from(1_000_000) {
                    let from_fmt = format!(
                        "0x{}...{}",
                        &from.to_string()[2..5],
                        &from.to_string()[from.to_string().len() - 3..]
                    );
                    let to_fmt = format!(
                        "0x{}...{}",
                        &to.to_string()[2..5],
                        &to.to_string()[to.to_string().len() - 3..]
                    );
                    state
                        .logs
                        .push(format!("{from_fmt} -> {to_fmt}, value: {value:?}"));

                    // Issue here as we have an async call data when we want to mint a NFT while pulling event
                    mint_new_whale_nft(*from).await;
                }
            }

            state.poll_count += 1;
            if state.poll_count >= POLL_LIMIT {
                state.timer_id.take();
            }
        })
    };

    // Clear the logs and poll count when starting a new watch
    STATE.with_borrow_mut(|state| {
        state.logs.clear();
        state.poll_count = 0;
    });

    let usdt_token_address = address!("833589fcd6edb6e08f4c7c32d4f71b54bda02913");
    let filter = Filter::new()
        .address(usdt_token_address)
        // By specifying an `event` or `event_signature` we listen for a specific event of the
        // contract. In this case the `Transfer(address,address,uint256)` event.
        .event(USDC::Transfer::SIGNATURE)
        .from_block(BlockNumberOrTag::Latest);

    // Initialize the poller and start watching
    // `with_limit` (optional) is used to limit the number of times to poll, defaults to 3
    // `with_poll_interval` (optional) is used to set the interval between polls, defaults to 7 seconds
    let poller = provider.watch_logs(&filter).await.unwrap();
    let timer_id = poller
        .with_limit(Some(POLL_LIMIT))
        .with_poll_interval(Duration::from_secs(10))
        .start(callback)
        .unwrap();

    // Save timer id to be able to stop watch before completion
    STATE.with_borrow_mut(|state| {
        state.timer_id = Some(timer_id);
    });

    Ok(format!("Watching for logs, polling {} times.", POLL_LIMIT))
}

/// Stop the watch before it reaches completion
#[ic_cdk::update]
async fn watch_usdc_transfer_stop() -> Result<String, String> {
    STATE.with_borrow_mut(|state| {
        if let Some(timer_id) = state.timer_id.take() {
            ic_cdk_timers::clear_timer(timer_id);
            Ok(())
        } else {
            Err("No timer to clear.".to_string())
        }
    })?;

    Ok("Watching for logs stopped.".to_string())
}

/// Returns a boolean that is `true` when watching and `false` otherwise.
#[ic_cdk::query]
async fn watch_usdc_transfer_is_polling() -> Result<bool, String> {
    STATE.with_borrow(|state| Ok(state.timer_id.is_some()))
}

/// Returns the number of polls made. Polls finish automatically, once the `POLL_LIMIT`
/// has been reached. This count is used to create a good interactive UI experience.
#[ic_cdk::query]
async fn watch_usdc_transfer_poll_count() -> Result<usize, String> {
    STATE.with_borrow(|state| Ok(state.poll_count))
}

/// Returns the list of logs returned by the watch. Gets reset on each start.
#[ic_cdk::query]
async fn watch_usdc_transfer_get() -> Result<Vec<String>, String> {
    STATE.with_borrow(|state| Ok(state.logs.iter().map(|log| format!("{log:?}")).collect()))
}

