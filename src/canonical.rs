//! RFC 8785 serialization with duplicate-key rejection and UTF-16 key ordering.
use crate::{Error, Result};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn parse(bytes: &[u8]) -> Result<Value> {
    struct Parser<'a> {
        bytes: &'a [u8],
        pos: usize,
    }
    impl Parser<'_> {
        fn ws(&mut self) {
            while self
                .bytes
                .get(self.pos)
                .is_some_and(|b| b" \t\n\r".contains(b))
            {
                self.pos += 1;
            }
        }
        fn string(&mut self) -> Result<String> {
            let begin = self.pos;
            self.pos += 1;
            loop {
                match self.bytes.get(self.pos) {
                    Some(b'"') => {
                        self.pos += 1;
                        return Ok(serde_json::from_slice(&self.bytes[begin..self.pos])?);
                    }
                    Some(b'\\') => self.pos += 2,
                    Some(_) => self.pos += 1,
                    None => return Err(Error::arg("JSON: unterminated string")),
                }
            }
        }
        fn expect(&mut self, c: u8) -> Result<()> {
            self.ws();
            if self.bytes.get(self.pos) != Some(&c) {
                return Err(Error::arg(format!(
                    "JSON: expected {} at byte {}",
                    c as char, self.pos
                )));
            }
            self.pos += 1;
            Ok(())
        }
        fn value(&mut self, depth: usize) -> Result<Value> {
            if depth > 256 {
                return Err(Error::arg("JSON: host nesting limit 256"));
            }
            self.ws();
            match self.bytes.get(self.pos) {
                Some(b'"') => Ok(Value::String(self.string()?)),
                Some(b'{') => {
                    self.pos += 1;
                    self.ws();
                    let mut map = Map::new();
                    if self.bytes.get(self.pos) == Some(&b'}') {
                        self.pos += 1;
                        return Ok(Value::Object(map));
                    }
                    loop {
                        self.ws();
                        if self.bytes.get(self.pos) != Some(&b'"') {
                            return Err(Error::arg("JSON: expected object key"));
                        }
                        let key = self.string()?;
                        self.expect(b':')?;
                        let val = self.value(depth + 1)?;
                        if map.insert(key.clone(), val).is_some() {
                            return Err(Error::arg(format!("JSON: duplicate key {key:?}")));
                        }
                        self.ws();
                        if self.bytes.get(self.pos) == Some(&b'}') {
                            self.pos += 1;
                            break;
                        }
                        self.expect(b',')?;
                    }
                    Ok(Value::Object(map))
                }
                Some(b'[') => {
                    self.pos += 1;
                    self.ws();
                    let mut items = Vec::new();
                    if self.bytes.get(self.pos) == Some(&b']') {
                        self.pos += 1;
                        return Ok(Value::Array(items));
                    }
                    loop {
                        items.push(self.value(depth + 1)?);
                        self.ws();
                        if self.bytes.get(self.pos) == Some(&b']') {
                            self.pos += 1;
                            break;
                        }
                        self.expect(b',')?;
                    }
                    Ok(Value::Array(items))
                }
                Some(_) => {
                    let begin = self.pos;
                    while self
                        .bytes
                        .get(self.pos)
                        .is_some_and(|c| !c.is_ascii_whitespace() && !b",]}".contains(c))
                    {
                        self.pos += 1;
                    }
                    if self.pos == begin {
                        return Err(Error::arg("JSON: empty value"));
                    }
                    let v: Value = serde_json::from_slice(&self.bytes[begin..self.pos])?;
                    if !matches!(v, Value::Null | Value::Bool(_) | Value::Number(_)) {
                        return Err(Error::arg("JSON: invalid scalar"));
                    }
                    Ok(v)
                }
                None => Err(Error::arg("JSON: expected value")),
            }
        }
    }
    let mut p = Parser { bytes, pos: 0 };
    let v = p.value(0)?;
    p.ws();
    if p.pos != bytes.len() {
        return Err(Error::arg("JSON: trailing bytes"));
    }
    Ok(v)
}
pub fn to_vec(value: &Value) -> Result<Vec<u8>> {
    fn emit(v: &Value, out: &mut Vec<u8>) -> Result<()> {
        match v {
            Value::Null => out.extend_from_slice(b"null"),
            Value::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
            Value::Number(n) => {
                let f = n
                    .as_f64()
                    .filter(|n| n.is_finite())
                    .ok_or_else(|| Error::arg("JCS: nonfinite binary64 number"))?;
                out.extend_from_slice(ryu_js::Buffer::new().format(f).as_bytes());
            }
            Value::String(s) => out.extend_from_slice(serde_json::to_string(s)?.as_bytes()),
            Value::Array(a) => {
                out.push(b'[');
                for (i, item) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(b',');
                    }
                    emit(item, out)?;
                }
                out.push(b']');
            }
            Value::Object(m) => {
                let mut keys: Vec<_> = m.keys().collect();
                keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
                out.push(b'{');
                for (i, k) in keys.into_iter().enumerate() {
                    if i > 0 {
                        out.push(b',');
                    }
                    out.extend_from_slice(serde_json::to_string(k)?.as_bytes());
                    out.push(b':');
                    emit(&m[k], out)?;
                }
                out.push(b'}');
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    emit(value, &mut out)?;
    Ok(out)
}
pub fn canonicalize(bytes: &[u8]) -> Result<Vec<u8>> {
    to_vec(&parse(bytes)?)
}
pub fn require_jcs(bytes: &[u8]) -> Result<Value> {
    let v = parse(bytes)?;
    if to_vec(&v)? != bytes {
        return Err(Error::arg("RFC 8785: input is not canonical JCS"));
    }
    Ok(v)
}
pub fn is_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
