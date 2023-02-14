#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use dao_primitives::*;
use frame_support::dispatch::DispatchError;
use serde::{self, Deserialize};
use serde_json::{json, Value};
use sp_runtime::{
	offchain,
	offchain::{
		storage::StorageValueRef,
		storage_lock::{BlockAndTime, StorageLock},
	},
	traits::BlockNumberProvider,
};
use sp_std::{marker::PhantomData, prelude::*, str, vec};

const ERC20_TOKEN_TOTAL_SUPPLY_SIGNATURE: &str =
	"0x18160ddd0000000000000000000000000000000000000000000000000000000000000000";
const ERC20_TOKEN_BALANCE_OF_SIGNATURE_PREFIX: &str = "0x70a08231000000000000000000000000";

const FETCH_TIMEOUT_PERIOD: u64 = 6000; // in milli-seconds
const LOCK_TIMEOUT_EXPIRATION: u64 = FETCH_TIMEOUT_PERIOD + 1000; // in milli-seconds
const LOCK_BLOCK_EXPIRATION: u32 = 3; // in block number

#[derive(Deserialize, Encode, Decode)]
pub struct EthRPCResponse {
	#[serde(deserialize_with = "de_string_to_bytes")]
	pub result: Vec<u8>,
}

pub struct HttpService<B: BlockNumberProvider + EthRpcProvider> {
	_phantom: PhantomData<B>,
}

impl<B: BlockNumberProvider + EthRpcProvider> EthHttpService for HttpService<B> {
	fn parse_block_number(
		response: Result<EthRPCResponse, DispatchError>,
	) -> Result<u32, DispatchError> {
		let result = response?.result;
		let value = Result::unwrap_or(str::from_utf8(&result), "");
		if value.is_empty() {
			return Err(DispatchError::Other("HttpFetchingError"))
		}

		let value_stripped = value.strip_prefix("0x").unwrap_or(value);
		let block_number = Result::unwrap_or(u32::from_str_radix(value_stripped, 16), 0_u32);

		Ok(block_number)
	}

	fn parse_token_balance(
		response: Result<EthRPCResponse, DispatchError>,
	) -> Result<u128, DispatchError> {
		let result = response?.result;

		let value = Result::unwrap_or(str::from_utf8(&result), "");
		let value_stripped = value.strip_prefix("0x").unwrap_or(value);

		let token_balance = Result::unwrap_or(u128::from_str_radix(value_stripped, 16), 0_u128);

		Ok(token_balance)
	}

	fn fetch_token_balance_of(
		token_address: Vec<u8>,
		account_id: Vec<u8>,
		block_number: Option<u32>,
	) -> Result<EthRPCResponse, DispatchError> {
		let to = str::from_utf8(&token_address[..]).expect("Failed to convert token address");

		let data = [
			ERC20_TOKEN_BALANCE_OF_SIGNATURE_PREFIX,
			str::from_utf8(&account_id[..]).map_err(|_| DispatchError::Other("InvalidInput"))?,
		]
		.concat();

		let params = json!([
			{
				"to": to,
				"data": data
			},
			Self::block_number(block_number)
		]);

		Self::fetch_from_eth(token_address, None, Some(params))
	}

	fn fetch_token_total_supply(
		token_address: Vec<u8>,
		block_number: Option<u32>,
	) -> Result<EthRPCResponse, DispatchError> {
		let to = str::from_utf8(&token_address[..]).expect("Failed to convert token address");
		let params = json!([
			{
				"to": to,
				"data": ERC20_TOKEN_TOTAL_SUPPLY_SIGNATURE
			},
			Self::block_number(block_number)
		]);
		Self::fetch_from_eth(token_address, None, Some(params))
	}

	fn fetch_from_eth(
		token_address: Vec<u8>,
		method: Option<Value>,
		params: Option<Value>,
	) -> Result<EthRPCResponse, DispatchError> {
		let key_suffix = &token_address[..];
		let key_vec = &[
			key_suffix,
			&b"::"[..2],
			&serde_json::to_vec(&method.clone().unwrap_or_else(|| json!(""))).map_err(|e| {
				log::error!("method parse error: {:?}", e);
				DispatchError::Other("HttpFetchingError")
			})?[..],
			&serde_json::to_vec(&params.clone().unwrap_or_else(|| json!(""))).map_err(|e| {
				log::error!("params parse error: {:?}", e);
				DispatchError::Other("HttpFetchingError")
			})?[..],
		]
		.concat()[..];
		let key = [&b"societal-dao::eth::"[..19], key_vec].concat();
		let s_info = StorageValueRef::persistent(&key[..]);

		if let Ok(Some(eth_rpc_response)) = s_info.get::<EthRPCResponse>() {
			return Ok(eth_rpc_response)
		}

		let lock_key = [&b"societal-dao::eth::lock::"[..25], key_vec].concat();
		let mut lock = StorageLock::<BlockAndTime<B>>::with_block_and_time_deadline(
			&lock_key[..],
			LOCK_BLOCK_EXPIRATION,
			offchain::Duration::from_millis(LOCK_TIMEOUT_EXPIRATION),
		);

		if let Ok(_guard) = lock.try_lock() {
			let json = json!({
				"jsonrpc": "2.0",
				"method": method.unwrap_or_else(|| json!("eth_call")),
				"id": 1,
				"params": params.unwrap_or_else(|| json!([]))
			});

			let body = &serde_json::to_vec(&json).expect("Failed to serialize")[..];

			match Self::fetch_n_parse(vec![body]) {
				Ok(eth_rpc_response) => {
					s_info.set(&eth_rpc_response);

					return Ok(eth_rpc_response)
				},
				Err(err) => return Err(err),
			}
		}

		Err(DispatchError::Other("HttpFetchingError"))
	}

