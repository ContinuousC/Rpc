/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

pub trait VarInt {
    fn from_varint(b: &[u8]) -> Self;
    fn to_varint(&self) -> Vec<u8>;
}

impl VarInt for u64 {
    fn from_varint(b: &[u8]) -> Self {
        match b[0] {
            0..=240 => b[0] as u64,
            241..=248 => 240 + (((b[0] - 241) as u64) << 8 | b[1] as u64),
            249 => 2288 + (((b[1] as u64) << 8) | b[2] as u64),
            250..=255 => b[1..(b[0] as usize - 246)]
                .iter()
                .fold(0, |n, i| n << 8 | (*i as u64)),
        }
    }
    fn to_varint(&self) -> Vec<u8> {
        match self {
            0..=240 => vec![*self as u8],
            241..=2287 => {
                let n0 = *self - 240;
                vec![((n0 >> 8) + 241) as u8, (n0 & 0xff) as u8]
            }
            2288..=67823 => {
                let n0 = *self - 2288;
                vec![249, (n0 >> 8) as u8, (n0 & 0xff) as u8]
            }
            67824.. => {
                let bs = self.to_be_bytes();
                let i = bs.iter().take_while(|b| **b == 0).count();
                let b = &bs[i..];
                std::iter::once((b.len() + 247) as u8)
                    .chain(b.iter().copied())
                    .collect()
            }
        }
    }
}

pub fn varint_len(b0: u8) -> usize {
    match b0 {
        0..=240 => 1,
        241..=248 => 2,
        249..=255 => b0 as usize - 246,
    }
}
