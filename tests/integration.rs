use ethers::{abi::AbiEncode, prelude::*};
use mev_wallet::{MevTxBuilder, MevWalletV0, MEV_WETH_ADDR, TX_TYPEHASH};

abigen!(
    IERC20,
    r#"[
        function transfer(address recipient, uint256 amount) public returns (bool)
        function balanceOf(address who) public view returns (uint256)
    ]"#,
);

use std::sync::Arc;

static PROVIDER: Lazy<Arc<Provider<Http>>> =
    Lazy::new(|| Arc::new(Provider::new("http://127.0.0.1:8545".parse().unwrap())));

static KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

static WALLET_ADDR: Lazy<H160> = Lazy::new(|| {
    "0x7c73380fefdc7857aa7791cefac0ae1afdea57e3"
        .parse()
        .unwrap()
});

static SIGNER: Lazy<LocalWallet> =
    Lazy::new(|| KEY.parse::<LocalWallet>().unwrap().with_chain_id(31337u64));

static SIGNER_MWARE: Lazy<Arc<SignerMiddleware<Arc<Provider<Http>>, LocalWallet>>> =
    Lazy::new(|| Arc::new(SignerMiddleware::new(PROVIDER.clone(), SIGNER.clone())));

#[tokio::test]
async fn it() {
    assert_eq!(
        SIGNER.address(),
        "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
            .parse()
            .unwrap()
    );

    let mev_weth = IERC20::new(*MEV_WETH_ADDR, SIGNER_MWARE.clone());
    let wallet = MevWalletV0::new(*WALLET_ADDR, SIGNER_MWARE.clone());

    assert_eq!(wallet.owner().await.unwrap(), SIGNER.address(),);
    assert_eq!(*TX_TYPEHASH, wallet.tx_typehash().call().await.unwrap());

    if mev_weth
        .balance_of(wallet.address())
        .await
        .unwrap()
        .is_zero()
    {
        mev_weth
            .transfer(wallet.address(), ethers::utils::parse_ether("2").unwrap())
            .send()
            .await
            .unwrap()
            .await
            .unwrap();
    }

    let tx = MevTxBuilder::default()
        .data(
            TransferCall {
                recipient: H160::default(),
                amount: U256::one(),
            }
            .encode(),
        )
        .to(mev_weth.address())
        .tip(1_000_000_000)
        .max_base_fee(U256::from(1230) * 1_000_000_000)
        .with_signer(SIGNER.clone())
        .chain_id(31337)
        .populate(&wallet)
        .await
        .unwrap();

    let again = tx.clone();
    let tx = tx.build(wallet.address()).await.unwrap();

    let again = again
        .nonce(tx.tx().nonce + 1)
        .build(wallet.address())
        .await
        .unwrap();

    let call = wallet.send(tx);
    call.clone().await.unwrap();

    let receipt = call.send().await.unwrap().await.unwrap().unwrap();
    assert_eq!(receipt.status, Some(1u64.into()));

    let again_receipt = wallet
        .send(again)
        .send()
        .await
        .unwrap()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(again_receipt.status, Some(1u64.into()));
}
