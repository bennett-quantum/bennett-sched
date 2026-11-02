//! Source-derived K25 construction descriptors, independent of gate expansion.
//! Frames are iterated lazily; an invocation's B never allocates B host records
//! just to compute its shape. Large zero images are likewise sparse until used.
use crate::{Error, Result, Status, layout::Layout, lproc::IndexSpace,
    object::{self, Function, Node, Object}};
use num_bigint::BigUint;
use num_traits::{ToPrimitive, Zero};
use std::collections::BTreeMap;

fn memory() -> Error { Error::new(Status::NoMem,"oracle state descriptor exceeds host size") }
fn add(a:u64,b:u64)->Result<u64>{a.checked_add(b).ok_or_else(memory)}
fn mul(a:u64,b:u64)->Result<u64>{a.checked_mul(b).ok_or_else(memory)}
fn power(exponent:u64)->Result<u64>{u32::try_from(exponent).ok().and_then(|e|1u64.checked_shl(e)).ok_or_else(memory)}
fn host(n:&BigUint)->Result<u64>{n.to_u64().ok_or_else(memory)}

#[derive(Clone,Debug)]
pub struct Image {
    pub bytes:u64,
    pub overlays:Vec<(u64,Vec<u8>)>,
}
impl Image {
    /// `limit` is the caller's available host buffer budget, not an ISA bound.
    pub fn materialize(&self,limit:usize)->Result<Vec<u8>>{
        let n=usize::try_from(self.bytes).map_err(|_|memory())?;
        if n>limit{return Err(memory())}
        let mut bytes=Vec::new();
        bytes.try_reserve_exact(n).map_err(|_|memory())?;
        bytes.resize(n,0);
        for (base,image) in &self.overlays {
            let start=usize::try_from(*base).map_err(|_|memory())?;
            let end=start.checked_add(image.len()).ok_or_else(memory)?;
            let destination=bytes.get_mut(start..end).ok_or_else(||Error::arg("global image exceeds ordinary memory"))?;
            destination.copy_from_slice(image);
        }
        Ok(bytes)
    }
}
#[derive(Clone,Debug)]
pub struct State {
    pub name:String,
    pub width:u64,
    pub initial:BigUint,
    pub image:Option<Image>,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum ByteStorage {Ordinary{offset:u64},Interface{position:usize,offset:u64}}
#[derive(Clone,Debug)]
pub struct ByteSpan {pub base:u64,pub extent:u64,pub storage:ByteStorage}
#[derive(Clone,Debug)]
pub struct Location {pub id:u64,pub width:u64,pub slot:u64,pub storage:String}
#[derive(Clone,Debug)]
pub struct Plan {
    pub index:IndexSpace,
    pub slot_widths:Vec<u64>,
    pub frame_width:u64,
    pub cell_width:u64,
    pub id_width:u64,
    /// All non-frame state in K25 allocation order.
    pub base:Vec<State>,
    pub frame_count:u64,
    pub bytes:Vec<ByteSpan>,
    pub locations:Vec<Location>,
}
fn zero(name:impl Into<String>,width:u64)->State{
    State{name:name.into(),width,initial:BigUint::zero(),image:None}
}
fn readable(f:&Function,object:&Object)->Result<BTreeMap<String,u64>>{
    let mut names=BTreeMap::new();
    let groups=if f.kind=="oracle"{vec![0]}else{vec![0,1]};
    for p in groups.iter().flat_map(|g|f.groups[*g].iter()).chain(&f.regs){
        names.insert(p.name.clone(),object::wire_width(&p.ty)?);
    }
    let group=if f.kind=="oracle"{4}else{3};
    for p in &f.groups[group]{
        let width=match p.ty.as_str(){"token"=>continue,"ptr"=>object.ptr_width,"f64"=>64,
            s=>crate::schedule::uint(&serde_json::json!(s.strip_prefix('i').ok_or_else(||Error::arg("invalid fixed argument type"))?))?};
        names.insert(p.name.clone(),width);
    }
    Ok(names)
}
fn row_slots(node:&Node,f:&Function,object:&Object,counters:Option<(u64,u64,u64,u64)>)->Result<Vec<u64>>{
    let names=readable(f,object)?;
    let read=|n:&str|names.get(n).copied().ok_or_else(||Error::arg(format!("§3.7: invalid source operand {n}")));
    let result=match node.op.as_str(){
        "lift.call"=>{
            let (_,_,sc,dc)=counters.ok_or_else(||Error::arg("lift.call requires layout"))?;
            let operands=node.operands.get(1).ok_or_else(||Error::arg("missing call arguments"))?;
            let mut widths=object::list(operands)?.iter().map(|n|read(n)).collect::<Result<Vec<_>>>()?;
            widths.extend([sc,sc,dc]);widths
        },
        "lift.ret"=>{
            if f.kind!="lproc"{return Err(Error::arg("lift.ret outside lproc"))}
            let (_,_,sc,dc)=counters.ok_or_else(||Error::arg("lift.ret requires layout"))?;
            vec![read(node.operands.first().ok_or_else(||Error::arg("missing return value"))?)?,sc,dc]
        },
        "lift.alloc"=>{
            let (hc,ac,_,_)=counters.ok_or_else(||Error::arg("lift.alloc requires layout"))?;
            vec![object.ptr_width,add(mul(2,object.ptr_width)?,2)?,hc,ac]
        },
        op if op.starts_with("lift.store.i")=>vec![crate::schedule::uint(&serde_json::json!(&op[12..]))?],
        _=>if let Some(lhs)=&node.lhs{
            let destinations=if lhs.starts_with('('){object::list(lhs)?}else{vec![lhs.clone()]};
            destinations.iter().map(|name|{
                let valid=f.regs.iter().chain(if f.kind=="lproc"{f.groups[..2].iter().flatten().collect::<Vec<_>>()}else{vec![]}).any(|p|p.name==*name);
                if !valid{return Err(Error::arg("lifted assignment writes outside declared locations"))}
                read(name)
            }).collect::<Result<_>>()?
        }else{vec![]},
    };
    Ok(result)
}
impl Plan {
    pub fn records(&self)->impl Iterator<Item=State>+'_ {
        self.base.iter().cloned().chain((0..self.frame_count).map(|i|zero(format!("history-{i}"),self.frame_width)))
    }
    pub fn form(object:&Object,name:&str,budget:[u64;4])->Result<Self>{
        object.validate_budget(budget)?;
        let index=IndexSpace::form(object,name)?;
        let f=&object.functions[name];
        let layout=Layout::from_object(object)?;
        let counters=layout.as_ref().map(|l|->Result<_>{Ok((add(l.h,2)?,add(l.a_tab,1)?,add(l.s_cells,1)?,128-((budget[3] as u128)+1).leading_zeros() as u64))}).transpose()?;
        let location_count=index.oracle_locations.len()+index.instances.iter().map(|i|i.locations.len()).sum::<usize>();
        let id_width=if location_count<=1{0}else{usize::BITS as u64-(location_count-1).leading_zeros() as u64};
        let widest=index.oracle_locations.iter().chain(index.instances.iter().flat_map(|i|&i.locations)).map(|l|l.width).max().unwrap_or(0);
        let cell_width=index.pc_width.max(counters.map_or(0,|c|c.2)).max(id_width).max(widest);
        let mut slot_widths=Vec::<u64>::new();
        for function in std::iter::once(f).chain(index.instances.iter().map(|i|&object.functions[&i.name])){
            for node in &function.body {
                for (i,width) in row_slots(node,function,object,counters)?.into_iter().enumerate(){
                    if i==slot_widths.len(){slot_widths.push(width)}else{slot_widths[i]=slot_widths[i].max(width)}
                }
            }
        }
        let sum=slot_widths.iter().try_fold(0u64,|a,b|add(a,*b))?;
        let frame_width=add(add(mul(index.pc_width,2)?,1)?,sum)?;
        let mut base=vec![zero("pc",index.pc_width),zero("next",index.pc_width),zero("faulted",1),zero("code",object::wire_width(&f.groups[2][0].ty)?),zero("result",object::wire_width(&f.groups[1][0].ty)?)];
        let mut bytes=Vec::new();
        let mut table=None;
        if let Some(l)=layout {
            let (hc,ac,sc,dc)=counters.expect("layout counters");
            let mappings=&l.interface_regions[name];
            base.extend([zero("heap_used",hc),zero("atop",ac),zero("sp",sc),zero("fp",sc),zero("depth",dc)]);
            base[6].initial=BigUint::from(l.globals.len()+mappings.len());
            let extent=power(l.h)?;
            let mut overlays=Vec::new();
            for g in &l.globals{
                let image=&object.data[&g.image];
                let data=(0..image.len()).step_by(2).map(|i|u8::from_str_radix(&image[i..i+2],16).expect("validated image")).collect::<Vec<_>>();
                overlays.push((host(&g.base)?,data));
            }
            let mut mem=zero("mem",mul(extent,8)?);
            mem.image=Some(Image{bytes:extent,overlays});
            base.push(mem);
            base.push(zero("stack",mul(power(l.s_cells)?,cell_width)?));
            let entry_width=add(mul(object.ptr_width,2)?,2)?;
            let table_width=mul(power(l.a_tab)?,entry_width)?;
            let mut initial=BigUint::zero();
            let pre=l.globals.iter().map(|g|(&g.base,&g.size)).chain(mappings.iter().map(|m|(&m.base,&m.extent)));
            for (i,(address,size)) in pre.enumerate(){
                let packed=address+(size<<usize::try_from(object.ptr_width).map_err(|_|memory())?)+(BigUint::from(3u8)<<usize::try_from(mul(object.ptr_width,2)?).map_err(|_|memory())?);
                initial|=packed<<usize::try_from(mul(i as u64,entry_width)?).map_err(|_|memory())?;
            }
            let mut state=zero("alloctab",table_width);state.initial=initial;table=Some(state);
            let mut sorted=mappings.iter().collect::<Vec<_>>();sorted.sort_by(|a,b|a.base.cmp(&b.base));
            let mut cursor=0;
            for m in sorted{
                let start=host(&m.base)?;
                if cursor<start{bytes.push(ByteSpan{base:cursor,extent:start-cursor,storage:ByteStorage::Ordinary{offset:cursor}})}
                let size=host(&m.extent)?;
                bytes.push(ByteSpan{base:start,extent:size,storage:ByteStorage::Interface{position:m.position,offset:0}});
                cursor=add(start,size)?;
            }
            if cursor<extent{bytes.push(ByteSpan{base:cursor,extent:extent-cursor,storage:ByteStorage::Ordinary{offset:cursor}})}
        }
        let mut locations=Vec::new();
        for (i,list) in std::iter::once(&index.oracle_locations).chain(index.instances.iter().map(|i|&i.locations)).enumerate(){
            for (slot,location) in list.iter().enumerate(){
                let storage=if i==0{format!("source.{}",location.name)}else{format!("instance/{}/source.{}",i-1,location.name)};
                base.push(zero(&storage,location.width));
                locations.push(Location{id:location.id,width:location.width,slot:slot as u64,storage});
            }
        }
        if let Some(table)=table{base.push(table)}
        Ok(Self{index,slot_widths,frame_width,cell_width,id_width,base,frame_count:budget[0],bytes,locations})
    }
}
