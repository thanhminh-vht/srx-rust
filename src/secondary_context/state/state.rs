/*
 * srx: The fast Symbol Ranking based compressor.
 * Copyright (C) 2023-2024  Mai Thanh Minh (a.k.a. thanhminhmr)
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either  version 3 of the  License,  or (at your option) any later
 * version.
 *
 * This program  is distributed in the hope  that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR  A PARTICULAR PURPOSE. See  the  GNU  General  Public   License  for more
 * details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see <https://www.gnu.org/licenses/>.
 *
 */
use crate::basic::Bit;

// -----------------------------------------------

// Split the giant table to another non-rs file to keep IDE happy
pub const STATE_TABLE: &[StateInfo] = include!("state_table.inc");

// -----------------------------------------------

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct StateInfo(u64);

impl StateInfo {
	pub const fn new(prediction: u32, next_if_zero: u16, next_if_one: u16) -> Self {
		Self(((prediction as u64) << 32) | ((next_if_zero as u64) << 16) | (next_if_one as u64))
	}

	pub fn next(&self, bit: Bit) -> u16 {
		(if bit.into() { self.0 } else { self.0 >> 16 }) as u16
	}

	pub fn prediction(&self) -> u32 {
		(self.0 >> 32) as u32
	}
}

// -----------------------------------------------

#[derive(Copy, Clone, Default)]
pub struct BitState(u16);

impl BitState {
	pub fn get_info(&self) -> StateInfo {
		STATE_TABLE[self.0 as usize]
	}

	pub fn update(&mut self, current_state: StateInfo, bit: Bit) {
		debug_assert!(STATE_TABLE[self.0 as usize] == current_state);
		self.0 = current_state.next(bit);
	}
}
