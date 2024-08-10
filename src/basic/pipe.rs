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
use super::buffer::Buffer;
use super::error::{AnyError, AnyResult};
use super::io::Closable;
use std::io::{Read, Write};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

// -----------------------------------------------

// The Pipe: PipedBufferedOutput --> PipedBufferedInput

// a buffer with data that the output side send to the input side over the channel
type ConsumerToProducer<T, const SIZE: usize> = (Buffer<T, SIZE>, usize);

// an empty buffer that the input side send back to the output side over the channel
type ProducerToConsumer<T, const SIZE: usize> = Buffer<T, SIZE>;

// -----------------------------------------------

// create a buffered pipe that can send things over thread border
pub fn pipe<T: Default + Copy + Send + 'static, const SIZE: usize>(
) -> (BufferedOutputPipe<T, SIZE>, BufferedInputPipe<T, SIZE>) {
	// create 2 sync channel to send and receive buffer
	let (output_sender, input_receiver): (
		SyncSender<ConsumerToProducer<T, SIZE>>,
		Receiver<ConsumerToProducer<T, SIZE>>,
	) = sync_channel(1);
	let (input_sender, output_receiver): (
		SyncSender<ProducerToConsumer<T, SIZE>>,
		Receiver<ProducerToConsumer<T, SIZE>>,
	) = sync_channel(1);
	// create two side of the pipe
	(
		BufferedOutputPipe {
			sender: output_sender,
			receiver: output_receiver,
			buffer: Some(Buffer::new()),
			index: 0,
		},
		BufferedInputPipe {
			sender: input_sender,
			receiver: input_receiver,
			buffer: Some(Buffer::new()),
			index: 0,
			length: 0,
		},
	)
}

// -----------------------------------------------

// the output side of the pipe
pub struct BufferedOutputPipe<T: Copy + Send + 'static, const SIZE: usize> {
	sender: SyncSender<ConsumerToProducer<T, SIZE>>,
	receiver: Receiver<ProducerToConsumer<T, SIZE>>,
	buffer: Option<Buffer<T, SIZE>>,
	index: usize,
}

impl<T: Copy + Send + 'static, const SIZE: usize> BufferedOutputPipe<T, SIZE> {
	// send the buffer to the input side of the pipe
	#[cold]
	fn sync(&mut self) -> AnyResult<()> {
		debug_assert!(self.buffer.is_some());
		debug_assert!(self.index > 0 && self.index <= SIZE);
		let buffer: Buffer<T, SIZE> = self.buffer.take().unwrap();
		self.sender.send((buffer, self.index))?;
		self.buffer = Some(self.receiver.recv()?);
		self.index = 0;
		Ok(())
	}

	// consume an element and put it into the buffer, send the whole buffer if full
	pub fn output(&mut self, value: T) -> AnyResult<()> {
		match &mut self.buffer {
			None => Err(AnyError::from_string("Broken pipe!")),
			Some(buffer) => {
				debug_assert!(self.index < SIZE);
				// put the element into the buffer
				buffer[self.index] = value;
				self.index += 1;
				debug_assert!(self.index <= SIZE);
				// check if buffer is full and sync if needed
				if self.index == SIZE {
					self.sync()?;
				}
				debug_assert!(self.index < SIZE);
				Ok(())
			}
		}
	}
}

impl<const SIZE: usize> BufferedOutputPipe<u8, SIZE> {
	// receive multiple bytes from standard reader
	pub fn receive_from<R: Read>(&mut self, reader: &mut R) -> AnyResult<usize> {
		match &mut self.buffer {
			None => Err(AnyError::from_string("Broken pipe!")),
			Some(buffer) => {
				debug_assert!(self.index < SIZE);
				// slice the remaining buffer and read multiple bytes
				let sliced_buffer: &mut [u8] = &mut buffer[self.index..SIZE];
				let produced_length: usize = reader.read(sliced_buffer)?;
				debug_assert!(produced_length <= sliced_buffer.len());
				self.index += produced_length;
				debug_assert!(self.index <= SIZE);
				// check if buffer is full and sync if needed
				if self.index == SIZE {
					self.sync()?;
				}
				Ok(produced_length)
			}
		}
	}
}

