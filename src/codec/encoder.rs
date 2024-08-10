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
use crate::secondary_context::{BitEncoder, StateInfo};
use std::io::{Read, Write};
use std::thread::{scope, ScopedJoinHandle};

// -----------------------------------------------

// Message is an encoding request from primary context to secondary context
#[derive(Copy, Clone)]
enum Message {
	Bit(usize, Bit),   // encoding a bit at context
	Byte(usize, Byte), // encoding a byte at context
}

// PackedMessage is a packed version of Message into an u32, suitable to transfer between threads
#[derive(Copy, Clone, Default)]
struct PackedMessage(u32);

impl PackedMessage {
	fn bit(context: usize, bit: Bit) -> Self {
		Self(u32::from(bit) << 30 | context as u32)
	}

	fn byte(context: usize, byte: Byte) -> Self {
		Self(0x80000000 | context as u32 | u32::from(byte))
	}

	fn get(&self) -> Message {
		if self.0 < 0x80000000 {
			Message::Bit((self.0 & 0x3FFFFFFF) as usize, Bit::from(self.0 >> 30))
		} else {
			Message::Byte((self.0 & 0x7FFFFF00) as usize, Byte::from(self.0 & 0xFF))
		}
	}
}

// -----------------------------------------------

fn run_primary_context_encoder<const IO_BUFFER_SIZE: usize, const MESSAGE_BUFFER_SIZE: usize>(
	mut input: BufferedInputPipe<u8, IO_BUFFER_SIZE>,
	mut output: BufferedOutputPipe<PackedMessage, MESSAGE_BUFFER_SIZE>,
) -> AnyResult<()> {
	let mut context: BridgedPrimaryContext = BridgedPrimaryContext::new();
	loop {
		let info: BridgedContextInfo = BridgedContextInfo::new(context.get_info());
		match input.produce()? {
			None => {
				output.output(PackedMessage::bit(info.first_context(), Bit::One))?;
				output.output(PackedMessage::bit(info.second_context(), Bit::Zero))?;
				output.output(PackedMessage::byte(
					info.literal_context(),
					info.first_byte(),
				))?;
				input.close()?;
				output.close()?;
				return Ok(());
			}
			Some(current_byte) => match context.matching(Byte::from(current_byte)) {
				ByteMatched::MatchFirst => {
					output.output(PackedMessage::bit(info.first_context(), Bit::Zero))?;
				}
				ByteMatched::NoMatch => {
					output.output(PackedMessage::bit(info.first_context(), Bit::One))?;
					output.output(PackedMessage::bit(info.second_context(), Bit::Zero))?;
					output.output(PackedMessage::byte(
						info.literal_context(),
						Byte::from(current_byte),
					))?;
				}
				ByteMatched::MatchSecond => {
					output.output(PackedMessage::bit(info.first_context(), Bit::One))?;
					output.output(PackedMessage::bit(info.second_context(), Bit::One))?;
					output.output(PackedMessage::bit(info.third_context(), Bit::Zero))?;
				}
				ByteMatched::MatchThird => {
					output.output(PackedMessage::bit(info.first_context(), Bit::One))?;
					output.output(PackedMessage::bit(info.second_context(), Bit::One))?;
					output.output(PackedMessage::bit(info.third_context(), Bit::One))?;
				}
			},
		}
	}
}

// -----------------------------------------------

struct SecondaryContextEncoder<const IO_BUFFER_SIZE: usize, const MESSAGE_BUFFER_SIZE: usize> {
	context: BridgedSecondaryContext,
	input: BufferedInputPipe<PackedMessage, MESSAGE_BUFFER_SIZE>,
	encoder: BitEncoder<IO_BUFFER_SIZE>,
}

