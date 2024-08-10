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

use crate::basic::{AnyError, AnyResult, BufferedInputPipe, BufferedOutputPipe, Closable};
use std::io::{Read, Write};
use std::thread::ScopedJoinHandle;

// -----------------------------------------------

pub fn run_file_reader<R: Read, const IO_BUFFER_SIZE: usize>(
	mut reader: R,
	mut output: BufferedOutputPipe<u8, IO_BUFFER_SIZE>,
) -> AnyResult<R> {
	while output.receive_from(&mut reader)? > 0 {}
	output.close()?;
	Ok(reader)
}

// -----------------------------------------------

pub fn run_file_writer<W: Write, const IO_BUFFER_SIZE: usize>(
	mut input: BufferedInputPipe<u8, IO_BUFFER_SIZE>,
	mut writer: W,
) -> AnyResult<W> {
	while input.transfer_to(&mut writer)? > 0 {}
	input.close()?;
	Ok(writer)
}

// -----------------------------------------------

pub fn thread_join<T>(thread_handle: ScopedJoinHandle<AnyResult<T>>) -> AnyResult<T> {
	match thread_handle.join() {
		Ok(value) => Ok(value?),
		Err(error) => Err(AnyError::from_box(error)),
	}
}
