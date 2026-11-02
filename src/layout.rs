//! Load-time byte layout checks (§7, §9.1). Address arithmetic is mathematical,
//! including exclusive endpoints equal to 2^ptr_width.
use crate::{Error, Result, object::{self, Object}, schedule::{fields, text, uint}};
use num_bigint::BigUint;
use num_traits::{One, Zero};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Global {
    pub name: String,
    pub base: BigUint,
    pub size: BigUint,
    pub image: String,
}
#[derive(Clone, Debug)]
pub struct Mapping {
    pub name: String,
    pub base: BigUint,
    pub extent: BigUint,
    pub position: usize,
}
#[derive(Clone, Debug)]
pub struct Layout {
    pub h: u64,
    pub a_tab: u64,
    pub s_cells: u64,
    pub null_size: BigUint,
    pub heap_base: BigUint,
    pub heap_limit_max: BigUint,
    /// JCS key order, which is the layout's normative order.
    pub globals: Vec<Global>,
    pub interface_regions: BTreeMap<String, Vec<Mapping>>,
}
pub fn integer(value: &Value) -> Result<BigUint> {
    let s = match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => return Err(Error::arg("layout requires nonnegative integers")),
    };
    if s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()) || (s.len() > 1 && s.starts_with('0')) {
        return Err(Error::arg("layout requires canonical nonnegative integers"));
    }
    s.parse().map_err(|_| Error::arg("invalid layout integer"))
}
// Test n <= 2^exponent without constructing that (possibly enormous) power.
pub fn within_power(n: &BigUint, exponent: u64) -> bool {
    n.bits() <= exponent || (n.bits().checked_sub(1) == Some(exponent)
        && !n.is_zero() && (n & (n - BigUint::one())).is_zero())
}
fn power(exponent: u64) -> Result<BigUint> {
    // This validator uses at most 8 MiB for a materialized extent integer.
    // Exhausting that host workspace is NoMem, never an unsupported ISA width.
    let words = exponent.checked_div(32).and_then(|n| n.checked_add(1))
        .filter(|n| *n <= 8 * 1024 * 1024 / 4)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| crate::Error::new(crate::Status::NoMem,"layout extent exceeds 8 MiB host integer workspace"))?;
    let mut limbs=Vec::new();
    limbs.try_reserve_exact(words).map_err(|_| crate::Error::new(crate::Status::NoMem,"layout extent exceeds host memory"))?;
    limbs.resize(words,0);
    limbs[words-1]=1u32 << (exponent % 32);
    Ok(BigUint::new(limbs))
}
fn map(value: &Value) -> Result<&serde_json::Map<String, Value>> {
    value.as_object().ok_or_else(|| Error::arg("layout map must be an object"))
}
fn disjoint(ranges: &[(BigUint, BigUint, String)]) -> Result<()> {
    let mut ranges = ranges.iter().filter(|(base,end,_)| base < end).collect::<Vec<_>>();
    ranges.sort_by(|a,b| a.0.cmp(&b.0));
    for pair in ranges.windows(2) {
        if pair[0].1 > pair[1].0 {
            return Err(Error::arg(format!("§7: layout overlap: {} and {}", pair[0].2, pair[1].2)));
        }
    }
    Ok(())
}
impl Layout {
    pub fn from_object(object: &Object) -> Result<Option<Self>> {
        let Some(v) = object.metadata.get("layout") else { return Ok(None) };
        fields(v, &["ptr_width", "H", "A_tab", "S_cells", "null_size", "heap_base", "heap_limit_max", "globals", "interface_regions"], &["block_image"])?;
        if uint(&v["ptr_width"])? != object.ptr_width {
            return Err(Error::arg("§9.1: layout ptr_width differs from the object"));
        }
        let h = uint(&v["H"])?;
        let a_tab = uint(&v["A_tab"])?;
        let s_cells = uint(&v["S_cells"])?;
        if h > object.ptr_width { return Err(Error::arg("§7.2: H exceeds ptr_width")); }
        let null_size = integer(&v["null_size"])?;
        if null_size.is_zero() { return Err(Error::arg("§7.1: null_size must be positive")); }
        let heap_base = integer(&v["heap_base"])?;
        let heap_limit_max = integer(&v["heap_limit_max"])?;
        let mut ranges = vec![(BigUint::zero(), null_size.clone(), "null".into()),
            (heap_base.clone(), &heap_base+&heap_limit_max, "heap".into())];
        let mut globals = Vec::new();
        for (name, g) in map(&v["globals"])? {
            fields(g, &["base", "size", "image"], &[])?;
            let image = text(&g["image"])?;
            if image != name || object.symbols.get(name).is_none_or(|s| s.kind != "data") {
                return Err(Error::arg("§9.2: global image must be its own data symbol"));
            }
            let size = integer(&g["size"])?;
            if size != integer(&object.symbols[name].signature["size"])?
                || object.data.get(name).is_none_or(|s| BigUint::from(s.len()/2) != size) {
                return Err(Error::arg("§9.2: global size differs from its data image size"));
            }
            let base = integer(&g["base"])?;
            ranges.push((base.clone(), &base+&size, format!("global {name}")));
            globals.push(Global {name:name.clone(),base,size,image:image.into()});
        }
        if !within_power(&BigUint::from(globals.len()), a_tab) {
            return Err(Error::arg("§7.1: globals exceed allocation-table pre-entries"));
        }
        for (base,end,_) in &ranges {
            if !within_power(base, object.ptr_width) || !within_power(end, object.ptr_width) {
                return Err(Error::arg("§7.1: layout range exceeds pointer address space"));
            }
        }
        disjoint(&ranges)?;
        let owners = map(&v["interface_regions"])?;
        for (name, mapping) in owners {
            if object.functions.get(name).is_none_or(|f| f.kind != "oracle") {
                return Err(Error::arg("§9.1: interface mapping owner must name an oracle"));
            }
            for m in map(mapping)?.values() { fields(m, &["base"], &[])?; }
        }
        let imports = object.metadata.get("imports").and_then(Value::as_object);
        // Names of externally passed region cells are never aliases for @symbols.
        for f in object.functions.values().filter(|f| ["oracle","action","perm_action"].contains(&f.kind.as_str())) {
            let group = if f.kind == "oracle" {3} else {2};
            for p in &f.groups[group] {
                if object.symbols.contains_key(&p.name) || imports.is_some_and(|i| i.contains_key(&p.name)) {
                    return Err(Error::arg("§9.1: interface region name collides with a symbol or import"));
                }
            }
        }
        let mut interface_regions = BTreeMap::new();
        for f in object.functions.values().filter(|f| f.kind == "oracle") {
            let parameters = &f.groups[3];
            let mappings = owners.get(&f.name).map(map).transpose()?;
            if mappings.map_or(0, |m| m.len()) != parameters.len()
                || mappings.is_some_and(|m| m.keys().any(|n| !parameters.iter().any(|p| p.name == *n))) {
                return Err(Error::arg("§7.2: interface mapping does not match oracle parameters"));
            }
            let mut selected = Vec::new();
            let mut oracle_ranges = ranges.clone();
            if let Some(mappings) = mappings {
                for (name, m) in mappings {
                    let position = parameters.iter().position(|p| p.name == *name).expect("validated mapping");
                    let (a,w) = object::region_shape(&parameters[position].ty)?;
                    if w != 8 { return Err(Error::arg("§7.2: mapped interface cells require W=8")); }
                    let base = integer(&m["base"])?;
                    // A valid nonempty mapped interval must fit H, so test A first
                    // before constructing 2^A from untrusted metadata.
                    if a > h { return Err(Error::arg("§7.2: mapped region exceeds lifted address space")); }
                    let extent = power(a)?;
                    oracle_ranges.push((base.clone(), &base+&extent, format!("region {name}")));
                    selected.push(Mapping{name:name.clone(),base,extent,position});
                }
            }
            if !within_power(&BigUint::from(globals.len()+selected.len()), a_tab) {
                return Err(Error::arg("§7.2: oracle pre-entries exceed allocation table"));
            }
            for (base,end,_) in &oracle_ranges {
                if !within_power(base,h) || !within_power(end,h) {
                    return Err(Error::arg("§7.2: range exceeds lifted address space"));
                }
            }
            disjoint(&oracle_ranges)?;
            interface_regions.insert(f.name.clone(),selected);
        }
        if let Some(image) = v.get("block_image") {
            let image = text(image)?;
            if object.symbols.get(image).is_none_or(|s| s.kind != "data")
                || object.data.get(image).is_none_or(|s| s.len()/2 % 512 != 0) {
                return Err(Error::arg("§7.3: block_image must name a data image of complete 512-byte sectors"));
            }
        }
        Ok(Some(Self{h,a_tab,s_cells,null_size,heap_base,heap_limit_max,globals,interface_regions}))
    }
}
