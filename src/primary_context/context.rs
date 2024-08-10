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

use crate::basic::{Buffer, Byte};

use super::history::ByteHistory;
use super::matched::ByteMatched;

// -----------------------------------------------

// The symbol ranking context that saves last 3 values of next byte
pub struct PrimaryContext<const SIZE: usize> {
	previous_byte: Byte,
	hash_value: usize,
	context: Buffer<ByteHistory, SIZE>,
}

impl<const SIZE: usize> PrimaryContext<SIZE> {
	// assert that SIZE is power of 2
	const _SIZE_CHECK: () = assert!(SIZE != 0 && (SIZE & (SIZE - 1)) == 0);

	pub fn new() -> Self {
		Self {
			previous_byte: Byte::from(0),
			hash_value: 0,
			context: Buffer::new(),
		}
	}

	pub fn get_info(&self) -> PrimaryContextInfo {
		let current_history: ByteHistory = self.context[self.hash_value];
		PrimaryContextInfo {
			previous_byte: self.previous_byte,
			first_byte: current_history.first_byte(),
			second_byte: current_history.second_byte(),
			third_byte: current_history.third_byte(),
			hash_value: self.hash_value,
			match_count: current_history.match_count(),
		}
	}

	fn next_hash(hash_value: usize, next_byte: Byte) -> usize {
		(hash_value * (5 << 5) + usize::from(next_byte) + 1) % SIZE
	}

	pub fn matching(&mut self, next_byte: Byte) -> ByteMatched {
		let matched: ByteMatched = self.context[self.hash_value].matching(next_byte);
		self.previous_byte = next_byte;
		self.hash_value = Self::next_hash(self.hash_value, next_byte);
		debug_assert!(self.hash_value < SIZE);
		matched
	}

	pub fn matched(&mut self, next_byte: Byte, matched: ByteMatched) {
		self.context[self.hash_value].matched(next_byte, matched);
		self.previous_byte = next_byte;
		self.hash_value = Self::next_hash(self.hash_value, next_byte);
		debug_assert!(self.hash_value < SIZE);
	}
}

// -----------------------------------------------

pub struct PrimaryContextInfo {
	previous_byte: Byte,
	first_byte: Byte,
	second_byte: Byte,
	third_byte: Byte,
	hash_value: usize,
	match_count: usize,
}

impl PrimaryContextInfo {
	pub fn previous_byte(&self) -> Byte {
		self.previous_byte
	}

	pub fn hash_value(&self) -> usize {
		self.hash_value
	}

	pub fn first_byte(&self) -> Byte {
		self.first_byte
	}

	pub fn second_byte(&self) -> Byte {
		self.second_byte
	}

	pub fn third_byte(&self) -> Byte {
		self.third_byte
	}

	pub fn match_count(&self) -> usize {
		self.match_count
	}
}
