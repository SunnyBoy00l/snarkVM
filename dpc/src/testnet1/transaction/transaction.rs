// Copyright (C) 2019-2021 Aleo Systems Inc.
// This file is part of the snarkVM library.

// The snarkVM library is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The snarkVM library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with the snarkVM library. If not, see <https://www.gnu.org/licenses/>.

use crate::{
    testnet1::{record::encrypted::*, Testnet1Components},
    traits::TransactionScheme,
    AleoAmount,
    Network,
    TransactionError,
};
use snarkvm_algorithms::{
    merkle_tree::MerkleTreeDigest,
    traits::{CommitmentScheme, SignatureScheme, CRH, SNARK},
};
use snarkvm_utilities::{
    serialize::{CanonicalDeserialize, CanonicalSerialize},
    to_bytes_le,
    FromBytes,
    ToBytes,
};

use blake2::{digest::Digest, Blake2s as b2s};
use std::{
    fmt,
    io::{Read, Result as IoResult, Write},
};

#[derive(Derivative)]
#[derivative(
    Clone(bound = "C: Testnet1Components"),
    PartialEq(bound = "C: Testnet1Components"),
    Eq(bound = "C: Testnet1Components")
)]
pub struct Transaction<C: Testnet1Components> {
    /// The network this transaction is included in
    pub network: Network,

    /// The root of the ledger commitment Merkle tree
    pub ledger_digest: MerkleTreeDigest<C::LedgerMerkleTreeParameters>,

    /// The serial numbers of the records being spend
    pub old_serial_numbers: Vec<<C::AccountSignature as SignatureScheme>::PublicKey>,

    /// The commitment of the new records
    pub new_commitments: Vec<<C::RecordCommitment as CommitmentScheme>::Output>,

    #[derivative(PartialEq = "ignore")]
    /// The commitment to the old record death and new record birth programs
    pub program_commitment: <C::ProgramIDCommitment as CommitmentScheme>::Output,

    #[derivative(PartialEq = "ignore")]
    /// The root of the local data merkle tree
    pub local_data_root: <C::LocalDataCRH as CRH>::Output,

    /// A transaction value balance is the difference between input and output record balances.
    /// This value effectively becomes the transaction fee for the miner. Only coinbase transactions
    /// can have a negative value balance representing tokens being minted.
    pub value_balance: AleoAmount,

    #[derivative(PartialEq = "ignore")]
    /// Randomized signatures that allow for authorized delegation of transaction generation
    pub signatures: Vec<<C::AccountSignature as SignatureScheme>::Signature>,

    /// Encrypted record and selector bits of the new records generated by the transaction
    pub encrypted_records: Vec<EncryptedRecord<C>>,

    #[derivative(PartialEq = "ignore")]
    /// Zero-knowledge proof attesting to the valididty of the transaction
    pub transaction_proof: <C::OuterSNARK as SNARK>::Proof,

    /// Public data associated with the transaction that must be unique among all transactions
    pub memorandum: [u8; 32],

    /// The ID of the inner SNARK being used
    pub inner_circuit_id: <C::InnerCircuitIDCRH as CRH>::Output,
}

impl<C: Testnet1Components> Transaction<C> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        old_serial_numbers: Vec<<Self as TransactionScheme>::SerialNumber>,
        new_commitments: Vec<<Self as TransactionScheme>::Commitment>,
        memorandum: <Self as TransactionScheme>::Memorandum,
        ledger_digest: MerkleTreeDigest<C::LedgerMerkleTreeParameters>,
        inner_circuit_id: <C::InnerCircuitIDCRH as CRH>::Output,
        transaction_proof: <C::OuterSNARK as SNARK>::Proof,
        program_commitment: <C::ProgramIDCommitment as CommitmentScheme>::Output,
        local_data_root: <C::LocalDataCRH as CRH>::Output,
        value_balance: AleoAmount,
        network: Network,
        signatures: Vec<<C::AccountSignature as SignatureScheme>::Signature>,
        encrypted_records: Vec<EncryptedRecord<C>>,
    ) -> Self {
        assert_eq!(C::NUM_INPUT_RECORDS, old_serial_numbers.len());
        assert_eq!(C::NUM_OUTPUT_RECORDS, new_commitments.len());
        assert_eq!(C::NUM_INPUT_RECORDS, signatures.len());
        assert_eq!(C::NUM_OUTPUT_RECORDS, encrypted_records.len());

        Self {
            old_serial_numbers,
            new_commitments,
            memorandum,
            ledger_digest,
            inner_circuit_id,
            transaction_proof,
            program_commitment,
            local_data_root,
            value_balance,
            network,
            signatures,
            encrypted_records,
        }
    }
}

