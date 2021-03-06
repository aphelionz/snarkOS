// Copyright (C) 2019-2021 Aleo Systems Inc.
// This file is part of the snarkOS library.

// The snarkOS library is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The snarkOS library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with the snarkOS library. If not, see <https://www.gnu.org/licenses/>.

#[derive(Debug, Error)]
pub enum StreamReadError {
    #[error("IO error: {}", _0)]
    Io(std::io::Error),

    #[error("{}", _0)]
    Message(String),
}

impl From<std::io::Error> for StreamReadError {
    fn from(error: std::io::Error) -> Self {
        StreamReadError::Io(error)
    }
}
