//! Published native-model syntax and exact rational obligations (§8.3).
//! Symbolic normalization/accuracy remains an independent checker obligation.
use super::*;
use num_bigint::BigInt;
use std::str::FromStr;

type Complex = (Rational, Rational);
type Matrix = Vec<Vec<Complex>>;
const VERSION: &str = "bennett.native-model/1";

fn rational(value: &Value) -> Result<Rational> {
    let pair = array(value)?;
    if pair.len() != 2 {
        return Err(Error::target(
            "native model rational needs numerator and denominator",
        ));
    }
    let n =
        BigInt::from_str(text(&pair[0])?).map_err(|_| Error::target("invalid model numerator"))?;
    let d = BigInt::from_str(text(&pair[1])?)
        .map_err(|_| Error::target("invalid model denominator"))?;
    if d <= BigInt::zero() || n.to_string() != text(&pair[0])? || d.to_string() != text(&pair[1])? {
        return Err(Error::target("noncanonical native model rational"));
    }
    let q = Rational::new(n.clone(), d.clone());
    if q.numer() != &n || q.denom() != &d {
        return Err(Error::target("native model rational must be reduced"));
    }
    Ok(q)
}

fn expression(value: &Value, bindings: &Value) -> Result<Option<Rational>> {
    if value.is_array() {
        return rational(value).map(Some);
    }
    if let Some(name) = value.get("var") {
        fields(value, &["var"], &[])?;
        if bindings.get(text(name)?).is_none() {
            return Err(Error::target("native model references undeclared binding"));
        }
        return Ok(None);
    }
    fields(value, &["op", "args"], &[])?;
    let op = text(&value["op"])?;
    let args = array(&value["args"])?;
    let arity = match op {
        "pi" | "i" => 0,
        "sqrt" | "exp" | "sin" | "cos" => 1,
        "+" | "-" | "*" | "/" => 2,
        _ => return Err(Error::target("unknown native model expression operator")),
    };
    if args.len() != arity {
        return Err(Error::target("native model expression arity mismatch"));
    }
    let evaluated = args
        .iter()
        .map(|v| expression(v, bindings))
        .collect::<Result<Vec<_>>>()?;
    let [Some(x), Some(y)] = evaluated.as_slice() else {
        return Ok(None);
    };
    Ok(Some(match op {
        "+" => x + y,
        "-" => x - y,
        "*" => x * y,
        "/" if !y.is_zero() => x / y,
        "/" => return Err(Error::target("native model expression divides by zero")),
        _ => unreachable!("only binary rational operators have two arguments"),
    }))
}

fn matrix(model: &Value) -> Result<Option<Matrix>> {
    let Some(encoded) = model["data"].get("matrix") else {
        // Other published representations must supply phase-bearing U and be
        // independently reconstructed; retaining them is not an ideal fallback.
        return Ok(None);
    };
    let rows = array(encoded)?;
    if !rows.len().is_power_of_two() || rows.len().trailing_zeros() as u64 != uint(&model["arity"])?
    {
        return Err(Error::target(
            "native model matrix dimension disagrees with arity",
        ));
    }
    let mut values = Vec::new();
    let mut exact = true;
    for row in rows {
        let entries = array(row)?;
        if entries.len() != rows.len() {
            return Err(Error::target("native model matrix must be square"));
        }
        let mut out = Vec::new();
        for entry in entries {
            let pair = array(entry)?;
            if pair.len() != 2 {
                return Err(Error::target(
                    "native complex entry needs real and imaginary parts",
                ));
            }
            match (
                expression(&pair[0], &model["parameter_bindings"])?,
                expression(&pair[1], &model["parameter_bindings"])?,
            ) {
                (Some(re), Some(im)) => out.push((re, im)),
                _ => exact = false,
            }
        }
        values.push(out);
    }
    Ok(exact.then_some(values))
}

fn unitary(matrix: &Matrix) -> Result<()> {
    for i in 0..matrix.len() {
        for j in 0..matrix.len() {
            let mut re = Rational::zero();
            let mut im = Rational::zero();
            for row in matrix {
                let (a, b) = &row[i];
                let (c, d) = &row[j];
                re += a * c + b * d;
                im += a * d - b * c;
            }
            if re != d::integer(u64::from(i == j)) || !im.is_zero() {
                return Err(Error::target("native model matrix is not unitary"));
            }
        }
    }
    Ok(())
}

