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
use crate::basic::{AnyResult, BufferedOutputPipe, Closable};

// -----------------------------------------------

pub struct BitEncoder<const SIZE: usize> {
	low: u32,
	high: u32,
	output: BufferedOutputPipe<u8, SIZE>,
}

impl<const SIZE: usize> BitEncoder<SIZE> {
	pub fn new(output: BufferedOutputPipe<u8, SIZE>) -> Self {
		Self {
			low: 0,
			high: 0xFFFFFFFF,
			output,
		}
	}

	#[cold]
	#[inline(always)]
	fn flush(&mut self) -> AnyResult<()> {
		debug_assert!((self.high ^ self.low) < 0x01000000);
		while {
			// write byte
			self.output.output((self.low >> 24) as u8)?;
			// shift new bits into high/low
			self.low <<= 8;
			self.high = (self.high << 8) | 0xFF;
			// check condition again
			(self.high ^ self.low) < 0x01000000
		} {}
		Ok(())
	}

	#[inline(always)]
	pub fn bit(&mut self, prediction: u32, bit: Bit) -> AnyResult<()> {
		// checking
		debug_assert!(self.low < self.high);
		// get delta
		let delta: u32 = (((self.high - self.low) as u64 * prediction as u64) >> 32) as u32;
		// calculate middle
		let middle: u32 = self.low + delta;
		debug_assert!(self.low <= middle && middle < self.high);
		// set new range limit
		*(match bit {
			Bit::Zero => &mut self.low,
			Bit::One => &mut self.high,
		}) = middle + (u32::from(bit) ^ 1);
		// shift bits out
		if (self.high ^ self.low) < 0x01000000 {
			self.flush()?;
		}
		// oke
		Ok(())
	}
}

impl<const SIZE: usize> Closable<()> for BitEncoder<SIZE> {
	fn close(mut self) -> AnyResult<()> {
		// write last byte and close
		self.output.output((self.low >> 24) as u8)?;
		self.output.close()
	}
}
