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

use std::any::Any;
use std::error::Error;
use std::fmt;
use std::fmt::{Debug, Display, Formatter};

// -----------------------------------------------

// A convenient kind of result that can contain any type of error
pub type AnyResult<T> = Result<T, AnyError>;

// -----------------------------------------------

// A convenient kind of error that can wrap anything, including other error
#[derive(Debug)]
pub enum AnyError {
	String(String),
	Error(Box<dyn Error + Send>),
	Box(Box<dyn Any + Send>),
}

impl AnyError {
	// A convenient function to create an error from string
	#[cold]
	#[inline(always)]
	pub fn from_string<S: Into<String>>(into_string: S) -> Self {
		Self::String(into_string.into())
	}

	// A convenient function to create an error from a Box
	#[cold]
	#[inline(always)]
	pub fn from_box(any: Box<dyn Any + Send>) -> Self {
		match any.downcast_ref::<String>() {
			Some(string) => Self::from_string(string),
			None => match any.downcast_ref::<&'static str>() {
				Some(&string) => Self::from_string(string),
				None => Self::Box(any),
			},
		}
	}
}

impl Display for AnyError {
	// A convenient function to print out the error
	fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
		match self {
			AnyError::String(value) => Display::fmt(value, formatter),
			AnyError::Error(value) => Display::fmt(value, formatter),
			AnyError::Box(value) => Debug::fmt(value, formatter),
		}
	}
}

impl<E: Error + Send + 'static> From<E> for AnyError {
	// A convenient function to create an error from anything
	#[cold]
	#[inline(always)]
	fn from(e: E) -> Self {
		Self::Error(Box::new(e))
	}
}
