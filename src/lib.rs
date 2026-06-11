use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::convert::TryInto;
use std::fmt; // Added for the Display trait

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct CompactSize {
    pub value: u64,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BitcoinError {
    InsufficientBytes,
    InvalidFormat,
}

impl CompactSize {
    pub fn new(value: u64) -> Self {
        Self { value }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        if self.value < 0xFD {
            vec![self.value as u8]
        } else if self.value <= 0xFFFF {
            let mut b = vec![0xFD];
            b.extend_from_slice(&(self.value as u16).to_le_bytes());
            b
        } else if self.value <= 0xFFFF_FFFF {
            let mut b = vec![0xFE];
            b.extend_from_slice(&(self.value as u32).to_le_bytes());
            b
        } else {
            let mut b = vec![0xFF];
            b.extend_from_slice(&self.value.to_le_bytes());
            b
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.is_empty() {
            return Err(BitcoinError::InsufficientBytes);
        }

        match bytes[0] {
            n @ 0x00..=0xFC => Ok((Self::new(n as u64), 1)),

            0xFD => {
                if bytes.len() < 3 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let v = u16::from_le_bytes(bytes[1..3].try_into().unwrap());
                Ok((Self::new(v as u64), 3))
            }

            0xFE => {
                if bytes.len() < 5 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let v = u32::from_le_bytes(bytes[1..5].try_into().unwrap());
                Ok((Self::new(v as u64), 5))
            }

            0xFF => {
                if bytes.len() < 9 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let v = u64::from_le_bytes(bytes[1..9].try_into().unwrap());
                Ok((Self::new(v), 9))
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Txid(pub [u8; 32]);

impl Serialize for Txid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&hex::encode(self.0))
    }
}

impl<'de> Deserialize<'de> for Txid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let bytes = hex::decode(s).map_err(serde::de::Error::custom)?;

        if bytes.len() != 32 {
            return Err(serde::de::Error::custom("Invalid txid length"));
        }

        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);

        Ok(Txid(arr))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct OutPoint {
    pub txid: Txid,
    pub vout: u32,
}

impl OutPoint {
    pub fn new(txid: [u8; 32], vout: u32) -> Self {
        Self {
            txid: Txid(txid),
            vout,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&self.txid.0);
        v.extend_from_slice(&self.vout.to_le_bytes());
        v
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 36 {
            return Err(BitcoinError::InsufficientBytes);
        }

        let mut txid = [0u8; 32];
        txid.copy_from_slice(&bytes[0..32]);

        let vout = u32::from_le_bytes(bytes[32..36].try_into().unwrap());

        Ok((Self::new(txid, vout), 36))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Script {
    pub bytes: Vec<u8>,
}

impl Script {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = CompactSize::new(self.bytes.len() as u64).to_bytes();
        v.extend_from_slice(&self.bytes);
        v
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (len, size) = CompactSize::from_bytes(bytes)?;
        let total = size + len.value as usize;

        if bytes.len() < total {
            return Err(BitcoinError::InsufficientBytes);
        }

        Ok((Self::new(bytes[size..total].to_vec()), total))
    }
}

impl Deref for Script {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    pub previous_output: OutPoint,
    pub signature_script: Script,
    pub sequence: u32,
}

impl TransactionInput {
    pub fn new(previous_output: OutPoint, signature_script: Script, sequence: u32) -> Self {
        Self {
            previous_output,
            signature_script,
            sequence,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = self.previous_output.to_bytes();
        v.extend_from_slice(&self.signature_script.to_bytes());
        v.extend_from_slice(&self.sequence.to_le_bytes());
        v
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let mut progress = 0;

        let (previous_output, size) = OutPoint::from_bytes(&bytes[progress..])?;
        progress += size;

        let (signature_script, size) = Script::from_bytes(&bytes[progress..])?;
        progress += size;

        if bytes[progress..].len() < 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let sequence = u32::from_le_bytes(bytes[progress..progress + 4].try_into().unwrap());
        progress += 4;

        Ok((Self::new(previous_output, signature_script, sequence), progress))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BitcoinTransaction {
    pub version: i32,
    pub inputs: Vec<TransactionInput>,
    pub lock_time: u32,
}

impl BitcoinTransaction {
    pub fn new(version: i32, inputs: Vec<TransactionInput>, lock_time: u32) -> Self {
        Self {
            version,
            inputs,
            lock_time,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = self.version.to_le_bytes().to_vec();
        v.extend_from_slice(&CompactSize::new(self.inputs.len() as u64).to_bytes());
        for input in &self.inputs {
            v.extend_from_slice(&input.to_bytes());
        }
        v.extend_from_slice(&self.lock_time.to_le_bytes());
        v
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let mut progress = 0;

        if bytes.len() < 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let version = i32::from_le_bytes(bytes[0..4].try_into().unwrap());
        progress += 4;

        let (input_count, size) = CompactSize::from_bytes(&bytes[progress..])?;
        progress += size;

        let mut inputs = Vec::with_capacity(input_count.value as usize);
        for _ in 0..input_count.value {
            let (input, size) = TransactionInput::from_bytes(&bytes[progress..])?;
            progress += size;
            inputs.push(input);
        }

        if bytes[progress..].len() < 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let lock_time = u32::from_le_bytes(bytes[progress..progress + 4].try_into().unwrap());
        progress += 4;

        Ok((Self::new(version, inputs, lock_time), progress))
    }
}

// Implemented Display for your last unit test requirement
impl fmt::Display for BitcoinTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Version: {}", self.version)?;
        writeln!(f, "Lock Time: {}", self.lock_time)?;
        for (i, input) in self.inputs.iter().enumerate() {
            writeln!(f, "  Input {}:", i)?;
            writeln!(f, "    Previous Output Vout: {}", input.previous_output.vout)?;
        }
        Ok(())
    }
}