impl<const IO_BUFFER_SIZE: usize, const MESSAGE_BUFFER_SIZE: usize>
	SecondaryContextEncoder<IO_BUFFER_SIZE, MESSAGE_BUFFER_SIZE>
{
	#[inline(always)]
	fn bit(&mut self, context_index: usize, bit: Bit) -> AnyResult<()> {
		let current_state: StateInfo = self.context.get_info(context_index);
		self.context.update(current_state, context_index, bit);
		self.encoder.bit(current_state.prediction(), bit)
	}

	fn byte(&mut self, context_index: usize, byte: Byte) -> AnyResult<()> {
		// code high 4 bits in first 15 contexts
		let high: usize = (usize::from(byte) >> 4) | 16;
		self.bit(context_index + 1, Bit::from(high >> 3 & 1))?;
		self.bit(context_index + (high >> 3), Bit::from(high >> 2 & 1))?;
		self.bit(context_index + (high >> 2), Bit::from(high >> 1 & 1))?;
		self.bit(context_index + (high >> 1), Bit::from(high & 1))?;
		// code low 4 bits in one of 16 blocks of 15 contexts (to reduce cache misses)
		let low_context: usize = context_index + 15 * (high - 15);
		let low: usize = (usize::from(byte) & 15) | 16;
		self.bit(low_context + 1, Bit::from(low >> 3 & 1))?;
		self.bit(low_context + (low >> 3), Bit::from(low >> 2 & 1))?;
		self.bit(low_context + (low >> 2), Bit::from(low >> 1 & 1))?;
		self.bit(low_context + (low >> 1), Bit::from(low & 1))?;
		// oke
		Ok(())
	}

	fn encode(mut self) -> AnyResult<()> {
		loop {
			match self.input.produce()? {
				None => {
					self.input.close()?;
					self.encoder.close()?;
					return Ok(());
				}
				Some(message) => match message.get() {
					Message::Bit(context_index, bit) => self.bit(context_index, bit)?,
					Message::Byte(context_index, value) => self.byte(context_index, value)?,
				},
			}
		}
	}
}

// -----------------------------------------------

fn run_secondary_context_encoder<const IO_BUFFER_SIZE: usize, const MESSAGE_BUFFER_SIZE: usize>(
	input: BufferedInputPipe<PackedMessage, MESSAGE_BUFFER_SIZE>,
	output: BufferedOutputPipe<u8, IO_BUFFER_SIZE>,
) -> AnyResult<()> {
	let encoder: SecondaryContextEncoder<IO_BUFFER_SIZE, MESSAGE_BUFFER_SIZE> =
		SecondaryContextEncoder {
			context: BridgedSecondaryContext::new(),
			input,
			encoder: BitEncoder::new(output),
		};
	encoder.encode()
}

// -----------------------------------------------

pub fn encode<
	R: Read + Send,
	W: Write + Send,
	const IO_BUFFER_SIZE: usize,
	const MESSAGE_BUFFER_SIZE: usize,
>(
	reader: R,
	writer: W,
) -> AnyResult<(R, W)> {
	scope(|scope| {
		// create pipe between file reader thread and primary context thread
		let (reader_output_pipe, reader_input_pipe): (
			BufferedOutputPipe<u8, IO_BUFFER_SIZE>,
			BufferedInputPipe<u8, IO_BUFFER_SIZE>,
		) = pipe::<u8, IO_BUFFER_SIZE>();

		// create pipe between primary context thread and secondary context thread
		let (message_writer, message_reader): (
			BufferedOutputPipe<PackedMessage, MESSAGE_BUFFER_SIZE>,
			BufferedInputPipe<PackedMessage, MESSAGE_BUFFER_SIZE>,
		) = pipe::<PackedMessage, MESSAGE_BUFFER_SIZE>();

		// create pipe between secondary context thread and file writer thread
		let (writer_output_pipe, writer_input_pipe): (
			BufferedOutputPipe<u8, IO_BUFFER_SIZE>,
			BufferedInputPipe<u8, IO_BUFFER_SIZE>,
		) = pipe::<u8, IO_BUFFER_SIZE>();

		// create file reader thread
		let file_reader: ScopedJoinHandle<AnyResult<R>> =
			scope.spawn(|| run_file_reader(reader, reader_output_pipe));

		// create primary context thread
		let primary_context_encoder: ScopedJoinHandle<AnyResult<()>> =
			scope.spawn(|| run_primary_context_encoder(reader_input_pipe, message_writer));

		// create secondary context thread
		let secondary_context_encoder: ScopedJoinHandle<AnyResult<()>> =
			scope.spawn(|| run_secondary_context_encoder(message_reader, writer_output_pipe));

		// create file writer thread
		let file_writer: ScopedJoinHandle<AnyResult<W>> =
			scope.spawn(|| run_file_writer(writer_input_pipe, writer));

		// join all thread
		let returned_reader: R = thread_join(file_reader)?;
		thread_join(primary_context_encoder)?;
		thread_join(secondary_context_encoder)?;
		let returned_writer: W = thread_join(file_writer)?;

		// give back the file handlers
		Ok((returned_reader, returned_writer))
	})
}
