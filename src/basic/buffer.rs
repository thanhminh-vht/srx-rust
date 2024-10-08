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

use std::ops::{Deref, DerefMut};

// -----------------------------------------------

// A constant size array in a Box
#[derive(Clone)]
pub struct Buffer<T: Copy, const SIZE: usize>(Box<[T]>);

impl<T: Copy + Default, const SIZE: usize> Buffer<T, SIZE> {
	pub fn new() -> Self {
		Self(vec![Default::default(); SIZE].into_boxed_slice())
	}
}

impl<T: Copy, const SIZE: usize> Deref for Buffer<T, SIZE> {
	type Target = [T];

	fn deref(&self) -> &Self::Target {
		self.0.deref()
	}
}

impl<T: Copy + Send + 'static, const SIZE: usize> DerefMut for Buffer<T, SIZE> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.0.deref_mut()
	}
}
