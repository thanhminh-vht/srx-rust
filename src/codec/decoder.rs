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

use super::bridged::{BridgedContextInfo, BridgedPrimaryContext, BridgedSecondaryContext};
use super::shared::{run_file_reader, run_file_writer, thread_join};
use crate::basic::{pipe, AnyResult, Bit, BufferedInputPipe, BufferedOutputPipe, Byte, Closable};
use crate::primary_context::ByteMatched;
use crate::secondary_context::{BitDecoder, StateInfo};
use std::io::{Read, Write};
use std::thread::{scope, ScopedJoinHandle};

// -----------------------------------------------

// currently there is no way to split the decoder into two separate passes like the encoder
struct CombinedContextDecoder<const IO_BUFFER_SIZE: usize> {
	primary_context: BridgedPrimaryContext,
	secondary_context: BridgedSecondaryContext,
	decoder: BitDecoder<IO_BUFFER_SIZE>,
	output: BufferedOutputPipe<u8, IO_BUFFER_SIZE>,
}

impl<const IO_BUFFER_SIZE: usize> CombinedContextDecoder<IO_BUFFER_SIZE> {
	#[inline(always)]
	fn bit(&mut self, context_index: usize) -> AnyResult<Bit> {
		let current_state: StateInfo = self.secondary_context.get_info(context_index);
		let bit: Bit = self.decoder.bit(current_state.prediction())?;
		self.secondary_context
			.update(current_state, context_index, bit);
		Ok(bit)
	}

	fn byte(&mut self, context_index: usize) -> AnyResult<Byte> {
		let mut high: usize = 1;
		high += high + usize::from(self.bit(context_index + high)?);
		high += high + usize::from(self.bit(context_index + high)?);
		high += high + usize::from(self.bit(context_index + high)?);
		high += high + usize::from(self.bit(context_index + high)?);
		let low_context: usize = context_index + 15 * (high - 15);
		let mut low: usize = 1;
		low += low + usize::from(self.bit(low_context + low)?);
		low += low + usize::from(self.bit(low_context + low)?);
		low += low + usize::from(self.bit(low_context + low)?);
		low += low + usize::from(self.bit(low_context + low)?);
		Ok(Byte::from(((high - 16) << 4) | (low - 16)))
	}

	fn decode(mut self) -> AnyResult<()> {
		loop {
			let info: BridgedContextInfo = BridgedContextInfo::new(self.primary_context.get_info());
			let (next_byte, matched): (Byte, ByteMatched) = match self.bit(info.first_context())? {
				// match first
				Bit::Zero => (info.first_byte(), ByteMatched::MatchFirst),
				// match next
				Bit::One => match self.bit(info.second_context())? {
					// literal
					Bit::Zero => {
						let next_byte: Byte = self.byte(info.literal_context())?;
						if next_byte == info.first_byte() {
							// eof
							self.decoder.close()?;
							self.output.close()?;
							return Ok(());
						}
						(next_byte, ByteMatched::NoMatch)
					}
					// match next
					Bit::One => match self.bit(info.third_context())? {
						// match second
						Bit::Zero => (info.second_byte(), ByteMatched::MatchSecond),
						// match third
						Bit::One => (info.third_byte(), ByteMatched::MatchThird),
					},
				},
			};
			self.output.output(next_byte.into())?;
			self.primary_context.matched(next_byte, matched);
		}
	}
}

// -----------------------------------------------

fn run_combined_context_decoder<const IO_BUFFER_SIZE: usize>(
	input: BufferedInputPipe<u8, IO_BUFFER_SIZE>,
	output: BufferedOutputPipe<u8, IO_BUFFER_SIZE>,
) -> AnyResult<()> {
	let decoder: CombinedContextDecoder<IO_BUFFER_SIZE> = CombinedContextDecoder {
		primary_context: BridgedPrimaryContext::new(),
		secondary_context: BridgedSecondaryContext::new(),
		decoder: BitDecoder::new(input),
		output,
	};
	decoder.decode()
}

// -----------------------------------------------

pub fn decode<R: Read + Send, W: Write + Send, const IO_BUFFER_SIZE: usize>(
	reader: R,
	writer: W,
) -> AnyResult<(R, W)> {
	scope(|scope| {
		// create pipe between file reader thread and decoder thread
		let (reader_output_pipe, reader_input_pipe): (
			BufferedOutputPipe<u8, IO_BUFFER_SIZE>,
			BufferedInputPipe<u8, IO_BUFFER_SIZE>,
		) = pipe::<u8, IO_BUFFER_SIZE>();

		// create pipe between decoder thread and file writer thread
		let (writer_output_pipe, writer_input_pipe): (
			BufferedOutputPipe<u8, IO_BUFFER_SIZE>,
			BufferedInputPipe<u8, IO_BUFFER_SIZE>,
		) = pipe::<u8, IO_BUFFER_SIZE>();

		// create file reader thread
		let file_reader: ScopedJoinHandle<AnyResult<R>> =
			scope.spawn(|| run_file_reader(reader, reader_output_pipe));

		// create decoder thread
		let combined_context_decoder: ScopedJoinHandle<AnyResult<()>> =
			scope.spawn(|| run_combined_context_decoder(reader_input_pipe, writer_output_pipe));

		// create file writer thread
		let file_writer: ScopedJoinHandle<AnyResult<W>> =
			scope.spawn(|| run_file_writer(writer_input_pipe, writer));

		// join all thread
		let returned_reader: R = thread_join(file_reader)?;
		thread_join(combined_context_decoder)?;
		let returned_writer: W = thread_join(file_writer)?;

		// give back the file handlers
		Ok((returned_reader, returned_writer))
	})
}