pub(super) fn validate(model: &Value) -> Result<()> {
    let checked = || -> Result<()> {
        if text(&model["version"])? != VERSION {
            return Ok(());
        }
        fields(
            model,
            &[
                "version",
                "kind",
                "arity",
                "basis",
                "parameter_bindings",
                "control_models",
                "data",
                "operand_map",
            ],
            &[],
        )?;
        if model["kind"] != "unitary" {
            return Err(Error::target("native model kind must be unitary"));
        }
        let basis = array(&model["basis"])?;
        let names = basis.iter().map(text).collect::<Result<BTreeSet<_>>>()?;
        if names.len() != basis.len()
            || names.contains("")
            || basis.len() as u64 != uint(&model["arity"])?
        {
            return Err(Error::target(
                "native model basis must name each operand once",
            ));
        }
        let bindings = model["parameter_bindings"]
            .as_object()
            .ok_or_else(|| Error::target("native model bindings must be an object"))?;
        for (name, binding) in bindings {
            if name.is_empty() || !["k", "c", "eps", "theta"].contains(&text(binding)?) {
                return Err(Error::target("native model has invalid parameter binding"));
            }
        }
        let controls = &model["control_models"];
        if text(&controls["kind"])? == "controlled_U" {
            fields(controls, &["kind", "controls", "disabled"], &[])?;
            if controls["controls"] != "c" || controls["disabled"] != "identity" {
                return Err(Error::target(
                    "native controlled_U needs c controls and disabled identity",
                ));
            }
        }
        let map = array(&model["operand_map"])?
            .iter()
            .map(uint)
            .collect::<Result<BTreeSet<_>>>()?;
        let width = array(&model["operand_map"])?.len();
        if map.len() != width || map.iter().copied().ne(0..width as u64) {
            return Err(Error::target(
                "native operand_map must be a complete permutation",
            ));
        }
        if model["data"].as_object().is_none_or(|data| data.is_empty()) {
            return Err(Error::target(
                "native model data must identify a phase-bearing unitary",
            ));
        }
        if let Some(matrix) = matrix(model)? {
            unitary(&matrix)?;
        }
        Ok(())
    };
    checked().map_err(|e| Error::target(format!("§8.3 native model: {}", e.reason)))
}

fn exact_opcode(matrix: &Matrix, op: &str) -> Result<()> {
    if !["x", "cx", "ccx", "mcx", "swap", "cz", "s", "sdg"].contains(&op) {
        return Ok(());
    }
    for (i, row) in matrix.iter().enumerate() {
        for (j, (re, im)) in row.iter().enumerate() {
            let (target, real, imaginary): (usize, i32, i32) = match op {
                "x" | "cx" | "ccx" | "mcx" => (j ^ 1, 1, 0),
                "swap" => (((j & 1) << 1) | ((j & 2) >> 1), 1, 0),
                "cz" => (j, if j == 3 { -1 } else { 1 }, 0),
                "s" => (j, if j == 0 { 1 } else { 0 }, if j == 0 { 0 } else { 1 }),
                "sdg" => (j, if j == 0 { 1 } else { 0 }, if j == 0 { 0 } else { -1 }),
                _ => unreachable!(),
            };
            let expected = if i == target {
                (
                    Rational::from_integer(real.into()),
                    Rational::from_integer(imaginary.into()),
                )
            } else {
                (Rational::zero(), Rational::zero())
            };
            if (re, im) != (&expected.0, &expected.1) {
                return Err(Error::target(
                    "§8.3: native model contradicts zero-epsilon opcode including phase",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn resolve(model: &Value, gate: &Gate, epsilon: &Rational) -> Result<()> {
    if model["version"] != VERSION {
        return Ok(());
    }
    let controlled = model["control_models"]["kind"] == "controlled_U";
    let arity = uint(&model["arity"])?;
    let operands = if controlled {
        arity + gate.control_wires().len() as u64
    } else {
        arity
    };
    if operands != gate.all_wires().len() as u64
        || array(&model["operand_map"])?.len() as u64 != operands
    {
        return Err(Error::target(
            "§8.3: native model resolved operand/control arity mismatch",
        ));
    }
    for binding in model["parameter_bindings"]
        .as_object()
        .expect("validated model bindings")
        .values()
    {
        if (binding == "eps" && epsilon.is_zero()) || (binding == "theta" && gate.theta.is_none()) {
            return Err(Error::target(
                "§8.3: native model parameter is unbound in this context",
            ));
        }
    }
    if epsilon.is_zero() && controlled {
        if let Some(matrix) = matrix(model)? {
            exact_opcode(&matrix, &gate.op)?;
        }
    }
    Ok(())
}
