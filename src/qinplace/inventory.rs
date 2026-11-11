//! K92 static owner inventory includes source sites in unentered loop bodies.
use super::*;

fn arguments(node: &object::Node) -> Result<Option<Vec<String>>> {
    let words = std::iter::once(node.op.clone())
        .chain(node.operands.clone())
        .collect::<Vec<_>>();
    let mut i = 0;
    loop {
        match words.get(i).map(String::as_str) {
            Some("adj") => i += 1,
            Some("ctl") => i += 2,
            Some("qinplace") => {
                if node.lhs.is_some() {
                    return Err(Error::arg("qinplace has no result binding"));
                }
                return Ok(Some(words[i + 1..].to_vec()));
            }
            Some(_) => return Ok(None),
            None => return Err(Error::arg("incomplete source instruction prefix")),
        }
    }
}
fn environment(function: &Function, inv: &crate::source::Invocation) -> Result<Env> {
    let mut ids = inv.token_ids.iter();
    let mut env = Env::new();
    for (parameter, value) in function.groups[3]
        .iter()
        .zip(inv.params.as_array().expect("validated invocation args"))
    {
        let token_identity = if parameter.ty == "token" {
            Some(
                *ids.next()
                    .ok_or_else(|| Error::arg("static pair token identity missing"))?,
            )
        } else {
            None
        };
        env.insert(
            parameter.name.clone(),
            Binding::Value {
                isa_type: json!(parameter.ty),
                value: value.clone(),
                token_identity,
            },
        );
    }
    if ids.next().is_some() {
        return Err(Error::arg("static pair has surplus token identities"));
    }
    Ok(env)
}
fn establish(
    object: &Object,
    g: &str,
    h: &str,
    mut budget: [u64; 4],
    params: &Value,
    ids: &[u64],
) -> Result<([u64; 4], Value, Value, Value)> {
    let mut params = params.clone();
    loop {
        match verify_with_ids(object, g, h, budget, &params, ids) {
            Ok((domain, tokens)) => return Ok((budget, params, domain, tokens)),
            Err(e) if e.status == Status::ScheduleInfeasible => {
                budget[0] = match budget[0] {
                    0 => 1,
                    n => n.checked_mul(2).ok_or_else(|| {
                        Error::new(
                            Status::NoMem,
                            "static inverse-pair proof search exceeds host budget counter",
                        )
                    })?,
                };
                // This is a standalone static theorem, not an entered call.
                // Keep its primary equality class tuple-consistent while the
                // verified proof budget grows; invocation bindings never change.
                let mut token_ids = ids.iter().skip(1);
                for (parameter, value) in object.functions[g].groups[4]
                    .iter()
                    .zip(params.as_array_mut().expect("static proof args"))
                {
                    if parameter.ty == "token"
                        && token_ids.next().expect("bound token identity") == &ids[0]
                    {
                        *value = json!(budget.map(|n| n.to_string()));
                    }
                }
            }
            Err(e) => return Err(e),
        }
    }
}
pub(crate) fn complete(object: &Object, trace: &mut Trace) -> Result<()> {
    for owner in 0..trace.invocations.len() {
        let inv = &trace.invocations[owner];
        let function = &object.functions[&inv.name];
        if !["action", "perm_action"].contains(&function.kind.as_str()) {
            continue;
        }
        let env = environment(function, inv)?;
        for node in object::flattened(&function.body) {
            let Some(args) = arguments(node)? else {
                continue;
            };
            if ![4, 7].contains(&args.len()) {
                return Err(Error::arg(
                    "qinplace requires two oracles, token, x and optional lists",
                ));
            }
            let g = args[0]
                .strip_prefix('@')
                .and_then(|n| object.functions.get(n))
                .ok_or_else(|| Error::arg("unresolved static qinplace oracle"))?;
            let h = args[1]
                .strip_prefix('@')
                .and_then(|n| object.functions.get(n))
                .ok_or_else(|| Error::arg("unresolved static qinplace inverse"))?;
            if g.kind != "oracle"
                || h.kind != "oracle"
                || ["in", "tgt", "fault", "mem", "args"]
                    .iter()
                    .any(|field| g.signature[*field] != h.signature[*field])
            {
                return Err(Error::arg("static qinplace oracle signature mismatch"));
            }
            let actuals = if args.len() == 7 {
                object::list(&args[6])?
            } else {
                vec![]
            };
            if actuals.len() != g.groups[4].len() {
                return Err(Error::arg(
                    "static qinplace classical argument count mismatch",
                ));
            }
            let (budget, params, token_ids) = bind(object, g, &args, &actuals, &env)?;
            let gh = object.closure_hash(&g.name)?.to_owned();
            let hh = object.closure_hash(&h.name)?.to_owned();
            if trace.qinplaces.iter().any(|p| {
                p.owner == owner
                    && ((p.g == gh && p.ginv == hh) || (p.g == hh && p.ginv == gh))
                    && p.budget == budget
                    && p.params == params
                    && p.token_ids == token_ids
            }) {
                continue;
            }
            let (budget, params, domain, arg_tokens) =
                establish(object, &g.name, &h.name, budget, &params, &token_ids)?;
            trace.qinplaces.push(Proof {
                owner,
                g: gh,
                ginv: hh,
                budget,
                params,
                domain,
                arg_tokens,
                token_ids,
                execution: None,
            });
        }
    }
    Ok(())
}