impl<C: Testnet1Components> TransactionScheme for Transaction<C> {
    type Commitment = <C::RecordCommitment as CommitmentScheme>::Output;
    type Digest = MerkleTreeDigest<C::LedgerMerkleTreeParameters>;
    type EncryptedRecord = EncryptedRecord<C>;
    type InnerCircuitID = <C::InnerCircuitIDCRH as CRH>::Output;
    type LocalDataRoot = <C::LocalDataCRH as CRH>::Output;
    type Memorandum = [u8; 32];
    type ProgramCommitment = <C::ProgramIDCommitment as CommitmentScheme>::Output;
    type SerialNumber = <C::AccountSignature as SignatureScheme>::PublicKey;
    type Signature = <C::AccountSignature as SignatureScheme>::Signature;
    type ValueBalance = AleoAmount;

    /// Transaction id = Hash of (serial numbers || commitments || memo)
    fn transaction_id(&self) -> Result<[u8; 32], TransactionError> {
        let mut pre_image_bytes: Vec<u8> = vec![];

        for serial_number in self.old_serial_numbers() {
            pre_image_bytes.extend(&to_bytes_le![serial_number]?);
        }

        for commitment in self.new_commitments() {
            pre_image_bytes.extend(&to_bytes_le![commitment]?);
        }

        pre_image_bytes.extend(self.memorandum());

        let mut h = b2s::new();
        h.update(&pre_image_bytes);

        let mut result = [0u8; 32];
        result.copy_from_slice(&h.finalize());
        Ok(result)
    }

    fn network_id(&self) -> u8 {
        self.network.id()
    }

    fn ledger_digest(&self) -> &Self::Digest {
        &self.ledger_digest
    }

    fn inner_circuit_id(&self) -> &Self::InnerCircuitID {
        &self.inner_circuit_id
    }

    fn old_serial_numbers(&self) -> &[Self::SerialNumber] {
        self.old_serial_numbers.as_slice()
    }

    fn new_commitments(&self) -> &[Self::Commitment] {
        self.new_commitments.as_slice()
    }

    fn memorandum(&self) -> &Self::Memorandum {
        &self.memorandum
    }

    fn program_commitment(&self) -> &Self::ProgramCommitment {
        &self.program_commitment
    }

    fn local_data_root(&self) -> &Self::LocalDataRoot {
        &self.local_data_root
    }

    fn value_balance(&self) -> Self::ValueBalance {
        self.value_balance
    }

    fn signatures(&self) -> &[Self::Signature] {
        &self.signatures
    }

    fn encrypted_records(&self) -> &[Self::EncryptedRecord] {
        &self.encrypted_records
    }

    fn size(&self) -> usize {
        let transaction_bytes = to_bytes_le![self].unwrap();
        transaction_bytes.len()
    }
}

impl<C: Testnet1Components> ToBytes for Transaction<C> {
    #[inline]
    fn write_le<W: Write>(&self, mut writer: W) -> IoResult<()> {
        for old_serial_number in &self.old_serial_numbers {
            CanonicalSerialize::serialize(old_serial_number, &mut writer).unwrap();
        }

        for new_commitment in &self.new_commitments {
            new_commitment.write_le(&mut writer)?;
        }

        self.memorandum.write_le(&mut writer)?;

        self.ledger_digest.write_le(&mut writer)?;
        self.inner_circuit_id.write_le(&mut writer)?;
        self.transaction_proof.write_le(&mut writer)?;
        self.program_commitment.write_le(&mut writer)?;
        self.local_data_root.write_le(&mut writer)?;

        self.value_balance.write_le(&mut writer)?;
        self.network.write_le(&mut writer)?;

        for signature in &self.signatures {
            signature.write_le(&mut writer)?;
        }

        for encrypted_record in &self.encrypted_records {
            encrypted_record.write_le(&mut writer)?;
        }

        Ok(())
    }
}

