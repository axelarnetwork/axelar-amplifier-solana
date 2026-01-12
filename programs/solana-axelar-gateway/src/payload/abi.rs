use alloy_primitives::U256;
use alloy_sol_types::{
    abi::{
        token::{DynSeqToken, PackedSeqToken, WordToken},
        Decoder,
    },
    sol, SolValue,
};

use crate::payload::{AxelarMessagePayload, PayloadError, SolanaAccountRepr};

sol! {
    #[repr(C)]
    struct SolanaGatewayPayload {
        bytes execute_payload;
        SolanaAccountRepr[] accounts;
    }
}

impl<'payload> AxelarMessagePayload<'payload> {
    /// Encodes the payload using the ABI encoding scheme.
    ///
    /// The payload is encoded the following way:
    /// - single byte indicating the encoding scheme.
    /// - encoded: The first element is the payload without the accounts.
    /// - encoded: The second element is the list of Solana accounts.
    ///
    /// FIXME: this function is very inefficient because it allocates up to 5
    /// vectors.
    pub(super) fn encode_abi_encoding(&self) -> Result<Vec<u8>, PayloadError> {
        let mut writer_vec = self.encoding_scheme_prefixed_array()?;
        let gateway_payload = SolanaGatewayPayload {
            execute_payload: self.payload_without_accounts.to_vec().into(),
            accounts: self.solana_accounts.clone(),
        };

        let res = gateway_payload.abi_encode_params();

        // This is unoptimal because we allocate 2 vectors and then move the data from
        // one to the other.
        writer_vec.extend(&res);

        Ok(writer_vec)
    }

    /// Decodes ABI-encoded data with zero-copy payload handling.
    ///
    /// # Implementation Note
    /// Originally used full alloy decoding, but refactored for zero-copy of large payloads
    /// while maintaining copies only for the small `SolanaAccountRepr` structs.
    ///
    /// # Debug Verification
    /// Debug builds verify our manual decoding against alloy's full (but allocating) decode.
    pub(super) fn decode_abi_encoding(
        data: &'payload [u8],
    ) -> Result<(&'payload [u8], Vec<SolanaAccountRepr>), PayloadError> {
        let (payload, accounts) = extract_payload_slice_and_solana_accounts(data)?;

        // Verify our implementation matches alloy's copying/owned decode
        #[cfg(debug_assertions)]
        {
            let SolanaGatewayPayload {
                execute_payload: allocated_payload,
                accounts: allocated_accounts,
            } = SolanaGatewayPayload::abi_decode_params(data, true)
                .map_err(|_| PayloadError::AbiError)?;

            debug_assert_eq!(payload, allocated_payload.to_vec(), "bad payload");
            debug_assert_eq!(accounts, allocated_accounts, "bad accounts");
        }

        Ok((payload, accounts))
    }
}

/// Performs manual decoding of an ABI-encoded `SolanaGatewayPayload` into its constituent parts.
///
/// The main motivation for this function is to avoid heap allocation of the payload bytes.
/// It achieves this by returning a slice into the original `data` buffer for the payload
/// field, while only allocating memory for the smaller account metadata structures.
///
/// # ABI Structure
/// Decodes `data` conforming to `SolanaGatewayPayload`, which consists of a bytes field
/// followed by an array of account metadata records.
///
/// # Arguments
/// * `data` - ABI-encoded payload bytes, excluding the `EncodingScheme` byte
///
/// # Returns
/// * A tuple containing:
///   - A slice of the original payload bytes (zero-copy)
///   - A vector of decoded Solana account metadata (heap-allocated)
#[inline]
fn extract_payload_slice_and_solana_accounts(
    data: &[u8],
) -> Result<(&[u8], Vec<SolanaAccountRepr>), PayloadError> {
    let mut decoder = Decoder::new(data, true);
    let decoded_sequence = decoder
        .decode_sequence::<<SolanaGatewayPayload as alloy_sol_types::SolType>::Token<'_>>()
        .map_err(|_| PayloadError::AbiError)?;

    // The payload bytes are packed inside the first token, and we can use it directly.
    // Account info is listed inside the second token.
    let (PackedSeqToken(payload_slice), DynSeqToken(account_words)) = decoded_sequence;

    // Process each account's data, which has three components: pubkey, signer and write status
    // They are all represented by a single word (u256, which has 32 bytes).
    let mut accounts = Vec::with_capacity(account_words.len());
    for (WordToken(pubkey_token), WordToken(signer), WordToken(writable)) in account_words {
        let signer = U256::from_be_bytes(signer.0);
        if signer > U256::from(1) {
            return Err(PayloadError::AbiError);
        }

        let writable = U256::from_be_bytes(writable.0);
        if writable > U256::from(1) {
            return Err(PayloadError::AbiError);
        }

        accounts.push(SolanaAccountRepr {
            pubkey: pubkey_token,
            is_signer: signer == U256::from(1),
            is_writable: writable == U256::from(1),
        });
    }

    Ok((payload_slice, accounts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload::encoding::tests::{account_fixture, account_fixture_2};

    #[test]
    fn solana_account_repr_round_trip_abi() {
        let repr = account_fixture_2();
        let repr_encoded = repr.abi_encode();
        let repr2 = SolanaAccountRepr::abi_decode(&repr_encoded, true).unwrap();
        assert_eq!(repr, repr2);
    }

    #[test]
    fn account_serialization_abi() {
        let accounts = account_fixture().to_vec();
        let encoded = accounts.abi_encode();
        let decoded = Vec::<SolanaAccountRepr>::abi_decode(&encoded, true).unwrap();

        assert_eq!(accounts, decoded);
    }
}