	fn fetch_n_parse(body: Vec<&[u8]>) -> Result<EthRPCResponse, DispatchError> {
		let resp_bytes = Self::fetch_from_remote(body).map_err(|e| {
			log::error!("fetch_from_remote error: {:?}", e);
			DispatchError::Other("HttpFetchingError")
		})?;

		let resp_str =
			str::from_utf8(&resp_bytes).map_err(|_| DispatchError::Other("HttpFetchingError"))?;

		let eth_rpc_response: EthRPCResponse = serde_json::from_str(resp_str)
			.map_err(|_| DispatchError::Other("HttpFetchingError"))?;

		Ok(eth_rpc_response)
	}

	fn fetch_from_remote(body: Vec<&[u8]>) -> Result<Vec<u8>, DispatchError> {
		let eth_rpc_url = &B::get_rpc_url()[..];
		let request = offchain::http::Request::post(str::from_utf8(eth_rpc_url).unwrap(), body);

		// Keeping the offchain worker execution time reasonable, so limiting the call to be
		// within 6s.
		let timeout =
			sp_io::offchain::timestamp().add(offchain::Duration::from_millis(FETCH_TIMEOUT_PERIOD));

		let pending = request
			.deadline(timeout) // Setting the timeout time
			.send() // Sending the request out by the host
			.map_err(|_| DispatchError::Other("HttpFetchingError"))?;

		let response = pending
			.try_wait(timeout)
			.map_err(|_| DispatchError::Other("HttpFetchingError"))?
			.map_err(|_| DispatchError::Other("HttpFetchingError"))?;

		if response.code != 200 {
			log::error!("Unexpected http request status code: {}", response.code);
			return Err(DispatchError::Other("HttpFetchingError"))
		}

		Ok(response.body().collect::<Vec<u8>>())
	}

	fn block_number(block_number: Option<u32>) -> Value {
		json!(match block_number {
			None => "latest".into(),
			Some(block_number) => "0x".to_owned() + &hex::encode(block_number.to_be_bytes())[2..],
		})
	}
}

impl EthHttpService for () {
	fn parse_block_number(
		_response: Result<EthRPCResponse, DispatchError>,
	) -> Result<u32, DispatchError> {
		Ok(0)
	}

	fn parse_token_balance(
		_response: Result<EthRPCResponse, DispatchError>,
	) -> Result<u128, DispatchError> {
		Ok(0)
	}

	fn fetch_token_balance_of(
		_token_address: Vec<u8>,
		_account_id: Vec<u8>,
		_block_number: Option<u32>,
	) -> Result<EthRPCResponse, DispatchError> {
		Ok(EthRPCResponse { result: vec![] })
	}

	fn fetch_token_total_supply(
		_token_address: Vec<u8>,
		_block_number: Option<u32>,
	) -> Result<EthRPCResponse, DispatchError> {
		Ok(EthRPCResponse { result: vec![] })
	}

	fn fetch_from_eth(
		_token_address: Vec<u8>,
		_method: Option<Value>,
		_params: Option<Value>,
	) -> Result<EthRPCResponse, DispatchError> {
		Ok(EthRPCResponse { result: vec![] })
	}

	fn fetch_n_parse(_body: Vec<&[u8]>) -> Result<EthRPCResponse, DispatchError> {
		Ok(EthRPCResponse { result: vec![] })
	}

	fn fetch_from_remote(_body: Vec<&[u8]>) -> Result<Vec<u8>, DispatchError> {
		Ok(vec![])
	}

	fn block_number(_block_number: Option<u32>) -> Value {
		json!(0)
	}
}

pub trait EthHttpService {
	fn parse_block_number(
		response: Result<EthRPCResponse, DispatchError>,
	) -> Result<u32, DispatchError>;
	fn parse_token_balance(
		response: Result<EthRPCResponse, DispatchError>,
	) -> Result<u128, DispatchError>;
	fn fetch_token_balance_of(
		token_address: Vec<u8>,
		account_id: Vec<u8>,
		block_number: Option<u32>,
	) -> Result<EthRPCResponse, DispatchError>;
	fn fetch_token_total_supply(
		token_address: Vec<u8>,
		block_number: Option<u32>,
	) -> Result<EthRPCResponse, DispatchError>;
	fn fetch_from_eth(
		token_address: Vec<u8>,
		method: Option<Value>,
		params: Option<Value>,
	) -> Result<EthRPCResponse, DispatchError>;
	fn fetch_n_parse(body: Vec<&[u8]>) -> Result<EthRPCResponse, DispatchError>;
	fn fetch_from_remote(body: Vec<&[u8]>) -> Result<Vec<u8>, DispatchError>;
	fn block_number(block_number: Option<u32>) -> Value;
}

pub trait EthRpcProvider {
	fn get_rpc_url() -> Vec<u8>;
}
