use crate::{TxKind, TxType};
use alloy_eips::eip2930::AccessList;
use alloy_network::{Signed, Transaction};
use alloy_primitives::{keccak256, Bytes, ChainId, Signature, U256};
use alloy_rlp::{length_of_length, BufMut, Decodable, Encodable, Header};
use std::mem;

/// Transaction with an [`AccessList`] ([EIP-2930](https://eips.ethereum.org/EIPS/eip-2930)).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TxEip2930 {
    /// Added as EIP-pub 155: Simple replay attack protection
    pub chain_id: ChainId,
    /// A scalar value equal to the number of transactions sent by the sender; formally Tn.
    pub nonce: u64,
    /// A scalar value equal to the number of
    /// Wei to be paid per unit of gas for all computation
    /// costs incurred as a result of the execution of this transaction; formally Tp.
    ///
    /// As ethereum circulation is around 120mil eth as of 2022 that is around
    /// 120000000000000000000000000 wei we are safe to use u128 as its max number is:
    /// 340282366920938463463374607431768211455
    pub gas_price: u128,
    /// A scalar value equal to the maximum
    /// amount of gas that should be used in executing
    /// this transaction. This is paid up-front, before any
    /// computation is done and may not be increased
    /// later; formally Tg.
    pub gas_limit: u64,
    /// The 160-bit address of the message call’s recipient or, for a contract creation
    /// transaction, ∅, used here to denote the only member of B0 ; formally Tt.
    pub to: TxKind,
    /// A scalar value equal to the number of Wei to
    /// be transferred to the message call’s recipient or,
    /// in the case of contract creation, as an endowment
    /// to the newly created account; formally Tv.
    pub value: U256,
    /// The accessList specifies a list of addresses and storage keys;
    /// these addresses and storage keys are added into the `accessed_addresses`
    /// and `accessed_storage_keys` global sets (introduced in EIP-2929).
    /// A gas cost is charged, though at a discount relative to the cost of
    /// accessing outside the list.
    pub access_list: AccessList,
    /// Input has two uses depending if transaction is Create or Call (if `to` field is None or
    /// Some). pub init: An unlimited size byte array specifying the
    /// EVM-code for the account initialisation procedure CREATE,
    /// data: An unlimited size byte array specifying the
    /// input data of the message call, formally Td.
    pub input: Bytes,
}

impl TxEip2930 {
    /// Calculates a heuristic for the in-memory size of the [TxEip2930] transaction.
    #[inline]
    pub fn size(&self) -> usize {
        mem::size_of::<ChainId>() + // chain_id
        mem::size_of::<u64>() + // nonce
        mem::size_of::<u128>() + // gas_price
        mem::size_of::<u64>() + // gas_limit
        self.to.size() + // to
        mem::size_of::<U256>() + // value
        self.access_list.size() + // access_list
        self.input.len() // input
    }

    /// Decodes the inner [TxEip2930] fields from RLP bytes.
    ///
    /// NOTE: This assumes a RLP header has already been decoded, and _just_ decodes the following
    /// RLP fields in the following order:
    ///
    /// - `chain_id`
    /// - `nonce`
    /// - `gas_price`
    /// - `gas_limit`
    /// - `to`
    /// - `value`
    /// - `data` (`input`)
    /// - `access_list`
    pub(crate) fn decode_inner(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        Ok(Self {
            chain_id: Decodable::decode(buf)?,
            nonce: Decodable::decode(buf)?,
            gas_price: Decodable::decode(buf)?,
            gas_limit: Decodable::decode(buf)?,
            to: Decodable::decode(buf)?,
            value: Decodable::decode(buf)?,
            input: Decodable::decode(buf)?,
            access_list: Decodable::decode(buf)?,
        })
    }

    /// Outputs the length of the transaction's fields, without a RLP header.
    pub(crate) fn fields_len(&self) -> usize {
        let mut len = 0;
        len += self.chain_id.length();
        len += self.nonce.length();
        len += self.gas_price.length();
        len += self.gas_limit.length();
        len += self.to.length();
        len += self.value.length();
        len += self.input.0.length();
        len += self.access_list.length();
        len
    }

    /// Encodes only the transaction's fields into the desired buffer, without a RLP header.
    pub(crate) fn encode_fields(&self, out: &mut dyn BufMut) {
        self.chain_id.encode(out);
        self.nonce.encode(out);
        self.gas_price.encode(out);
        self.gas_limit.encode(out);
        self.to.encode(out);
        self.value.encode(out);
        self.input.0.encode(out);
        self.access_list.encode(out);
    }

    /// Inner encoding function that is used for both rlp [`Encodable`] trait and for calculating
    /// hash that for eip2718 does not require rlp header
    pub(crate) fn encode_with_signature(&self, signature: &Signature, out: &mut dyn BufMut) {
        let payload_length = self.fields_len() + signature.rlp_vrs_len();
        let header = Header { list: true, payload_length };
        header.encode(out);
        self.encode_fields(out);
        signature.write_rlp_vrs(out);
    }

    /// Output the length of the RLP signed transaction encoding, _without_ a RLP string header.
    pub fn payload_len_with_signature_without_header(&self, signature: &Signature) -> usize {
        let payload_length = self.fields_len() + signature.rlp_vrs_len();
        // 'transaction type byte length' + 'header length' + 'payload length'
        1 + length_of_length(payload_length) + payload_length
    }

    /// Output the length of the RLP signed transaction encoding. This encodes with a RLP header.
    pub fn payload_len_with_signature(&self, signature: &Signature) -> usize {
        let len = self.payload_len_with_signature_without_header(signature);
        length_of_length(len) + len
    }

    /// Get transaction type.
    pub const fn tx_type(&self) -> TxType {
        TxType::Eip2930
    }
}

