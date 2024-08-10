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

use crate::basic::Byte;
use crate::primary_context::{PrimaryContext, PrimaryContextInfo};
use crate::secondary_context::SecondaryContext;

// -----------------------------------------------

pub const PRIMARY_CONTEXT_SIZE: usize = 1 << 24;
pub const SECONDARY_CONTEXT_SIZE: usize = 0x4000 * 256 + (1024 + 32) * 768;

// -----------------------------------------------

pub type BridgedPrimaryContext = PrimaryContext<PRIMARY_CONTEXT_SIZE>;
pub type BridgedSecondaryContext = SecondaryContext<SECONDARY_CONTEXT_SIZE>;

// -----------------------------------------------

pub struct BridgedContextInfo {
	bit_context: usize,
	literal_context: usize,
	primary_context_info: PrimaryContextInfo,
}

impl BridgedContextInfo {
	pub fn new(primary_context_info: PrimaryContextInfo) -> Self {
		let match_count: usize = primary_context_info.match_count();
		Self {
			bit_context: 0x4000 * 256
				+ if match_count < 4 {
					(usize::from(primary_context_info.previous_byte()) << 2) | match_count
				} else {
					1024 + if match_count - 4 <= 63 {
						(match_count - 4) >> 1
					} else {
						31
					}
				} * 768,
			literal_context: (primary_context_info.hash_value() & 0x3FFF) * 256,
			primary_context_info,
		}
	}

	pub fn first_context(&self) -> usize {
		self.bit_context + usize::from(self.primary_context_info.first_byte())
	}

	pub fn second_context(&self) -> usize {
		self.bit_context
			+ 0x100 + ((usize::from(self.primary_context_info.second_byte())
			+ usize::from(self.primary_context_info.third_byte()))
			& 0xFF)
	}

	pub fn third_context(&self) -> usize {
		self.bit_context
			+ 0x200 + ((usize::from(self.primary_context_info.second_byte()) * 2)
			.wrapping_sub(usize::from(self.primary_context_info.third_byte()))
			& 0xFF)
	}

	pub fn literal_context(&self) -> usize {
		self.literal_context
	}

	pub fn first_byte(&self) -> Byte {
		self.primary_context_info.first_byte()
	}

	pub fn second_byte(&self) -> Byte {
		self.primary_context_info.second_byte()
	}

	pub fn third_byte(&self) -> Byte {
		self.primary_context_info.third_byte()
	}
}