impl<C: Testnet1Components> FromBytes for Transaction<C> {
    #[inline]
    fn read_le<R: Read>(mut reader: R) -> IoResult<Self> {
        // Read the old serial numbers
        let num_old_serial_numbers = C::NUM_INPUT_RECORDS;
        let mut old_serial_numbers = Vec::with_capacity(num_old_serial_numbers);
        for _ in 0..num_old_serial_numbers {
            let old_serial_number: <C::AccountSignature as SignatureScheme>::PublicKey =
                CanonicalDeserialize::deserialize(&mut reader).unwrap();

            old_serial_numbers.push(old_serial_number);
        }

        // Read the new commitments
        let num_new_commitments = C::NUM_OUTPUT_RECORDS;
        let mut new_commitments = Vec::with_capacity(num_new_commitments);
        for _ in 0..num_new_commitments {
            let new_commitment: <C::RecordCommitment as CommitmentScheme>::Output = FromBytes::read_le(&mut reader)?;
            new_commitments.push(new_commitment);
        }

        let memorandum: [u8; 32] = FromBytes::read_le(&mut reader)?;

        let ledger_digest: MerkleTreeDigest<C::LedgerMerkleTreeParameters> = FromBytes::read_le(&mut reader)?;
        let inner_circuit_id: <C::InnerCircuitIDCRH as CRH>::Output = FromBytes::read_le(&mut reader)?;
        let transaction_proof: <C::OuterSNARK as SNARK>::Proof = FromBytes::read_le(&mut reader)?;
        let program_commitment: <C::ProgramIDCommitment as CommitmentScheme>::Output = FromBytes::read_le(&mut reader)?;
        let local_data_root: <C::LocalDataCRH as CRH>::Output = FromBytes::read_le(&mut reader)?;

        let value_balance: AleoAmount = FromBytes::read_le(&mut reader)?;
        let network: Network = FromBytes::read_le(&mut reader)?;

        // Read the signatures
        let num_signatures = C::NUM_INPUT_RECORDS;
        let mut signatures = Vec::with_capacity(num_signatures);
        for _ in 0..num_signatures {
            let signature: <C::AccountSignature as SignatureScheme>::Signature = FromBytes::read_le(&mut reader)?;
            signatures.push(signature);
        }

        // Read the encrypted records
        let num_encrypted_records = C::NUM_OUTPUT_RECORDS;
        let mut encrypted_records = Vec::with_capacity(num_encrypted_records);
        for _ in 0..num_encrypted_records {
            let encrypted_record: EncryptedRecord<C> = FromBytes::read_le(&mut reader)?;

            encrypted_records.push(encrypted_record);
        }

        Ok(Self {
            network,
            ledger_digest,
            old_serial_numbers,
            new_commitments,
            program_commitment,
            local_data_root,
            value_balance,
            signatures,
            encrypted_records,
            inner_circuit_id,
            transaction_proof,
            memorandum,
        })
    }
}

// TODO add debug support for record ciphertexts
impl<C: Testnet1Components> fmt::Debug for Transaction<C> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Transaction {{ network_id: {:?}, digest: {:?}, inner_circuit_id: {:?}, old_serial_numbers: {:?}, new_commitments: {:?}, program_commitment: {:?}, local_data_root: {:?}, value_balance: {:?}, signatures: {:?}, transaction_proof: {:?}, memorandum: {:?} }}",
            self.network,
            self.ledger_digest,
            self.inner_circuit_id,
            self.old_serial_numbers,
            self.new_commitments,
            self.program_commitment,
            self.local_data_root,
            self.value_balance,
            self.signatures,
            self.transaction_proof,
            self.memorandum,
        )
    }
}