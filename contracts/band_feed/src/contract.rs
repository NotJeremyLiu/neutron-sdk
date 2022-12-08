#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Binary, Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult, SubMsg, Uint64};
use cw2::set_contract_version;
use neutron_sdk::bindings::msg::{NeutronMsg, IbcFee};
use neutron_sdk::bindings::types::ProtobufAny;
use cosmos_sdk_proto::cosmos::base::v1beta1::Coin as ProstCoin;
use neutron_sdk::NeutronResult;
use neutron_sdk::interchain_txs::helpers::get_port_id;
use neutron_sdk::bindings::query::InterchainQueries;
use neutron_sdk::bindings::types::KVKey;
use neutron_sdk::interchain_queries::types::QueryPayload;
use ::prost::Message;

use crate::storage::SudoPayload;
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:band_feed";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    deps.api.debug("WASMDEBUG: instantiate");
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> NeutronResult<Response<NeutronMsg>> {
  match msg {
      ExecuteMsg::AskPrice{} => execute_ask_price(deps, env),
  }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> NeutronResult<Binary>{
    match msg {
        QueryMsg::GetPrice{} => query_price(deps, env),
    }
}

fn execute_ask_price(mut deps: DepsMut, env: Env, ) -> NeutronResult<Response<NeutronMsg>> {
    let cosmos_msg = ask_price_msg()?;
    // We use a submessage here because we need the process message reply to save
    // the outgoing IBC packet identifier for later.
    let submsg = msg_with_sudo_callback(
        deps.branch(),
        cosmos_msg,
        SudoPayload {
            port_id: get_port_id(env.contract.address.as_str(), "TODO: interchain_account_id"),
            message: "message".to_string(),
        },
    )?;

    // TODO: this on ack where we have request_id
    // // TODO: is this correct? ask_price and create query right away!
    // let update_period = 15;
    // let connection_id = "TODO".to_string();
    // let msg = new_price_request_query_msg(connection_id, request_id, update_period)?;

    // Ok(Response::default().add_message(msg).add_submessages(vec![submsg]))

    Ok(Response::default().add_submessages(vec![submsg]))
}

fn query_price(_deps: Deps, _env: Env) -> NeutronResult<Binary>{
    unimplemented!()
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OracleRequestPacketData {
    #[prost(string, tag = "1")]
    pub client_id: ::prost::alloc::string::String,
    #[prost(uint64, tag = "2")]
    pub oracle_script_id: u64,
    #[prost(bytes = "vec", tag = "3")]
    pub calldata: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "4")]
    pub ask_count: u64,
    #[prost(uint64, tag = "5")]
    pub min_count: u64,
    #[prost(message, repeated, tag = "6")]
    pub fee_limit: ::prost::alloc::vec::Vec<ProstCoin>,
    #[prost(uint64, tag = "7")]
    pub prepare_gas: u64,
    #[prost(uint64, tag = "8")]
    pub execute_gas: u64,
}

fn ask_price_msg() -> StdResult<NeutronMsg> {
    let price_msg = OracleRequestPacketData{
        client_id: "TODO".to_string(),
        oracle_script_id: 0,
        calldata: vec![1,2,3],
        ask_count: 1,
        min_count: 1,
        fee_limit: vec![ProstCoin{amount: "1".to_string(), denom: "uatom".to_string()}],
        prepare_gas: 10000,
        execute_gas: 1000,
    };
    let mut buf = Vec::new();
    buf.reserve(price_msg.encoded_len());
    if let Err(e) = price_msg.encode(&mut buf) {
        return Err(StdError::generic_err(format!("Encode error: {}", e)));
    }

    let any_msg = ProtobufAny {
        // FIXME
        type_url: "/oracle.types.Request".to_string(),
        value: Binary::from(buf),
    };

    // FIXME
    let fee = IbcFee{
        recv_fee: vec![Coin::new(100, "uatom")],
        ack_fee: vec![Coin::new(100, "uatom")],
        timeout_fee: vec![Coin::new(100, "uatom")],
    };

    let interchain_account_id = "todo";
    let timeout: u64 = 60 * 60 * 24 * 7 * 2;


    Ok(NeutronMsg::submit_tx(
        "connection_idTODO".to_string(),
        interchain_account_id.clone().to_string(),
        vec![any_msg],
        "".to_string(),
        timeout,
        fee,
    ))
}

pub fn new_price_request_query_msg(
    connection_id: String,
    request_id: Uint64,
    update_period: u64,
) -> NeutronResult<NeutronMsg> {
    let mut kv_keys: Vec<KVKey> = Vec::with_capacity(1);

    let supply_key = create_request_key(request_id)?;

    let kv_key = KVKey {
        path: BAND_STORE_KEY.to_string(),
        key: Binary(supply_key),
    };

    kv_keys.push(kv_key);

    NeutronMsg::register_interchain_query(QueryPayload::KV(kv_keys), connection_id, update_period)
}

const BAND_STORE_KEY: &str = "oracle";
// Keys for stored bandchain requests
// https://github.com/bandprotocol/chain/blob/v2.4.1/x/oracle/types/keys.go#L46
const REQUEST_KEY: u8 = 0x01;

// Creates Cosmos-sdk storage key for request with given request_id in bandchain
// https://github.com/bandprotocol/chain/blob/v2.4.1/x/oracle/types/keys.go#L63
fn create_request_key(
    request_id: Uint64,
) -> NeutronResult<Vec<u8>> {
    let mut key: Vec<u8> = vec![REQUEST_KEY];
    key.extend_from_slice(request_id.to_be_bytes().as_slice());

    Ok(key)
}

#[cfg(test)]
mod tests {}

fn msg_with_sudo_callback<C: Into<CosmosMsg<T>>, T>(
    deps: DepsMut,
    msg: C,
    payload: SudoPayload,
) -> StdResult<SubMsg<T>> {
    crate::storage::save_reply_payload(deps.storage, payload)?;
    Ok(SubMsg::reply_on_success(msg, crate::storage::SUDO_PAYLOAD_REPLY_ID))
}