impl<T: Copy + Send + 'static, const SIZE: usize> Closable<()> for BufferedOutputPipe<T, SIZE> {
	// send the remaining data in buffer and close the pipe
	fn close(mut self) -> AnyResult<()> {
		if self.buffer.is_some() && self.index > 0 {
			debug_assert!(self.index <= SIZE);
			self.sync()
		} else {
			Ok(())
		}
	}
}

// -----------------------------------------------

// the input side of the pipe
pub struct BufferedInputPipe<T: Copy + Send + 'static, const SIZE: usize> {
	sender: SyncSender<ProducerToConsumer<T, SIZE>>,
	receiver: Receiver<ConsumerToProducer<T, SIZE>>,
	buffer: Option<Buffer<T, SIZE>>,
	length: usize,
	index: usize,
}

impl<T: Copy + Send + 'static, const SIZE: usize> BufferedInputPipe<T, SIZE> {
	// send the buffer to the output side of the pipe
	#[cold]
	fn sync(&mut self) -> AnyResult<()> {
		debug_assert!(self.buffer.is_some() && self.index == self.length && self.length <= SIZE);
		// take the old buffer and set it to None
		let old_buffer: Buffer<T, SIZE> = self.buffer.take().unwrap();
		// receive the new buffer
		if let Ok((new_buffer, length)) = self.receiver.recv() {
			debug_assert!(length > 0 && length <= SIZE);
			// set the new buffer and its length
			self.buffer = Some(new_buffer);
			self.length = length;
			self.index = 0;
			// send the old buffer away. If the output side is already closed,
			// this will error out, which can be safely discarded
			let _error_safely_discarded_ = self.sender.send(old_buffer);
		}
		Ok(())
	}

	// if able, read one element from the buffer, sync if needed
	pub fn produce(&mut self) -> AnyResult<Option<T>> {
		debug_assert!(self.index <= self.length && self.length <= SIZE);
		// sync if the buffer is empty
		if self.buffer.is_some() && self.index == self.length {
			self.sync()?;
		}
		// try to read from buffer
		match &mut self.buffer {
			// no buffer, return none
			None => Ok(None),
			// there is a buffer, return one element
			Some(buffer) => {
				debug_assert!(self.index < self.length && self.length <= SIZE);
				let value: T = buffer[self.index];
				self.index += 1;
				debug_assert!(self.index <= self.length);
				Ok(Some(value))
			}
		}
	}
}

impl<const SIZE: usize> BufferedInputPipe<u8, SIZE> {
	// transfer multiple bytes to standard writer
	pub(crate) fn transfer_to<W: Write>(&mut self, writer: &mut W) -> AnyResult<usize> {
		debug_assert!(self.index <= self.length && self.length <= SIZE);
		// sync if the buffer is empty
		if self.buffer.is_some() && self.index == self.length {
			self.sync()?;
		}
		// try to transfer the buffer
		match &mut self.buffer {
			// no buffer, return ok
			None => Ok(0),
			// there is a buffer
			Some(buffer) => {
				debug_assert!(self.index < self.length && self.length <= SIZE);
				// slice it and write from the last position
				let sliced_buffer: &[u8] = &buffer[self.index..self.length];
				let consumed_length: usize = writer.write(sliced_buffer)?;
				// check how much got consumed and update the index
				debug_assert!(consumed_length <= sliced_buffer.len());
				self.index += consumed_length;
				debug_assert!(self.index <= SIZE);
				Ok(consumed_length)
			}
		}
	}
}

impl<T: Copy + Send + 'static, const SIZE: usize> Closable<()> for BufferedInputPipe<T, SIZE> {
	fn close(self) -> AnyResult<()> {
		Ok(())
	}
}