impl Encodable for TxEip2930 {
    fn encode(&self, out: &mut dyn BufMut) {
        Header { list: true, payload_length: self.fields_len() }.encode(out);
        self.encode_fields(out);
    }

    fn length(&self) -> usize {
        let payload_length = self.fields_len();
        length_of_length(payload_length) + payload_length
    }
}

impl Decodable for TxEip2930 {
    fn decode(data: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let header = Header::decode(data)?;
        let remaining_len = data.len();

        if header.payload_length > remaining_len {
            return Err(alloy_rlp::Error::InputTooShort);
        }

        Self::decode_inner(data)
    }
}

impl Transaction for TxEip2930 {
    type Signature = Signature;
    // type Receipt = ReceiptWithBloom;

    fn encode_for_signing(&self, out: &mut dyn BufMut) {
        out.put_u8(self.tx_type() as u8);
        Header { list: true, payload_length: self.fields_len() }.encode(out);
        self.encode_fields(out);
    }

    fn payload_len_for_signature(&self) -> usize {
        let payload_length = self.fields_len();
        // 'transaction type byte length' + 'header length' + 'payload length'
        1 + length_of_length(payload_length) + payload_length
    }

    fn into_signed(self, signature: Signature) -> Signed<Self> {
        let payload_length = 1 + self.fields_len() + signature.rlp_vrs_len();
        let mut buf = Vec::with_capacity(payload_length);
        buf.put_u8(TxType::Eip2930 as u8);
        self.encode_signed(&signature, &mut buf);
        let hash = keccak256(&buf);

        // Drop any v chain id value to ensure the signature format is correct at the time of
        // combination for an EIP-2930 transaction. V should indicate the y-parity of the
        // signature.
        Signed::new_unchecked(self, signature.with_parity_bool(), hash)
    }

    fn encode_signed(&self, signature: &Signature, out: &mut dyn BufMut) {
        self.encode_with_signature(signature, out)
    }

    fn decode_signed(buf: &mut &[u8]) -> alloy_rlp::Result<alloy_network::Signed<Self>> {
        let header = Header::decode(buf)?;
        if !header.list {
            return Err(alloy_rlp::Error::UnexpectedString);
        }

        let tx = Self::decode_inner(buf)?;
        let signature = Signature::decode_rlp_vrs(buf)?;

        Ok(tx.into_signed(signature))
    }

    fn input(&self) -> &[u8] {
        &self.input
    }

    fn input_mut(&mut self) -> &mut Bytes {
        &mut self.input
    }

    fn set_input(&mut self, input: Bytes) {
        self.input = input;
    }

    fn to(&self) -> TxKind {
        self.to
    }

    fn set_to(&mut self, to: TxKind) {
        self.to = to;
    }

    fn value(&self) -> U256 {
        self.value
    }

    fn set_value(&mut self, value: U256) {
        self.value = value;
    }

    fn chain_id(&self) -> Option<ChainId> {
        Some(self.chain_id)
    }

    fn set_chain_id(&mut self, chain_id: ChainId) {
        self.chain_id = chain_id;
    }

    fn nonce(&self) -> u64 {
        self.nonce
    }

    fn set_nonce(&mut self, nonce: u64) {
        self.nonce = nonce;
    }

    fn gas_limit(&self) -> u64 {
        self.gas_limit
    }

    fn set_gas_limit(&mut self, limit: u64) {
        self.gas_limit = limit;
    }

    fn gas_price(&self) -> Option<U256> {
        Some(U256::from(self.gas_price))
    }

    fn set_gas_price(&mut self, price: U256) {
        if let Ok(price) = price.try_into() {
            self.gas_price = price;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TxEip2930;
    use crate::{TxEnvelope, TxKind};
    use alloy_network::{Signed, Transaction};
    use alloy_primitives::{Address, Bytes, Signature, U256};
    use alloy_rlp::{Decodable, Encodable};

    #[test]
    fn test_decode_create() {
        // tests that a contract creation tx encodes and decodes properly
        let request = TxEip2930 {
            chain_id: 1u64,
            nonce: 0,
            gas_price: 1,
            gas_limit: 2,
            to: TxKind::Create,
            value: U256::from(3_u64),
            input: Bytes::from(vec![1, 2]),
            access_list: Default::default(),
        };
        let signature = Signature::test_signature();

        let tx = request.into_signed(signature);

        let mut encoded = Vec::new();
        tx.encode(&mut encoded);
        assert_eq!(encoded.len(), tx.length());

        let decoded = Signed::decode(&mut &*encoded).unwrap();
        assert_eq!(decoded, tx);
    }

    #[test]
    fn test_decode_call() {
        let request = TxEip2930 {
            chain_id: 1u64,
            nonce: 0,
            gas_price: 1,
            gas_limit: 2,
            to: TxKind::Call(Address::default()),
            value: U256::from(3_u64),
            input: Bytes::from(vec![1, 2]),
            access_list: Default::default(),
        };

        let signature = Signature::test_signature();

        let tx = request.into_signed(signature);

        let envelope = TxEnvelope::Eip2930(tx);

        let mut encoded = Vec::new();
        envelope.encode(&mut encoded);
        assert_eq!(encoded.len(), envelope.length());

        assert_eq!(
            alloy_primitives::hex::encode(&encoded),
            "b86401f8610180010294000000000000000000000000000000000000000003820102c080a0840cfc572845f5786e702984c2a582528cad4b49b2a10b9db1be7fca90058565a025e7109ceb98168d95b09b18bbf6b685130e0562f233877d492b94eee0c5b6d1"
        );

        let decoded = TxEnvelope::decode(&mut encoded.as_ref()).unwrap();
        assert_eq!(decoded, envelope);
    }
}
