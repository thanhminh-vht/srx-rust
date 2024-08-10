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
use crate::primary_context::ByteMatched;

// -----------------------------------------------

// Split the giant table to another non-rs file to keep IDE happy
pub const STATE_TABLE: &[HistoryState] = include!("state_table.inc");

// -----------------------------------------------

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct HistoryState(u64);

impl HistoryState {
	pub const fn new(
		first_count: u8,
		next_if_first: u8,
		next_if_second: u8,
		next_if_third: u8,
		next_if_miss: u8,
	) -> Self {
		Self(
			(next_if_first as u64)
				| ((next_if_second as u64) << 8)
				| ((next_if_third as u64) << 16)
				| ((next_if_miss as u64) << 24)
				| ((first_count as u64) << 32),
		)
	}

	fn next(&self, matched: ByteMatched) -> usize {
		match matched {
			ByteMatched::MatchFirst => (self.0 & 0xFF) as usize,
			ByteMatched::MatchSecond => ((self.0 >> 8) & 0xFF) as usize,
			ByteMatched::MatchThird => ((self.0 >> 16) & 0xFF) as usize,
			ByteMatched::NoMatch => ((self.0 >> 24) & 0xFF) as usize,
		}
	}

	fn match_count(&self) -> usize {
		(self.0 >> 32) as usize
	}
}

// -----------------------------------------------

#[derive(Clone, Copy, Default)]
pub(crate) struct ByteHistory(u32);

impl ByteHistory {
	pub fn first_byte(self) -> Byte {
		Byte::from((self.0 >> 8) & 0xFF)
	}

	pub fn second_byte(self) -> Byte {
		Byte::from((self.0 >> 16) & 0xFF)
	}

	pub fn third_byte(self) -> Byte {
		Byte::from(self.0 >> 24)
	}

	fn state(self) -> HistoryState {
		STATE_TABLE[(self.0 & 0xFF) as usize]
	}

	pub fn match_count(self) -> usize {
		self.state().match_count()
	}

	pub fn matching(&mut self, next_byte: Byte) -> ByteMatched {
		let byte_history: u32 = self.0;
		let mask: u32 = byte_history ^ (0x01_01_01_00 * u32::from(next_byte));
		let (matched, updated_history): (ByteMatched, u32) = if (mask & 0x00_00_FF_00) == 0 {
			(
				ByteMatched::MatchFirst,
				// matched the first byte, keep the order of bytes
				byte_history & 0xFF_FF_FF_00,
			)
		} else if (mask & 0x00_FF_00_00) == 0 {
			(
				ByteMatched::MatchSecond,
				// matched the second byte, swap the first and the second place
				(byte_history & 0xFF_00_00_00)
					| (((byte_history & 0x00_00_FF_00) | u32::from(next_byte)) << 8),
			)
		} else if (mask & 0xFF_00_00_00) == 0 {
			(
				ByteMatched::MatchThird,
				// matched the third byte, move old first/second to second/third and set the first byte
				((byte_history & 0x00_FF_FF_00) | u32::from(next_byte)) << 8,
			)
		} else {
			(
				ByteMatched::NoMatch,
				// not match, move old first/second to second/third and set the first byte
				((byte_history & 0x00_FF_FF_00) | u32::from(next_byte)) << 8,
			)
		};
		self.0 = updated_history | self.state().next(matched) as u32;
		matched
	}

	pub fn matched(&mut self, next_byte: Byte, matched: ByteMatched) {
		let byte_history: u32 = self.0;
		let updated_history: u32 = match matched {
			ByteMatched::MatchFirst => {
				// matched the first byte, keep the order of bytes
				byte_history & 0xFF_FF_FF_00
			}
			ByteMatched::MatchSecond => {
				(byte_history & 0xFF_00_00_00)
					| (((byte_history & 0x00_00_FF_00) | u32::from(next_byte)) << 8)
			}
			ByteMatched::MatchThird => {
				// matched the third byte, move old first/second to second/third and set the first byte
				((byte_history & 0x00_FF_FF_00) | u32::from(next_byte)) << 8
			}
			ByteMatched::NoMatch => {
				// not match, move old first/second to second/third and set the first byte
				((byte_history & 0x00_FF_FF_00) | u32::from(next_byte)) << 8
			}
		};
		self.0 = updated_history | self.state().next(matched) as u32;
	}
}
