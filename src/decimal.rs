//! Exact decimal/rational operations; rounding only at the K12 display boundary.
use crate::{Error, Result};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};
use serde_json::Value;
pub type Rational = BigRational;

pub fn parse(s: &str) -> Result<Rational> {
    let bad = || Error::arg(format!("invalid exact decimal {s:?}"));
    if s.is_empty() || s.trim() != s {
        return Err(bad());
    }
    let (mantissa, exponent) = if let Some(i) = s.find(['e', 'E']) {
        (&s[..i], s[i + 1..].parse::<i32>().map_err(|_| bad())?)
    } else {
        (s, 0)
    };
    let negative = mantissa.starts_with('-');
    let digits = if negative { &mantissa[1..] } else { mantissa };
    let parts: Vec<_> = digits.split('.').collect();
    if parts.len() > 2
        || parts[0].is_empty()
        || !parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(bad());
    }
    let scale = parts.get(1).map_or(0, |s| s.len()) as i64 - i64::from(exponent);
    if scale.unsigned_abs() > 1_000_000 {
        return Err(Error::arg(
            "exact decimal exceeds host exponent limit 1000000",
        ));
    }
    let mut numerator = BigInt::parse_bytes(parts.concat().as_bytes(), 10).ok_or_else(bad)?;
    if negative {
        numerator = -numerator;
    }
    let ten = BigInt::from(10);
    Ok(if scale >= 0 {
        Rational::new(numerator, ten.pow(scale as u32))
    } else {
        Rational::from_integer(numerator * ten.pow((-scale) as u32))
    })
}
pub fn value(v: &Value) -> Result<Rational> {
    match v {
        Value::String(s) => parse(s),
        Value::Number(n) => parse(&n.to_string()),
        _ => Err(Error::arg("expected decimal number or string")),
    }
}
pub fn nonnegative(v: &Value) -> Result<Rational> {
    let q = value(v)?;
    if q.is_negative() {
        Err(Error::arg("decimal must be nonnegative"))
    } else {
        Ok(q)
    }
}
pub fn integer(n: u64) -> Rational {
    Rational::from_integer(n.into())
}
pub fn ceil(q: &Rational) -> BigInt {
    let n = q.numer() / q.denom();
    if q.is_positive() && q.numer() % q.denom() != BigInt::zero() {
        n + 1
    } else {
        n
    }
}
fn ten_power(e: i64) -> Rational {
    if e >= 0 {
        Rational::from_integer(BigInt::from(10).pow(e as u32))
    } else {
        Rational::new(BigInt::one(), BigInt::from(10).pow((-e) as u32))
    }
}
pub fn upper(q: &Rational) -> Result<String> {
    if q.is_negative() {
        return Err(Error::arg("resource value must be nonnegative"));
    }
    if q.is_zero() {
        return Ok("0".into());
    }
    let mut e = q.numer().to_str_radix(10).len() as i64 - q.denom().to_str_radix(10).len() as i64;
    while q < &ten_power(e) {
        e -= 1;
    }
    while q >= &ten_power(e + 1) {
        e += 1;
    }
    let scale = e - 33;
    let rounded = ceil(&(q / ten_power(scale))).to_str_radix(10);
    let mut text = if scale >= 0 {
        format!("{rounded}{}", "0".repeat(scale as usize))
    } else {
        let frac = (-scale) as usize;
        if rounded.len() > frac {
            let p = rounded.len() - frac;
            format!("{}.{}", &rounded[..p], &rounded[p..])
        } else {
            format!("0.{}{rounded}", "0".repeat(frac - rounded.len()))
        }
    };
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    Ok(text)
}
/// Exact spelling for a terminating rational, used for bindings and decisions.
pub fn exact(q: &Rational) -> Result<String> {
    let mut denominator = q.denom().clone();
    let mut twos = 0;
    let mut fives = 0;
    while &denominator % 2 == BigInt::zero() {
        denominator /= 2;
        twos += 1;
    }
    while &denominator % 5 == BigInt::zero() {
        denominator /= 5;
        fives += 1;
    }
    if denominator != BigInt::one() {
        return Err(Error::arg(
            "nonterminating rational needs an explicit decimal enclosure",
        ));
    }
    let scale = twos.max(fives);
    let n = q.numer() * BigInt::from(2).pow(scale - twos) * BigInt::from(5).pow(scale - fives);
    let negative = n.is_negative();
    let digits = n.abs().to_string();
    let frac = scale as usize;
    let mut s = if frac == 0 {
        digits
    } else if digits.len() > frac {
        let p = digits.len() - frac;
        format!("{}.{}", &digits[..p], &digits[p..])
    } else {
        format!("0.{}{digits}", "0".repeat(frac - digits.len()))
    };
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if negative && s != "0" {
        s.insert(0, '-');
    }
    Ok(s)
}
/// Choose a finite downward share; consumers evaluate exactly these same bytes.
pub fn share(epsilon: &Rational, count: u64) -> Result<Rational> {
    let q = epsilon / integer(count.max(1));
    if exact(&q).is_ok() {
        return Ok(q);
    }
    let exponent =
        q.numer().to_str_radix(10).len() as i64 - q.denom().to_str_radix(10).len() as i64;
    let digits = u32::try_from((100 - exponent).max(0))
        .map_err(|_| Error::arg("epsilon share host exponent limit"))?;
    let scale = BigInt::from(10).pow(digits);
    let scaled = &q * Rational::from_integer(scale.clone());
    Ok(Rational::new(scaled.to_integer(), scale))
}
