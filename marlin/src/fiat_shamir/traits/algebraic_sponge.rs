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

use snarkvm_fields::PrimeField;
use snarkvm_gadgets::fields::FpGadget;
use snarkvm_r1cs::{ConstraintSystem, SynthesisError};

/// Trait for an algebraic sponge.
pub trait AlgebraicSponge<CF: PrimeField>: Clone {
    /// Initializes an algebraic sponge.
    fn new() -> Self;
    /// Takes in field elements.
    fn absorb(&mut self, elems: &[CF]);
    /// Takes out field elements.
    fn squeeze(&mut self, num: usize) -> Vec<CF>;
}

/// Trait for an algebraic sponge such as Poseidon.
pub trait AlgebraicSpongeVar<CF: PrimeField, PS: AlgebraicSponge<CF>>: Clone {
    /// Create the new sponge.
    fn new<CS: ConstraintSystem<CF>>(cs: CS) -> Self;

    /// Instantiate from a plaintext sponge.
    fn constant<CS: ConstraintSystem<CF>>(cs: CS, ps: &PS) -> Self;

    /// Take in field elements.
    fn absorb<CS: ConstraintSystem<CF>>(&mut self, cs: CS, elems: &[FpGadget<CF>]) -> Result<(), SynthesisError>;

    /// Output field elements.
    fn squeeze<CS: ConstraintSystem<CF>>(&mut self, cs: CS, num: usize) -> Result<Vec<FpGadget<CF>>, SynthesisError>;
}