//! Shared additive-then-multiplicative modifier evaluator.
#[derive(Clone,Debug,serde::Serialize,serde::Deserialize)]
pub struct Effect { pub stat: String, pub add: f32, pub multiply: f32 }
pub fn evaluate<'a>(stat:&str,base:f32,effects:impl IntoIterator<Item=&'a Effect>)->f32 {
    let (mut add,mut multiply)=(0.0,1.0);
    for e in effects {if e.stat==stat {add+=e.add;multiply*=e.multiply;}}
    ((base+add)*multiply).max(0.0)
}
